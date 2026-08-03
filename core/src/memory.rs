use std::fmt::Debug;
use std::marker::PhantomData;
use zerocopy::TryFromBytes;

/// A shared-memory page that stamps each write with a counter.
///
/// AC rewrites the physics page at 333 Hz while we copy ~600 bytes out of it,
/// so a read can straddle a write and come back holding two different frames
/// spliced together. The counter is how a reader detects that: identical
/// before and after the copy means no write landed in between.
///
/// This matters more than the raw frequency suggests, because the values most
/// sensitive to it are differences and maxima — the jerk accumulators behind
/// the smoothness scores, and the peak-G tracking. A single spliced frame
/// shows up there as a phantom lockup or an impossible G reading that the
/// rest of the lap then carries.
pub trait Versioned {
    fn packet_id(&self) -> i32;
}

/// How many times to re-read before giving up and taking what we have.
///
/// A torn read needs the writer to land inside our copy, so two in a row is
/// already unlikely and three is vanishingly so. Retrying forever would be
/// worse than one bad frame.
const MAX_TEAR_RETRIES: usize = 3;

#[cfg(not(target_os = "windows"))]
pub struct SharedMemory<T> {
    mmap: memmap2::Mmap,
    _phantom: PhantomData<T>,
}

#[cfg(not(target_os = "windows"))]
unsafe impl<T> Send for SharedMemory<T> {}
#[cfg(not(target_os = "windows"))]
unsafe impl<T> Sync for SharedMemory<T> {}

#[cfg(not(target_os = "windows"))]
impl<T> SharedMemory<T> {
    pub fn connect(name: &str) -> Result<Self, Box<dyn std::error::Error>>
    where
        T: Debug,
    {
        use memmap2::Mmap;
        use std::fs::File;

        let file = File::open(name)?;
        let mmap = unsafe { Mmap::map(&file) };

        let Ok(mmap) = mmap else {
            return Err(format!("Cannot map a memory file: {}", name).into());
        };

        Ok(Self {
            mmap,
            _phantom: PhantomData,
        })
    }

    pub fn get(&self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: TryFromBytes + Debug,
    {
        use anyhow::anyhow;
        let size = std::mem::size_of::<T>();
        let bytes = &self.mmap;
        if bytes.len() < size {
            return Err(anyhow!("Incorrect buffer size").into());
        }
        let bytes = &bytes[..size];
        T::try_read_from_bytes(bytes)
            .map_err(|err| anyhow::format_err!("Error converting type: {err:?}").into())
    }

    /// Read a page, retrying while the write counter moves under us.
    ///
    /// See [`Versioned`] for why. Falls through to the last read after
    /// [`MAX_TEAR_RETRIES`]: a stale frame beats spinning on a game that is
    /// writing faster than we can read.
    pub fn get_stable(&self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: TryFromBytes + Debug + Versioned,
    {
        let mut value = self.get()?;
        for _ in 0..MAX_TEAR_RETRIES {
            let again = self.get()?;
            if again.packet_id() == value.packet_id() {
                return Ok(value);
            }
            value = again;
        }
        Ok(value)
    }
}

#[cfg(target_os = "windows")]
pub struct SharedMemory<T> {
    handle: windows::Win32::Foundation::HANDLE,
    ptr: *const u8,
    _phantom: PhantomData<T>,
}

#[cfg(target_os = "windows")]
unsafe impl<T> Send for SharedMemory<T> {}
#[cfg(target_os = "windows")]
unsafe impl<T> Sync for SharedMemory<T> {}

#[cfg(target_os = "windows")]
impl<T> SharedMemory<T> {
    pub fn connect(name: &str) -> Result<Self, Box<dyn std::error::Error>>
    where
        T: Debug,
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile, OpenFileMappingW};
        use windows::core::HSTRING;

        let h_name = HSTRING::from(name);
        unsafe {
            let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &h_name)?;

            let addr = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, std::mem::size_of::<T>());

            let ptr = addr.Value as *const u8;

            if ptr.is_null() {
                let _ = CloseHandle(handle);
                return Err("Failed to map view of file".into());
            }

            Ok(Self {
                handle,
                ptr,
                _phantom: PhantomData,
            })
        }
    }

    pub fn get(&self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: TryFromBytes + Debug,
    {
        let size = std::mem::size_of::<T>();
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, size) };
        T::try_read_from_bytes(bytes)
            .map_err(|err| anyhow::format_err!("Error converting type: {err:?}").into())
    }

    /// Read a page, retrying while the write counter moves under us.
    /// See the non-Windows implementation for the reasoning.
    pub fn get_stable(&self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: TryFromBytes + Debug + Versioned,
    {
        let mut value = self.get()?;
        for _ in 0..MAX_TEAR_RETRIES {
            let again = self.get()?;
            if again.packet_id() == value.packet_id() {
                return Ok(value);
            }
            value = again;
        }
        Ok(value)
    }
}

#[cfg(target_os = "windows")]
impl<T> Drop for SharedMemory<T> {
    fn drop(&mut self) {
        unsafe {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Memory::{MEMORY_MAPPED_VIEW_ADDRESS, UnmapViewOfFile};

            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr as _,
            });
            let _ = CloseHandle(self.handle);
        }
    }
}
