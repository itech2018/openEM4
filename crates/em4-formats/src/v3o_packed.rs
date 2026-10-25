//! Decompress packed Emergency 4 V3O assets in memory.
//! Wrapper verified against a user-supplied original V3O:
//! 0x1337C0D3 magic, version 1, decompressed byte length at offset 8,
//! reserved 8 bytes and zlib payload at offset 20.
use std::io::{self, Read};
use flate2::read::ZlibDecoder;
const MAGIC:u32=0x1337C0D3;
const MAX_UNPACKED:usize=64*1024*1024;
fn invalid(msg:&str)->io::Error { io::Error::new(io::ErrorKind::InvalidData,msg.to_string()) }
pub fn unpack(bytes:&[u8])->io::Result<Vec<u8>> {
 let data=unpack_resource(bytes)?;
 if !data.starts_with(b"[VNUM=") {return Err(invalid("decompressed V3O lacks version header"));}
 Ok(data)
}

/// Generic packed game resource decoder: does not assume a V3O payload.
pub fn unpack_resource(bytes:&[u8])->io::Result<Vec<u8>> {
 if bytes.starts_with(b"[VNUM=") {if bytes.len()>MAX_UNPACKED{return Err(invalid("V3O exceeds size limit"));}return Ok(bytes.to_vec());}
 if bytes.len()<22 {return Err(invalid("V3O header too short"));}
 let word=|i:usize|u32::from_le_bytes(bytes[i..i+4].try_into().unwrap());
 if word(0)!=MAGIC {return Err(invalid("unrecognized V3O packing signature"));}
 if word(4)!=1 {return Err(invalid("unsupported V3O pack wrapper version"));}
 let expected=word(8) as usize;
 if expected==0||expected>MAX_UNPACKED {return Err(invalid("invalid V3O decompressed size"));}
 if word(12)!=0 {return Err(invalid("unsupported V3O reserved header field"));}
 let mut data=Vec::with_capacity(expected.min(1024*1024));
 let mut reader=ZlibDecoder::new(&bytes[20..]).take(expected as u64+1);
 reader.read_to_end(&mut data)?;
 if data.len()!=expected {return Err(invalid("V3O uncompressed length mismatch"));}
 Ok(data)
}
#[cfg(test)]
mod tests {
 use super::*;
 use std::io::Write;
 #[test] fn packed_round_trip(){
  let data=b"[VNUM=1.43]\r\n[NUMVERTICES=0]\r\n";
  let mut enc=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());
  enc.write_all(data).unwrap();
  let mut file=Vec::new();file.extend_from_slice(&MAGIC.to_le_bytes());
  file.extend_from_slice(&1u32.to_le_bytes());file.extend_from_slice(&(data.len() as u32).to_le_bytes());
  file.extend_from_slice(&[0;8]);file.extend_from_slice(&enc.finish().unwrap());
  assert_eq!(unpack(&file).unwrap(),data);
 }
 #[test] fn rejects_wrong_size(){
  let mut b=vec![0;22];b[..4].copy_from_slice(&MAGIC.to_le_bytes());b[4..8].copy_from_slice(&1u32.to_le_bytes());b[8..12].copy_from_slice(&100u32.to_le_bytes());
  assert!(unpack(&b).is_err());
 }
}
