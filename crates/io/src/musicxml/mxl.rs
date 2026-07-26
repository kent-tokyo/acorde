use std::io::{Cursor, Read};
use zip::ZipArchive;
use acorde_core::Score;
use crate::Error;

const MAX_MXL_COMPRESSED: usize   = 32 * 1024 * 1024; // 32 MB
const MAX_MXL_DECOMPRESSED: u64   = 32 * 1024 * 1024; // 32 MB (zip-bomb guard)

pub fn parse_mxl(data: &[u8]) -> Result<Score, Error> {
    if data.len() > MAX_MXL_COMPRESSED {
        return Err(Error::TooLarge(data.len()));
    }
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| Error::Zip(format!("invalid MXL zip: {e}")))?;

    let xml = if let Some(path) = read_container_rootfile(&mut archive) {
        validate_zip_path(&path)?;
        read_entry(&mut archive, &path)?
    } else {
        find_score_entry(&mut archive)?
    };

    super::parser::parse_musicxml(&xml)
}

fn validate_zip_path(path: &str) -> Result<(), Error> {
    if path.starts_with('/') || path.starts_with("..") || path.contains("/../") || path.ends_with("/..") {
        return Err(Error::Zip(format!("invalid entry path: '{path}'")));
    }
    Ok(())
}

fn read_container_rootfile(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    let mut entry = archive.by_name("META-INF/container.xml").ok()?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf).ok()?;
    let tag = "rootfile full-path=\"";
    let start = buf.find(tag)? + tag.len();
    let end = buf[start..].find('"')? + start;
    Some(buf[start..end].to_string())
}

fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, Error> {
    let entry = archive
        .by_name(name)
        .map_err(|_| Error::Zip(format!("entry '{name}' not found")))?;
    let mut buf = String::new();
    entry.take(MAX_MXL_DECOMPRESSED)
        .read_to_string(&mut buf)
        .map_err(|e| Error::Zip(format!("failed to read '{name}': {e}")))?;
    Ok(buf)
}

fn find_score_entry(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<String, Error> {
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let e = archive.by_index(i).ok()?;
            let name_lc = e.name().to_ascii_lowercase();
            if (name_lc.ends_with(".xml") || name_lc.ends_with(".musicxml"))
                && !name_lc.starts_with("meta-inf")
            {
                Some(e.name().to_string())
            } else {
                None
            }
        })
        .collect();

    for name in names {
        if let Ok(xml) = read_entry(archive, &name) {
            if xml.contains("score-partwise") || xml.contains("score-timewise") {
                return Ok(xml);
            }
        }
    }
    Err(Error::Zip("no MusicXML score entry found in archive".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bytes_returns_err() {
        assert!(parse_mxl(&[]).is_err());
    }

    #[test]
    fn garbage_bytes_returns_err() {
        assert!(parse_mxl(b"not a zip file at all!!!").is_err());
    }
}
