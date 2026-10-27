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

/// How often a connection asks whether its pages are still the ones being
/// written.
///
/// A `stat` of three files once a second is nothing; sixty times a second it
/// would be a hundred and eighty system calls to learn something that changes
/// a few times a day.
const RECHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// What a refresh says when the pages were published again underneath it.
pub const REPLACED: &str = "the pages were published again since this connection \
                            opened them, and the old ones are read by nobody — \
                            reconnect";

/// When a connection last asked [`SharedMemory::replaced`].
///
/// Kept by the page set rather than by each page, because the three are
/// replaced together and asking about them together is one decision.
#[derive(Default)]
pub struct Recheck {
    at: Option<std::time::Instant>,
}

impl Recheck {
    /// Whether a second has passed since the last time this said yes.
    pub fn due(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self
            .at
            .is_some_and(|at| now.duration_since(at) < RECHECK_EVERY)
        {
            return false;
        }
        self.at = Some(now);
        true
    }
}

#[cfg(not(target_os = "windows"))]
pub struct SharedMemory<T> {
    mmap: memmap2::Mmap,
    /// Where the page was opened from, so it can be asked about again.
    path: std::path::PathBuf,
    /// The device and inode that were mapped.
    ///
    /// **A mapping is of a file, not of a name.** When the bridge is started
    /// again it makes the pages afresh, and the name under `/dev/shm` then
    /// belongs to a new file while this mapping goes on holding the old one —
    /// which nothing writes to any more, and which reads as a session that
    /// stopped at whatever frame it stopped at. Nothing fails: the reader
    /// waits for a car that is already on track in the pages beside it, for
    /// as long as the program is left open. That was "it needs restarting
    /// after it has been sitting there a while".
    identity: Option<(u64, u64)>,
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
        let identity = file.metadata().ok().map(|meta| identity_of(&meta));
        let mmap = unsafe { Mmap::map(&file) };

        let Ok(mmap) = mmap else {
            return Err(format!("Cannot map a memory file: {}", name).into());
        };

        Ok(Self {
            mmap,
            path: std::path::PathBuf::from(name),
            identity,
            _phantom: PhantomData,
        })
    }

    /// Whether the name this page was opened under now belongs to another
    /// file, or to none.
    ///
    /// See [`SharedMemory::identity`]. A page that could not be identified
    /// when it was opened is never called replaced: that would reconnect on
    /// every check for as long as it was open.
    pub fn replaced(&self) -> bool {
        let Some(mapped) = self.identity else {
            return false;
        };
        match std::fs::metadata(&self.path) {
            Ok(meta) => identity_of(&meta) != mapped,
            Err(_) => true,
        }
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

#[cfg(not(target_os = "windows"))]
fn identity_of(meta: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (meta.dev(), meta.ino())
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

    /// Never, on Windows.
    ///
    /// A named section stays the same section for as long as anybody holds a
    /// handle to it, and this connection does — a game started again opens
    /// the one already there rather than making another under the same name.
    pub fn replaced(&self) -> bool {
        false
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

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    /// **The bridge started again is a new file under the old name**, and a
    /// mapping of the old one reads its last frame for ever. This is the
    /// whole of "it needs restarting after sitting there a while".
    #[test]
    fn a_page_made_again_under_the_same_name_is_noticed() {
        let folder = std::env::temp_dir().join(format!("acpe-replaced-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a scratch folder");
        let page = folder.join("a-page");
        std::fs::write(&page, [1u8; 8]).expect("a page");

        let mapped =
            SharedMemory::<u64>::connect(page.to_str().expect("a path")).expect("the page maps");
        assert!(!mapped.replaced(), "the page it opened is the page there");

        // What a bridge does when it starts: the old file goes and a new one
        // takes the name.
        std::fs::remove_file(&page).expect("the old page goes");
        assert!(
            mapped.replaced(),
            "a name with nothing under it is not ours"
        );
        std::fs::write(&page, [2u8; 8]).expect("a new page");
        assert!(mapped.replaced(), "and a new file under it is not either");
        assert_eq!(
            mapped.get().expect("the old mapping still reads"),
            u64::from_ne_bytes([1; 8]),
            "which is exactly the trap: it reads, and it is the old frame"
        );

        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Asked once a second, not sixty times.
    #[test]
    fn the_check_is_due_once_a_second() {
        let mut recheck = Recheck::default();
        assert!(recheck.due(), "the first time is always due");
        assert!(!recheck.due(), "and not again within the second");
    }
}
