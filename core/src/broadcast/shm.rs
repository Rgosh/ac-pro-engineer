//! The shared-memory mapping the in-game panel reads, as a sink.
//!
//! No behaviour change: this is the writer the application has always used,
//! wearing the [`Sink`] interface so it sits in the same list as everything
//! else. Worth doing precisely because it is the odd one out — if the abstraction
//! could not hold the transport the whole project is built around, it would be
//! the wrong abstraction.
//!
//! It keeps its every-tick rate. A mapping is a memcpy and the panel reads
//! whatever is there when it draws, so there is nothing to gain by sending less
//! often and a stale-looking overlay to lose.

use super::Sink;
use crate::overlay::frame::OverlayFrame;
use crate::overlay::shared_writer::OverlayWriter;

pub struct SharedMemorySink {
    writer: OverlayWriter,
}

impl SharedMemorySink {
    pub fn open() -> std::io::Result<Self> {
        OverlayWriter::open()
            .map(|writer| Self { writer })
            .map_err(std::io::Error::other)
    }

    pub fn from_writer(writer: OverlayWriter) -> Self {
        Self { writer }
    }
}

impl Sink for SharedMemorySink {
    fn name(&self) -> &str {
        "shared memory (in-game panel)"
    }

    fn publish(&mut self, frame: &OverlayFrame) -> std::io::Result<()> {
        self.writer.publish(frame);
        Ok(())
    }

    fn shutdown(&mut self) {
        // So the panel hides at once instead of waiting out its liveness
        // timeout and holding the last frame on screen for two seconds.
        self.writer.publish_shutdown();
    }
}
