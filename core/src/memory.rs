use memmap2::Mmap;
use std::fmt::Debug;
use std::fs::File;
use std::marker::PhantomData;
use tracing::info;

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

    pub fn get(&self) -> &T
    where
        T: Debug,
    {
        info!(
            "Required size: {}, actual size: {}",
            size_of::<T>(),
            self.mmap.len()
        );
        assert!(self.mmap.len() >= size_of::<T>(), "Invalid size");

        let ptr = self.mmap.as_ptr();

        // Alignment check
        assert_eq!(
            (ptr as usize) % align_of::<T>(),
            0,
            "mmap not aligned for T"
        );

        unsafe { &*(ptr as *const T) }
    }
}
