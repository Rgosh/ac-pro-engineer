//! Publishing [`OverlayFrame`] into shared memory for the in-game Lua app.
//!
//! Two platforms, one file format:
//!
//! * **Windows** — a named file mapping, created directly. AC and the Lua app
//!   are in the same Win32 session, so they see `Local\<name>`.
//! * **Linux** — a plain file under `/dev/shm/`. The application is a native
//!   Linux process and cannot create a Win32 named object that Wine resolves,
//!   so `shm-bridge.exe`, which already runs under Wine whenever telemetry is
//!   being read, wraps this same file in a named mapping. Both sides then look
//!   at the same bytes. This is the mechanism the bridge already uses for AC's
//!   own pages, run in the opposite direction.
//!
//! The reader may be mid-frame while we write, so writes are guarded by a
//! sequence lock rather than a mutex — there is no lock to share across a
//! process boundary and a language boundary, and a reader that notices a torn
//! frame can simply keep the previous one.

use crate::overlay::frame::{OVERLAY_MMF_NAME, OverlayFrame};
use std::io;
use std::path::PathBuf;

/// Where the backing file lives on Linux. Wine sees this as `Z:\dev\shm\…`,
/// which is how `shm-bridge` reaches it.
#[cfg(not(target_os = "windows"))]
const SHM_DIR: &str = "/dev/shm";

/// Publishes overlay frames into shared memory.
///
/// Dropping it releases the mapping. On Linux the backing file is deliberately
/// left in place: the bridge may still hold a mapping over it, and unlinking
/// underneath Wine would leave the game reading a file with no name.
pub struct OverlayWriter {
    inner: Inner,
    /// Incremented by two per publish, so it is even between writes.
    sequence: u32,
    name: String,
}

impl OverlayWriter {
    /// Open — creating if necessary — the shared block the overlay reads.
    pub fn open() -> io::Result<Self> {
        Self::open_named(OVERLAY_MMF_NAME)
    }

    /// Open a block under a specific name.
    ///
    /// Exists so tests get a block each — they all publish to the same struct
    /// and would otherwise race over one file — and so a second instance can
    /// be told to publish somewhere else rather than fight the first.
    pub fn open_named(name: &str) -> io::Result<Self> {
        Ok(Self {
            inner: Inner::open(name, size_of::<OverlayFrame>())?,
            sequence: 0,
            name: name.to_string(),
        })
    }

    /// Path of this writer's backing file, for diagnostics and tests.
    #[cfg(not(target_os = "windows"))]
    pub fn backing_path(&self) -> PathBuf {
        PathBuf::from(SHM_DIR).join(&self.name)
    }

    /// Publish a frame.
    ///
    /// The sequence field is managed here rather than by the caller: it goes
    /// odd before the body is written and even after, so a reader that sees an
    /// odd value, or a different value either side of its read, knows it
    /// caught a write in progress.
    ///
    /// The two stores are separated by fences. Without them nothing stops the
    /// compiler or the CPU moving the body across the sequence update, which
    /// would defeat the whole scheme — the reader would see the new sequence
    /// with the old body.
    pub fn publish(&mut self, frame: &OverlayFrame) {
        self.sequence = self.sequence.wrapping_add(1); // odd: write starting
        self.inner.write_sequence(self.sequence);
        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

        self.inner.write_body(frame);

        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
        self.sequence = self.sequence.wrapping_add(1); // even: write complete
        self.inner.write_sequence(self.sequence);
    }

    /// Mark the overlay as no longer being fed.
    ///
    /// Called on shutdown so the in-game overlay hides immediately rather than
    /// waiting for its liveness timeout and showing frozen numbers in between.
    pub fn publish_shutdown(&mut self) {
        let mut frame = OverlayFrame::empty();
        frame.sequence = 0;
        self.inner.write_body(&frame);
        self.inner.write_sequence(0);
        self.sequence = 0;
    }
}

// ---------------------------------------------------------------------------
// Linux: an ordinary file under /dev/shm that shm-bridge wraps for Wine.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
struct Inner {
    map: memmap2::MmapMut,
}

#[cfg(not(target_os = "windows"))]
impl Inner {
    fn open(name: &str, size: usize) -> io::Result<Self> {
        let path = PathBuf::from(SHM_DIR).join(name);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        // Sized before mapping. A file shorter than the struct would make
        // every write past its end a SIGBUS rather than an error.
        file.set_len(size as u64)?;

        // Safety: the file is sized to at least `size`, and this process is
        // the only writer. A concurrent reader seeing a partial write is what
        // the sequence lock exists for.
        let map = unsafe { memmap2::MmapMut::map_mut(&file)? };
        Ok(Self { map })
    }

    fn write_sequence(&mut self, sequence: u32) {
        // The sequence is the second u32 of the struct.
        let offset = size_of::<u32>();
        self.map[offset..offset + 4].copy_from_slice(&sequence.to_ne_bytes());
    }

    fn write_body(&mut self, frame: &OverlayFrame) {
        // Safety: OverlayFrame is repr(C) and contains no padding (asserted in
        // its own tests) and no pointers, so its bytes are meaningful on their
        // own.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (frame as *const OverlayFrame) as *const u8,
                size_of::<OverlayFrame>(),
            )
        };
        self.map[..bytes.len()].copy_from_slice(bytes);
    }
}

