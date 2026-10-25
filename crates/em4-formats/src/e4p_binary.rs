//! Read-only extraction of length-prefixed UTF-16LE strings from binary E4P.
//! This is a partial structural inspector, not a full prototype deserializer.
use std::{collections::BTreeSet,io::{self,ErrorKind}};
use serde::Serialize;
const MAX_SIZE:usize=16*1024*1024;
#[derive(Debug,Serialize)]
pub struct BinaryPrototype {
 pub bytes:usize,
 pub format:String,
 pub prototype_paths:Vec<String>,
 pub model_paths:Vec<String>,
 pub child_prototype_paths:Vec<String>,
 pub strings_found:usize,
 pub section_names:Vec<String>,
 pub command_names:Vec<String>,
}
fn invalid(msg:&str)->io::Error{io::Error::new(ErrorKind::InvalidData,msg)}
pub fn inspect(bytes:&[u8])->io::Result<BinaryPrototype>{
 if bytes.len()>MAX_SIZE{return Err(invalid("prototype exceeds 16 MiB limit"));}
 let mut strings=BTreeSet::new();
 let mut offset=0usize;
 while offset+12<=bytes.len(){
  let n=u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()) as usize;
  if !(4..=512).contains(&n){offset+=1;continue;}
  let end=match n.checked_mul(2).and_then(|v|v.checked_add(offset+4)){Some(v) if v<=bytes.len()=>v,_=>{offset+=1;continue}};
  let data=&bytes[offset+4..end];
  if data.chunks_exact(2).all(|c|c[1]==0 && (c[0].is_ascii_graphic()||c[0]==b' ')){
   let value:String=data.chunks_exact(2).map(|c|c[0] as char).collect();
   strings.insert(value);
   offset=end;
  }else{offset+=1;}
 }
 let mut models=BTreeSet::new();let mut children=BTreeSet::new();let mut prototypes=BTreeSet::new();
 for v in &strings {
  let p=v.replace('\\',"/");
  let lower=p.to_ascii_lowercase();
  if p.contains("..")||p.contains('\0')||p.len()>512{continue;}
  if lower.ends_with(".v3o"){models.insert(p);}
  else if lower.ends_with(".e4p"){
   if lower.contains("/asf.e4p"){prototypes.insert(p);}else{children.insert(p);}
  }
 }
 let sections=["Prototype","Children","PhysicsObject","RigidBody","Wheels","Doors","VehicleWheel","Door","ModelName","lights"];
 let commands=["MoveTo","LoadUp","Unload","GoHome"];
 Ok(BinaryPrototype{
  bytes:bytes.len(),format:"length_prefixed_utf16le_binary".into(),
  prototype_paths:prototypes.into_iter().collect(),
  model_paths:models.into_iter().collect(),child_prototype_paths:children.into_iter().collect(),
  strings_found:strings.len(),
  section_names:sections.iter().filter(|x|strings.contains(**x)).map(|s|(*s).into()).collect(),
  command_names:commands.iter().filter(|x|strings.contains(**x)).map(|s|(*s).into()).collect(),
 })
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn extracts_model_and_child(){
  let mut b=Vec::new();
  for s in ["ModelName","mod:Models/Vehicle/test.V3O","mod:Prototypes/Wheels/a.e4p"] {
   b.extend_from_slice(&(s.len() as u32).to_le_bytes());
   for c in s.encode_utf16(){b.extend_from_slice(&c.to_le_bytes());}
  }
  let r=inspect(&b).unwrap();
  assert_eq!(r.model_paths,vec!["mod:Models/Vehicle/test.V3O"]);
  assert_eq!(r.child_prototype_paths,vec!["mod:Prototypes/Wheels/a.e4p"]);
 }
 #[test]fn short_data(){assert_eq!(inspect(&[0,1,2]).unwrap().strings_found,0);}
}
