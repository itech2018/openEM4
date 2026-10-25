//! Non-mutating, bounded inspection of legacy Emergency 4 assets.
//! This module does NOT yet decode packed V3O geometry or E4P prototypes.

pub mod dds;
pub mod v3o_text;
pub mod v3o_packed;
pub mod v3o_mesh;
use serde::Serialize;
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Inspection {
    Dds { width: u32, height: u32, mipmaps: u32, fourcc: String },
    V3o { representation: String, version_hint: Option<String> },
    E4p { representation: String, bytes: usize },
    E4m { representation: String, bytes: usize },
    Unknown { extension: String, bytes: usize },
}

pub fn inspect(path: &Path) -> io::Result<Inspection> {
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > 256 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "asset exceeds 256 MiB inspection limit"));
    }
    let mut data = Vec::new();
    use io::Read;
    file.read_to_end(&mut data)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    inspect_bytes(&ext, &data)
}
pub fn inspect_bytes(extension: &str, bytes: &[u8]) -> io::Result<Inspection> {
    match extension.to_ascii_lowercase().as_str() {
        "dds" => inspect_dds(bytes),
        "v3o" => {
            let unpacked = v3o_packed::unpack(bytes)?;
            let text = std::str::from_utf8(&unpacked).ok();
            let printable = text.map(|s| !s.contains('\0') && s.chars().filter(|c| c.is_control() && !c.is_whitespace()).count() < 8).unwrap_or(false);
            let version_hint = text.and_then(|s| s.lines().take(12).find(|l| l.contains("VNUM"))).map(|s| s.trim().chars().take(120).collect());
            Ok(Inspection::V3o { representation: if bytes.starts_with(b"[VNUM=") { "text_v3o" } else if printable { "packed_v3o_zlib" } else { "unrecognized_text" }.into(), version_hint })
        }
        "e4p" => Ok(Inspection::E4p { representation: "opaque_unparsed".into(), bytes: bytes.len() }),
        "e4m" => Ok(Inspection::E4m { representation: "opaque_unparsed".into(), bytes: bytes.len() }),
        _ => Ok(Inspection::Unknown { extension: extension.to_ascii_lowercase(), bytes: bytes.len() }),
    }
}
fn inspect_dds(bytes: &[u8]) -> io::Result<Inspection> {
    if bytes.len() < 128 || &bytes[..4] != b"DDS " || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 124 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid or truncated DDS header"));
    }
    let height = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let mipmaps = u32::from_le_bytes(bytes[28..32].try_into().unwrap()).max(1);
    if width == 0 || height == 0 || width > 32768 || height > 32768 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid DDS dimensions"));
    }
    let fourcc = String::from_utf8_lossy(&bytes[84..88]).trim_matches('\0').to_string();
    Ok(Inspection::Dds { width, height, mipmaps, fourcc })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn dds_header() {
        let mut b = vec![0; 128];
        b[..4].copy_from_slice(b"DDS ");
        b[4..8].copy_from_slice(&124u32.to_le_bytes());
        b[12..16].copy_from_slice(&256u32.to_le_bytes());
        b[16..20].copy_from_slice(&512u32.to_le_bytes());
        b[84..88].copy_from_slice(b"DXT5");
        assert_eq!(inspect_bytes("dds", &b).unwrap(), Inspection::Dds { width: 512, height: 256, mipmaps: 1, fourcc: "DXT5".into() });
    }
    #[test] fn malformed_dds() { assert!(inspect_bytes("dds", b"DDS ").is_err()); }
    #[test] fn opaque_v3o() {
        assert_eq!(inspect_bytes("v3o", &[0,1,2]).unwrap(), Inspection::V3o { representation: "binary_or_unknown".into(), version_hint: None });
    }
}