// ---------------------------------------------------------------------------
// Windows: a named file mapping the Lua app opens directly.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
struct Inner {
    handle: windows::Win32::Foundation::HANDLE,
    view: *mut u8,
    size: usize,
}

#[cfg(target_os = "windows")]
impl Inner {
    fn open(name: &str, size: usize) -> io::Result<Self> {
        use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows::Win32::System::Memory::{
            CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile, PAGE_READWRITE,
        };
        use windows::core::HSTRING;

        // `Local\` scopes the object to the session, which is where AC lives.
        let full_name = HSTRING::from(format!("Local\\{name}"));

        unsafe {
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                size as u32,
                &full_name,
            )
            .map_err(|e| io::Error::other(format!("CreateFileMapping failed: {e}")))?;

            let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size);
            if view.Value.is_null() {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                return Err(io::Error::other("MapViewOfFile returned null"));
            }

            Ok(Self {
                handle,
                view: view.Value as *mut u8,
                size,
            })
        }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // Safety: the view is `size` bytes and stays mapped until Drop.
        unsafe { std::slice::from_raw_parts_mut(self.view, self.size) }
    }

    fn write_sequence(&mut self, sequence: u32) {
        let offset = size_of::<u32>();
        self.as_mut_slice()[offset..offset + 4].copy_from_slice(&sequence.to_ne_bytes());
    }

    fn write_body(&mut self, frame: &OverlayFrame) {
        // Safety: as in the Linux implementation — repr(C), no padding, no
        // pointers.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (frame as *const OverlayFrame) as *const u8,
                size_of::<OverlayFrame>(),
            )
        };
        self.as_mut_slice()[..bytes.len()].copy_from_slice(bytes);
    }
}

#[cfg(target_os = "windows")]
impl Drop for Inner {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Memory::{MEMORY_MAPPED_VIEW_ADDRESS, UnmapViewOfFile};
        unsafe {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view as _,
            });
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for Inner {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::frame::OVERLAY_VERSION;

    /// Read a frame back out of the shared block the way the Lua app would.
    #[cfg(not(target_os = "windows"))]
    fn read_back(writer: &OverlayWriter) -> OverlayFrame {
        let bytes = std::fs::read(writer.backing_path()).expect("read shm");
        assert!(bytes.len() >= size_of::<OverlayFrame>());
        // Safety: written by `publish` from a value of this exact type.
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const OverlayFrame) }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_published_frame_reads_back_field_for_field() {
        let mut writer = OverlayWriter::open_named("acpe-test-roundtrip").expect("open shm");

        let mut frame = OverlayFrame::empty();
        frame.speed_kmh = 213.5;
        frame.rpm = 7400;
        frame.gear = 4;
        frame.tyre_pressure_psi = [27.1, 27.3, 26.8, 26.9];
        frame.fuel_laps_remaining = 12.5;

        writer.publish(&frame);

        let read = read_back(&writer);
        assert_eq!(read.version, OVERLAY_VERSION);
        assert_eq!(read.speed_kmh, 213.5);
        assert_eq!(read.rpm, 7400);
        assert_eq!(read.gear, 4);
        assert_eq!(read.tyre_pressure_psi, [27.1, 27.3, 26.8, 26.9]);
        assert_eq!(read.fuel_laps_remaining, 12.5);
    }

    /// The reader uses the parity of this field to tell a settled frame from
    /// one being written, so it must be even once `publish` returns.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_sequence_is_even_between_writes_and_advances_each_time() {
        let mut writer = OverlayWriter::open_named("acpe-test-sequence").expect("open shm");
        let frame = OverlayFrame::empty();

        writer.publish(&frame);
        let first = read_back(&writer).sequence;
        assert_eq!(first % 2, 0, "settled frames have an even sequence");

        writer.publish(&frame);
        let second = read_back(&writer).sequence;
        assert_eq!(second % 2, 0);
        assert_ne!(
            first, second,
            "the overlay uses this advancing to know we are alive"
        );
    }

    /// Shutdown has to be distinguishable from "running but idle", or the
    /// overlay keeps drawing the last numbers after the app has gone.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn shutdown_zeroes_the_sequence() {
        let mut writer = OverlayWriter::open_named("acpe-test-shutdown").expect("open shm");
        let mut frame = OverlayFrame::empty();
        frame.speed_kmh = 100.0;
        writer.publish(&frame);
        assert_ne!(read_back(&writer).sequence, 0);

        writer.publish_shutdown();
        assert_eq!(
            read_back(&writer).sequence,
            0,
            "zero means never written, so the overlay hides"
        );
    }

    /// The file has to be at least struct-sized before it is mapped, or a
    /// write past the end is a SIGBUS rather than an error.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_backing_file_is_sized_to_the_struct() {
        let writer = OverlayWriter::open_named("acpe-test-size").expect("open shm");
        let meta = std::fs::metadata(writer.backing_path()).expect("stat");
        assert!(meta.len() as usize >= size_of::<OverlayFrame>());
    }
}
