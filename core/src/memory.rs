use std::fmt::Debug;
use std::io::Read;
use std::marker::PhantomData;
use zerocopy::TryFromBytes;

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
