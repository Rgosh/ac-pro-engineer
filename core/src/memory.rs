use anyhow::anyhow;
use memmap2::Mmap;
use std::fmt::Debug;
use std::fs::File;
use std::marker::PhantomData;
use zerocopy::{IntoBytes, TryFromBytes};

pub struct SharedMemory<T> {
    mmap: Mmap,
    _phantom: PhantomData<T>,
}

unsafe impl<T> Send for SharedMemory<T> {}
unsafe impl<T> Sync for SharedMemory<T> {}

#[allow(unsafe_code)]
impl<T> SharedMemory<T> {
    pub fn connect(name: &str) -> Result<Self, Box<dyn std::error::Error>>
    where
        T: Debug,
    {
        use memmap2::Mmap;

        let file = File::open(name)?;

        let mmap = unsafe { Mmap::map(&file) };

        let Ok(mmap) = mmap else {
            return Err(format!("Cannot map a memory file file: {}", name).into());
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
        let size = size_of::<T>();
        let bytes = &self.mmap;
        if bytes.len() < size {
            return Err(anyhow!("Incorrect buffer size").into());
        }
        let bytes = bytes[..size].as_bytes();
        T::try_read_from_bytes(bytes)
            .map_err(|err| anyhow::format_err!("Error converting type: {err:?}").into())
    }
}
