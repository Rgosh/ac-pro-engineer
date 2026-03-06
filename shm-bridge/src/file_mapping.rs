// Copyright (c) 2024 Damir Jelić
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{fs::File, os::windows::prelude::AsRawHandle};

use anyhow::{Context, Result, anyhow};
use windows::{
    Wdk::System::SystemServices::PAGE_READWRITE,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Memory::{CreateFileMappingW, PAGE_PROTECTION_FLAGS},
    },
    core::HSTRING,
};

/// File-backed named shared memory[1].
///
/// This will create named shared memory backed by a file.
///
/// The shared memory will be kept alive as long as the [`FileMapping`] object
/// is allive, dropping it will free the shared memory.
///
/// The [`FileMapping`] uses the Windows [`CreateFileMappingW`] function
/// underneath.
///
/// [1]: https://learn.microsoft.com/en-us/windows/win32/memory/creating-named-shared-memory
pub struct FileMapping {
    handle: HANDLE,
}

impl FileMapping {
    /// Create a new [`FileMapping`], the given file will be used as the backing
    /// storage.
    ///
    /// # Arguments
    ///
    /// * `name` - The name the [`FileMapping`] should have, other Windows
    ///   applications can open the shared memory using this name.
    ///
    /// * `file` - The file that should be used as the backing storage of the
    ///   [`FileMapping`]. The file will be resized to have the correct length.
    ///
    /// * `size` - The desiered size the [`FileMapping`] should have, i.e. the
    ///   number of bytes the [`FileMapping`] should have.
    pub fn new(name: &str, file: &File, size: usize) -> Result<Self> {
        // Ensure the file is of the correct size.
        file.set_len(size as u64)
            .context("Couldn't set the file size of the FileMapping.")?;

        let high_size: u32 = ((size as u64 & 0xFFFF_FFFF_0000_0000_u64) >> 32) as u32;
        let low_size: u32 = (size as u64 & 0xFFFF_FFFF_u64) as u32;

        // Windows uses UTF-16, so we need to convert the UTF-8 based Rust string
        // accordingly.
        let name = HSTRING::from(name);
        let handle = HANDLE(file.as_raw_handle() as _);

        let handle = unsafe {
            CreateFileMappingW(
                handle,
                None,
                PAGE_PROTECTION_FLAGS(PAGE_READWRITE),
                high_size,
                low_size,
                &name,
            )
        };

        match handle {
            Ok(handle) => Ok(FileMapping { handle }),
            Err(e) => Err(anyhow!("Failed to create the FileMapping: {e}")),
        }
    }
}

impl Drop for FileMapping {
    fn drop(&mut self) {
        // There's not much we can do if an error happens here, so let's ignore it.
        let _unused = unsafe { CloseHandle(self.handle) };
    }
}
