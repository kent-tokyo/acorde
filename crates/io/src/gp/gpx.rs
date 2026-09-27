//! Guitar Pro 6 (`.gpx`) container: an optional "BCFZ" bit-level LZ compression around a
//! "BCFS" sector file system (4 KiB sectors) whose `score.gpif` entry holds GPIF XML.

use crate::Error;

const SECTOR: usize = 0x1000;
/// Bound on the decompressed container, matching the GPIF entry limit of `.gp` files.
const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte: 0,
            bit: 0,
        }
    }

    fn bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.byte)?;
        let value = (byte >> (7 - self.bit)) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.byte += 1;
        }
        Some(u32::from(value))
    }

    /// `count` bits, most significant first.
    fn bits(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, _| Some((value << 1) | self.bit()?))
    }

    /// `count` bits, least significant first.
    fn bits_reversed(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, index| Some(value | (self.bit()? << index)))
    }

    fn le_u32(&mut self) -> Option<u32> {
        let bytes = [
            self.bits(8)? as u8,
            self.bits(8)? as u8,
            self.bits(8)? as u8,
            self.bits(8)? as u8,
        ];
        Some(u32::from_le_bytes(bytes))
    }
}

fn le_u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Undo BCFZ compression: a stream of literal runs and back-references into the output.
fn decompress(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut reader = BitReader::new(data);
    let expected = reader
        .le_u32()
        .ok_or_else(|| Error::Xml("truncated Guitar Pro 6 header".into()))?
        as usize;
    if expected > MAX_DECOMPRESSED {
        return Err(Error::TooLarge(expected));
    }
    let mut output: Vec<u8> = Vec::with_capacity(expected.min(16 * 1024 * 1024));
    while output.len() < expected {
        let Some(flag) = reader.bit() else {
            break;
        };
        if flag == 1 {
            let Some(word) = reader.bits(4) else {
                break;
            };
            let (Some(offset), Some(size)) =
                (reader.bits_reversed(word), reader.bits_reversed(word))
            else {
                break;
            };
            let (offset, size) = (offset as usize, size as usize);
            let start = output
                .len()
                .checked_sub(offset)
                .filter(|_| offset > 0)
                .ok_or_else(|| Error::Xml("invalid Guitar Pro 6 back-reference".into()))?;
            let count = offset.min(size);
            let copy = output[start..start + count].to_vec();
            output.extend_from_slice(&copy);
        } else {
            let Some(size) = reader.bits_reversed(2) else {
                break;
            };
            for _ in 0..size {
                let Some(byte) = reader.bits(8) else {
                    break;
                };
                output.push(byte as u8);
            }
        }
        if output.len() > MAX_DECOMPRESSED {
            return Err(Error::TooLarge(output.len()));
        }
    }
    Ok(output)
}

/// Find `score.gpif` in a BCFS sector file system.
fn read_file_system(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut offset = SECTOR;
    while offset + 3 < data.len() {
        if le_u32_at(data, offset) == Some(2) {
            let name_bytes = data
                .get(offset + 0x04..offset + 0x04 + 127)
                .unwrap_or_default();
            let name_end = name_bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(name_bytes.len());
            let name = String::from_utf8_lossy(&name_bytes[..name_end]);
            let size = le_u32_at(data, offset + 0x8c).unwrap_or(0) as usize;
            if name == "score.gpif" {
                if size > MAX_DECOMPRESSED {
                    return Err(Error::TooLarge(size));
                }
                let mut file = Vec::with_capacity(size);
                let pointers = offset + 0x94;
                for index in 0.. {
                    let Some(sector) = le_u32_at(data, pointers + 4 * index) else {
                        break;
                    };
                    if sector == 0 || file.len() >= size {
                        break;
                    }
                    let start = (sector as usize)
                        .checked_mul(SECTOR)
                        .filter(|start| *start < data.len())
                        .ok_or_else(|| Error::Xml("invalid Guitar Pro 6 sector".into()))?;
                    let end = (start + SECTOR).min(data.len());
                    file.extend_from_slice(&data[start..end]);
                }
                file.truncate(size);
                return Ok(file);
            }
        }
        offset += SECTOR;
    }
    Err(Error::Xml(
        "Guitar Pro 6 file has no score.gpif entry".into(),
    ))
}

