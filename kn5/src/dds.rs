//! DDS pictures as a model carries them: how big each level is, and the
//! same picture smaller.
//!
//! **Smaller by dropping levels, never by encoding again.** A texture with
//! mipmaps already holds itself at half the size, and a quarter, behind its
//! largest level; a level of detail seen from thirty metres needs nothing
//! bigger. Taking the first levels off keeps the format exactly — BC5
//! normal maps, BC1 with its one-bit alpha, the luminance pictures some
//! cars use — where decoding and compressing again would change what a
//! shader reads.

/// A DDS file's head, as far as sizes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    /// Where the pictures begin: 128, or 148 after a DX10 header.
    pub data: usize,
    /// Bytes in a block of four by four, for a block format; else zero and
    /// `pixel` is set.
    block: u64,
    /// Bytes in a pixel, for an uncompressed format.
    pixel: u64,
}

impl Head {
    /// Bytes in one level of the given side.
    pub fn level_bytes(&self, level: u32) -> u64 {
        let w = u64::from((self.width >> level).max(1));
        let h = u64::from((self.height >> level).max(1));
        match self.block {
            0 => w * h * self.pixel,
            block => w.div_ceil(4) * h.div_ceil(4) * block,
        }
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes)
}

/// The head of a DDS file in a format whose sizes are known — `None` for
/// anything else, which is then left as it is.
pub fn head(bytes: &[u8]) -> Option<Head> {
    if bytes.get(..4) != Some(b"DDS ".as_slice()) || word(bytes, 4)? != 124 {
        return None;
    }
    let height = word(bytes, 12)?;
    let width = word(bytes, 16)?;
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return None;
    }
    let levels = word(bytes, 28).unwrap_or(0).max(1);
    let flags = word(bytes, 80)?;
    let four = bytes.get(84..88)?;
    let bits = word(bytes, 88)?;
    let (data, block, pixel) = match four {
        b"DXT1" | b"ATI1" | b"BC4U" => (128, 8, 0),
        b"DXT2" | b"DXT3" | b"DXT4" | b"DXT5" | b"ATI2" | b"BC5U" => (128, 16, 0),
        b"DX10" => {
            // DXGI formats by number: the block ones a car carries.
            let block = match word(bytes, 128)? {
                70..=72 | 79..=81 => 8,
                73..=78 | 82..=84 | 94..=99 => 16,
                _ => return None,
            };
            (148, block, 0)
        }
        _ if flags & 0x40 != 0 && bits % 8 == 0 && bits > 0 => (128, 0, u64::from(bits / 8)),
        _ if flags & 0x2_0000 != 0 && bits % 8 == 0 && bits > 0 => (128, 0, u64::from(bits / 8)),
        _ => return None,
    };
    Some(Head {
        width,
        height,
        levels,
        data,
        block,
        pixel,
    })
}

/// The picture with its `drop` largest levels taken off, while at least
/// one level and a side of 32 remain — `None` when there is nothing to take
/// (no mipmaps, an unknown format, a picture too small already, or a file
/// shorter than its head says).
pub fn smaller(bytes: &[u8], drop: u32) -> Option<Vec<u8>> {
    let head = head(bytes)?;
    let mut drop = drop.min(head.levels.saturating_sub(1));
    while drop > 0 && (head.width >> drop).min(head.height >> drop) < 32 {
        drop -= 1;
    }
    if drop == 0 {
        return None;
    }
    let skipped: u64 = (0..drop).map(|l| head.level_bytes(l)).sum();
    let kept: u64 = (drop..head.levels).map(|l| head.level_bytes(l)).sum();
    let from = head.data + usize::try_from(skipped).ok()?;
    let to = from + usize::try_from(kept).ok()?;
    let rest = bytes.get(from..to)?;
    let mut out = bytes.get(..head.data)?.to_vec();
    let mut set = |at: usize, value: u32| {
        if let Some(slot) = out.get_mut(at..at + 4) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
    };
    set(12, (head.height >> drop).max(1));
    set(16, (head.width >> drop).max(1));
    set(28, head.levels - drop);
    // The pitch or linear size of the first level, where the file gives it.
    let first = Head {
        width: (head.width >> drop).max(1),
        height: (head.height >> drop).max(1),
        ..head
    };
    let pitch = match head.block {
        0 => u64::from(first.width) * head.pixel,
        _ => first.level_bytes(0),
    };
    set(20, u32::try_from(pitch).unwrap_or(0));
    out.extend_from_slice(rest);
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A DXT1 picture of `side` with its full chain, each level filled with
    /// its own number so the levels can be told apart.
    pub(crate) fn dxt1(side: u32) -> Vec<u8> {
        let levels = 32 - side.leading_zeros();
        let mut out = b"DDS ".to_vec();
        let mut words = vec![0u32; 31];
        words[0] = 124;
        words[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x2_0000;
        words[2] = side;
        words[3] = side;
        words[6] = levels;
        words[18] = 32;
        words[19] = 0x4;
        for w in words {
            out.extend(w.to_le_bytes());
        }
        out[84..88].copy_from_slice(b"DXT1");
        for level in 0..levels {
            let s = u64::from((side >> level).max(1));
            let n = s.div_ceil(4) * s.div_ceil(4) * 8;
            out.extend(std::iter::repeat_n(level as u8, n as usize));
        }
        out
    }

    #[test]
    fn a_picture_loses_its_largest_levels_and_keeps_the_rest() {
        let big = dxt1(256);
        let head_before = head(&big).unwrap();
        assert_eq!(head_before.levels, 9);
        let half = smaller(&big, 1).unwrap();
        let h = head(&half).unwrap();
        assert_eq!((h.width, h.height, h.levels), (128, 128, 8));
        // The first level now is what was the second.
        assert_eq!(half[128], 1);
        assert_eq!(
            half.len() as u64,
            128 + (0..8).map(|l| h.level_bytes(l)).sum::<u64>()
        );
        // And it reads as a sound DDS to the check before every save.
        assert!(crate::lint::dds_problem(&half).is_none());
    }

    #[test]
    fn a_small_picture_or_one_with_no_levels_is_left() {
        assert!(smaller(&dxt1(32), 1).is_none());
        let mut one = dxt1(256);
        one[28..32].copy_from_slice(&1u32.to_le_bytes());
        assert!(smaller(&one, 1).is_none());
        assert!(smaller(b"not a picture", 1).is_none());
        // Never below a side of 32, however much is asked.
        let h = head(&smaller(&dxt1(128), 5).unwrap()).unwrap();
        assert_eq!(h.width, 32);
    }
}
