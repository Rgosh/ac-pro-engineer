//! A cursor over a little-endian byte stream.
//!
//! **Every read is checked.** A model file is somebody else's data — a mod, a
//! half-finished download, a format that changed — and a parser that indexes
//! into it and trusts the result is a parser that panics inside a driver's
//! telemetry program. Everything here returns an error instead, and the caller
//! reports the file as unreadable rather than the application going down.

/// What went wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not start with what a model starts with.
    NotAModel,
    /// A version this parser has not been checked against.
    Version(u32),
    /// The file claimed something longer than what is left of it.
    Truncated {
        at: usize,
        wanted: usize,
        left: usize,
    },
    /// A node type outside the three the format defines.
    UnknownNode { at: usize, kind: u32 },
    /// A name that is not text.
    NotText { at: usize },
}

impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAModel => write!(out, "not a kn5 model"),
            Error::Version(version) => {
                write!(out, "model version {version} has not been checked")
            }
            Error::Truncated { at, wanted, left } => write!(
                out,
                "the file wants {wanted} bytes at {at} and has {left} left"
            ),
            Error::UnknownNode { at, kind } => {
                write!(out, "node kind {kind} at {at} is not one of the three")
            }
            Error::NotText { at } => write!(out, "a name at {at} is not text"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub fn at(&self) -> usize {
        self.at
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(count).ok_or(Error::Truncated {
            at: self.at,
            wanted: count,
            left: 0,
        })?;
        if end > self.bytes.len() {
            return Err(Error::Truncated {
                at: self.at,
                wanted: count,
                left: self.bytes.len().saturating_sub(self.at),
            });
        }
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    /// Move past `count` bytes without reading them.
    ///
    /// Most of a model is vertex data this crate has no use for, and stepping
    /// over it is the difference between reading a 35 MB car in a moment and
    /// allocating it.
    pub fn skip(&mut self, count: usize) -> Result<()> {
        self.take(count).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn f32(&mut self) -> Result<f32> {
        let bytes = self.take(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// A length-prefixed name.
    pub fn text(&mut self) -> Result<String> {
        let at = self.at;
        let count = self.u32()? as usize;
        let bytes = self.take(count)?;
        let _ = at;
        // Lossy on purpose: a mod with a stray byte in a node name is a mod
        // with a stray byte in a node name, not a car this program refuses to
        // draw.
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}