/// Extract the GPIF XML from a `.gpx` file.
pub(super) fn read_gpif(data: &[u8]) -> Result<String, Error> {
    if data.len() > MAX_DECOMPRESSED {
        return Err(Error::TooLarge(data.len()));
    }
    let file_system = match data.get(..4) {
        Some(b"BCFZ") => {
            let decompressed = decompress(&data[4..])?;
            // The decompressed stream starts with its own "BCFS" header.
            if !decompressed.starts_with(b"BCFS") {
                return Err(Error::Xml("corrupt Guitar Pro 6 container".into()));
            }
            decompressed[4..].to_vec()
        }
        Some(b"BCFS") => data[4..].to_vec(),
        _ => return Err(Error::Xml("not a Guitar Pro 6 file".into())),
    };
    let gpif = read_file_system(&file_system)?;
    crate::decode_xml_text(&gpif)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode `bytes` as BCFZ literal runs (three bytes per run), the simplest valid stream.
    fn bcfz_literals(bytes: &[u8]) -> Vec<u8> {
        let mut bits: Vec<u8> = Vec::new();
        let push_bits = |bits: &mut Vec<u8>, value: u32, count: u32, reversed: bool| {
            for index in 0..count {
                let shift = if reversed { index } else { count - 1 - index };
                bits.push(((value >> shift) & 1) as u8);
            }
        };
        for chunk in bytes.chunks(3) {
            push_bits(&mut bits, 0, 1, false);
            push_bits(&mut bits, chunk.len() as u32, 2, true);
            for byte in chunk {
                push_bits(&mut bits, u32::from(*byte), 8, false);
            }
        }
        let mut out = b"BCFZ".to_vec();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (index, bit) in chunk.iter().enumerate() {
                byte |= bit << (7 - index);
            }
            out.push(byte);
        }
        out
    }

    fn file_system(gpif: &[u8]) -> Vec<u8> {
        // Sector 0: header area; sector 1: file entry; sector 2..: data.
        let mut fs = vec![0u8; SECTOR * 2];
        fs[SECTOR..SECTOR + 4].copy_from_slice(&2u32.to_le_bytes());
        fs[SECTOR + 4..SECTOR + 4 + 10].copy_from_slice(b"score.gpif");
        fs[SECTOR + 0x8c..SECTOR + 0x90].copy_from_slice(&(gpif.len() as u32).to_le_bytes());
        let sectors = gpif.len().div_ceil(SECTOR);
        for index in 0..sectors {
            let pointer = SECTOR + 0x94 + 4 * index;
            fs[pointer..pointer + 4].copy_from_slice(&((2 + index) as u32).to_le_bytes());
        }
        let mut data = gpif.to_vec();
        data.resize(sectors * SECTOR, 0);
        fs.extend_from_slice(&data);
        fs
    }

    #[test]
    fn bcfz_and_bcfs_containers_yield_the_gpif() {
        let gpif = "<GPIF><Score><Title>Six</Title></Score></GPIF>".repeat(200);
        let mut plain = b"BCFS".to_vec();
        plain.extend_from_slice(&file_system(gpif.as_bytes()));
        assert_eq!(read_gpif(&plain).unwrap(), gpif);
        let mut inner = b"BCFS".to_vec();
        inner.extend_from_slice(&file_system(gpif.as_bytes()));
        assert_eq!(read_gpif(&bcfz_literals(&inner)).unwrap(), gpif);
    }

    #[test]
    fn corrupt_containers_are_rejected_without_panicking() {
        assert!(read_gpif(b"BCFZ").is_err());
        assert!(read_gpif(b"BCFZ\xff\xff\xff\x7f").is_err());
        // A back-reference before the start of the output.
        assert!(read_gpif(b"BCFZ\x10\x00\x00\x00\xff\xff\xff\xff").is_err());
        assert!(read_gpif(b"BCFS\x00\x00").is_err());
    }
}
