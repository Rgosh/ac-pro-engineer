//! The engineer's voices, carried inside the program.
//!
//! **Embedded, not downloaded.** A pack is the whole of one voice — every
//! line of [`super::lines::LINES`] as a short Ogg clip — and about 1.2 MB.
//! Both voices together are less than a single screenshot on the site, and
//! carried in the binary they work offline, on the first start, with nothing
//! to fetch or to keep in step with the program's version.
//!
//! The file is what `tools/voicebank/build.sh --pack core/data/radio` writes:
//! `PERADIO1`, a four-byte little-endian header length, a JSON header naming
//! each clip's offset, length and duration, and the clips one after another.
//! The voice bank only writes a pack from a build in which recognition
//! confirmed every clip says its line.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const MAGIC: &[u8; 8] = b"PERADIO1";

/// Which of the engineer's voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    #[default]
    Male,
    Female,
}

impl Voice {
    /// The pack built into the program for this voice.
    pub fn embedded(self) -> &'static [u8] {
        match self {
            Voice::Male => include_bytes!("../../data/radio/male-en.pack"),
            Voice::Female => include_bytes!("../../data/radio/female-en.pack"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Voice::Male => "male",
            Voice::Female => "female",
        }
    }
}

#[derive(Debug, Deserialize)]
struct Header {
    rate: u32,
    clips: HashMap<String, Entry>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Entry {
    at: usize,
    len: usize,
    seconds: f32,
}

/// One voice: where each clip is, borrowed from the pack's bytes.
#[derive(Debug)]
pub struct Pack {
    bytes: &'static [u8],
    body: usize,
    rate: u32,
    clips: HashMap<String, Entry>,
}

impl Pack {
    /// Read a pack's header. The clips stay where they are until asked for.
    pub fn parse(bytes: &'static [u8]) -> Result<Pack> {
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            bail!("not a voice pack");
        }
        let length = bytes
            .get(8..12)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
            .context("a voice pack with no header length")? as usize;
        let head = bytes
            .get(12..12 + length)
            .context("a voice pack shorter than its header")?;
        let header: Header = serde_json::from_slice(head).context("a voice pack's header")?;
        let body = 12 + length;
        for (key, entry) in &header.clips {
            if body + entry.at + entry.len > bytes.len() {
                bail!("clip {key} runs past the end of the pack");
            }
        }
        Ok(Pack {
            bytes,
            body,
            rate: header.rate,
            clips: header.clips,
        })
    }

    /// The voice built into the program.
    pub fn embedded(voice: Voice) -> Result<Pack> {
        Pack::parse(voice.embedded())
    }

    /// A clip's Ogg bytes.
    pub fn clip(&self, key: &str) -> Option<&'static [u8]> {
        let entry = self.clips.get(key)?;
        let start = self.body + entry.at;
        self.bytes.get(start..start + entry.len)
    }

    /// How long each clip lasts, for [`super::Radio::with_lengths`].
    pub fn lengths(&self) -> HashMap<String, f32> {
        self.clips
            .iter()
            .map(|(k, e)| (k.clone(), e.seconds))
            .collect()
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn has(&self, key: &str) -> bool {
        self.clips.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::radio::lines::{LINES, clip_key};

    /// **Every line the radio can ask for is in both voices.** A line added
    /// to the table and not yet spoken by the voice bank is a silence on the
    /// radio that nobody would notice until a driver did — so it fails here.
    #[test]
    fn both_voices_have_every_line() {
        for voice in [Voice::Male, Voice::Female] {
            let pack = Pack::embedded(voice).unwrap();
            let mut missing = Vec::new();
            for line in LINES {
                match line.values {
                    Some((low, high)) => {
                        for n in low..=high {
                            if !pack.has(&clip_key(line, Some(n))) {
                                missing.push(clip_key(line, Some(n)));
                            }
                        }
                    }
                    None => {
                        if !pack.has(line.id) {
                            missing.push(line.id.to_string());
                        }
                    }
                }
            }
            assert!(
                missing.is_empty(),
                "the {} voice has no clip for {missing:?} — run tools/voicebank/build.sh --pack core/data/radio",
                voice.name()
            );
        }
    }

    #[test]
    fn a_clip_is_an_ogg() {
        let pack = Pack::embedded(Voice::Female).unwrap();
        let clip = pack.clip("radio.check").unwrap();
        assert_eq!(clip.get(..4), Some(b"OggS".as_slice()));
        assert!(pack.lengths().get("radio.check").is_some_and(|s| *s > 0.5));
    }

    #[test]
    fn a_broken_pack_is_refused() {
        assert!(Pack::parse(b"not a pack at all").is_err());
        assert!(Pack::parse(b"PERADIO1\xff\xff\x00\x00{}").is_err());
    }
}
