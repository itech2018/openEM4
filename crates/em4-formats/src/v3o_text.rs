//! Safe best-effort references from unpacked, textual V3O models.
//! Not a V3O binary decoder. No write operations on original assets.
use std::{collections::BTreeSet,fs,io,path::Path};

pub fn texture_references(bytes: &[u8]) -> io::Result<Vec<String>> {
    if bytes.len() > 16*1024*1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData,"V3O text too large"));
    }
    let s=std::str::from_utf8(bytes)
        .map_err(|_|io::Error::new(io::ErrorKind::InvalidData,"packed or non-UTF-8 V3O: unpack a COPY with the EM4 editor"))?;
    if s.contains('\0') {
        return Err(io::Error::new(io::ErrorKind::InvalidData,"binary V3O cannot be scanned as text"));
    }
    let mut found=BTreeSet::new();
    for line in s.lines() {
        // Some unpacked V3O files contain a filename on its own line,
        // others contain quoted or whitespace-delimited texture paths.
        for part in line.split(|c:char| c.is_whitespace() || c=='"' || c=='\'' || c==';') {
            let candidate=part.trim_matches(|c:char| c==',' || c=='(' || c==')');
            let lower=candidate.to_ascii_lowercase();
            if (lower.ends_with(".dds") || lower.ends_with(".tga") || lower.ends_with(".png") || lower.ends_with(".bmp"))
                && candidate.len()<=260
                && !candidate.starts_with('/')
                && !candidate.contains("..")
                && !candidate.contains(':')
                && !candidate.chars().any(|c| c.is_control())
            {
                found.insert(candidate.replace('\\',"/"));
            }
        }
    }
    Ok(found.into_iter().collect())
}
pub fn inspect_file(path:&Path)->io::Result<Vec<String>> {
    let meta=fs::metadata(path)?;
    if meta.len()>16*1024*1024 {return Err(io::Error::new(io::ErrorKind::InvalidData,"V3O text too large"));}
    texture_references(&crate::v3o_packed::unpack(&fs::read(path)?)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn references() {
        let s=b"TEXTURE vehicle.dds\n'Models\\Fire\\door.tga'\nvehicle.dds\n";
        assert_eq!(texture_references(s).unwrap(),vec!["Models/Fire/door.tga","vehicle.dds"]);
    }
    #[test] fn packed_rejected(){assert!(texture_references(&[0,0,3,9]).is_err());}
    #[test] fn traversal_ignored(){assert!(texture_references(b"../secret.dds C:\\evil.dds").unwrap().is_empty());}
}
