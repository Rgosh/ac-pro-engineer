use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, FILE_MAP_READ, UnmapViewOfFile};
use std::ffi::c_void;

pub struct SharedMemory<T> {
    handle: HANDLE,
    ptr: *const T,
}

impl<T> SharedMemory<T> {
    pub fn connect(name: &str) -> Option<Self> {
        unsafe {
            let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = OpenFileMappingW(
                FILE_MAP_READ.0,
                false,
                PCWSTR(wide_name.as_ptr()),
            ).ok()?;

            if handle.is_invalid() { return None; }

            let ptr = MapViewOfFile(
                handle,
                FILE_MAP_READ,
                0,
                0,
                std::mem::size_of::<T>(),
            ).Value as *const T;

            if ptr.is_null() {
                CloseHandle(handle);
                return None;
            }
            Some(Self { handle, ptr })
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> Drop for SharedMemory<T> {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                let address = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut c_void,
                };
                let _ = UnmapViewOfFile(address);
            }
            let _ = CloseHandle(self.handle);
        }
    }
}