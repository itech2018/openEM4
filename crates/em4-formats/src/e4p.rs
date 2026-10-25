//! Read-only bounded inspection of Emergency 4 prototype resources.
//! A structural research tool, not a complete E4P decoder.
use std::{fs,io::{self,ErrorKind},path::Path};
use serde::Serialize;
const MAX:usize=16*1024*1024;
#[derive(Debug,Serialize)]
pub struct PrototypeReport {
 pub size:usize, pub encoding:String, pub candidate_model_paths:Vec<String>,
 pub candidate_child_paths:Vec<String>, pub preview_lines:Vec<String>
}
fn err(msg:&str)->io::Error{io::Error::new(ErrorKind::InvalidData,msg)}
fn extensions(s:&str)->Vec<String> {
 let mut out=std::collections::BTreeSet::new();
 for part in s.split(|c:char|c=='"'||c=='\''||c=='<'||c=='>'||c=='='||c==';'||c.is_whitespace()){
  let p=part.trim_matches(|c:char|c==','||c=='('||c==')').replace('\\',"/");
  let l=p.to_ascii_lowercase();
  if (l.ends_with(".v3o")||l.ends_with(".e4p"))&&p.len()<=512&&!p.contains("..")&&!p.contains(':'){
   out.insert(p);
  }
 }
 out.into_iter().collect()
}
pub fn inspect(bytes:&[u8])->io::Result<PrototypeReport> {
 if bytes.len()>MAX{return Err(err("E4P file exceeds 16 MiB research limit"));}
 let mut encoding="binary_or_unknown";
 let mut unpacked:Option<Vec<u8>>=None;
 if bytes.len()>=20 && bytes[..4]==0x1337C0D3u32.to_le_bytes() {
  match crate::v3o_packed::unpack_resource(bytes) {
   Ok(content)=>{unpacked=Some(content);encoding="packed_zlib_text_candidate";},
   Err(e)=>{eprintln!("E4P appears packed, but V3O unpacker rejected it: {e}");}
  }
 }
 let bytes_to_read=unpacked.as_deref().unwrap_or(bytes);
 let mut text=String::new();
 if let Ok(s)=std::str::from_utf8(bytes_to_read){
  if !s.contains('\0')&&s.chars().filter(|c|c.is_control()&&!c.is_whitespace()).count()<16 {
   if unpacked.is_none(){encoding="utf8_text_candidate";} text=s.to_string();
  }
 }
 let candidates=extensions(&text);
 Ok(PrototypeReport{
  size:bytes.len(),encoding:encoding.to_owned(),
  candidate_model_paths:candidates.iter().filter(|p|p.to_ascii_lowercase().ends_with(".v3o")).cloned().collect(),
  candidate_child_paths:candidates.iter().filter(|p|p.to_ascii_lowercase().ends_with(".e4p")).cloned().collect(),
  preview_lines:text.lines().take(8).map(|l|l.chars().take(160).collect()).collect()
 })
}
pub fn inspect_path(path:&Path)->io::Result<PrototypeReport>{
 if fs::metadata(path)?.len()>MAX as u64{return Err(err("E4P file exceeds 16 MiB research limit"));}
 inspect(&fs::read(path)?)
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn reference_scan(){
  let x=inspect(b"Model \"Models/Vehicles/TEC/asf.V3O\"\nChild 'Prototypes/Vehicles/TEC/part.e4p'").unwrap();
  assert_eq!(x.candidate_model_paths.len(),1);assert_eq!(x.candidate_child_paths.len(),1);
 }
 #[test]fn binary_fallback(){let x=inspect(&[0,255,0,3]).unwrap();assert_eq!(x.encoding,"binary_or_unknown");}
}
