//! Narrow, read-only extractor for observed Emergency 4 E4P child attachment records.
//! Does not claim complete E4P format coverage.
use serde::Serialize;
use std::{fs,io::{self,ErrorKind},path::Path};
#[derive(Debug,Clone,Serialize)]
pub struct Attachment {
 pub prototype:String,pub position:[f32;3],pub rotation:[f32;9],pub category:String,pub door_axis:Option<[f32;3]>,pub open_angle:Option<f32>
}
#[derive(Debug,Serialize)]
pub struct Assembly {
 pub body_model:Option<String>,pub attachments:Vec<Attachment>,pub warnings:Vec<String>
}
fn invalid(s:&str)->io::Error {io::Error::new(ErrorKind::InvalidData,s)}
fn u32le(bytes:&[u8],offset:usize)->Option<u32>{
 Some(u32::from_le_bytes(bytes.get(offset..offset+4)?.try_into().ok()?))
}
fn word(bytes:&[u8],at:usize)->Option<(String,usize)> {
 let count=u32le(bytes,at)? as usize;
 if count==0||count>512{return None;}
 let end=at.checked_add(4+count*2)?;
 let data=bytes.get(at+4..end)?;
 let chars:Vec<u16>=data.chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect();
 let text=String::from_utf16(&chars).ok()?;
 if text.chars().any(|c|c.is_control()) {return None;}
 Some((text,end))
}
fn value_string(bytes:&[u8],end:usize)->Option<String>{
 if u32le(bytes,end)?!=4{return None;}
 word(bytes,end+4).map(|(s,_)|s)
}
fn value_float(bytes:&[u8],end:usize)->Option<f32>{
 if u32le(bytes,end)?!=2{return None;}
 let f=f32::from_bits(u32le(bytes,end+4)?);
 if f.is_finite(){Some(f)}else{None}
}
fn scan(bytes:&[u8])->Vec<(usize,String,usize)>{
 let mut result=Vec::new();let mut at=0;
 while at+8<=bytes.len(){
  if let Some((name,end))=word(bytes,at){
   // only recognize known keys, so other bytes are not interpreted as values
   if matches!(name.as_str(),"ModelName"|"Filename"|"PosX"|"PosY"|"PosZ"|"Rotation"|"Child"|"VehicleWheel"|"Doors"|"Wheels"|"Door"|"AxisX"|"AxisY"|"AxisZ"|"OpenAngle"){
    result.push((at,name,end));at=end;continue;
   }
  }
  at+=1;
 }
 result
}
pub fn parse(bytes:&[u8])->io::Result<Assembly>{
 if bytes.len()>16*1024*1024{return Err(invalid("E4P exceeds 16 MiB"));}
 let tokens=scan(bytes);
 let body_model=tokens.iter().filter(|(_,k,_)|k=="ModelName")
  .filter_map(|(_,_,end)|value_string(bytes,*end))
  .find(|p|p.to_ascii_lowercase().ends_with(".v3o"));
 let mut attachments=Vec::new();
 for (i,(_,key,end)) in tokens.iter().enumerate(){
  if key!="Filename"{continue;}
  let Some(filename)=value_string(bytes,*end) else{continue};
  if !filename.to_ascii_lowercase().ends_with(".e4p"){continue;}
  // Attachment position lives in the fields immediately following Filename.
  let mut pos=[None;3];let mut rotation=None;
  let stop=(i+8).min(tokens.len());
  for (_,k,e) in tokens.iter().take(stop).skip(i+1){
   if matches!(k.as_str(),"Filename"|"Child"|"ModelName"){break;}
   match k.as_str(){
    "PosX"=>pos[0]=value_float(bytes,*e),
    "PosY"=>pos[1]=value_float(bytes,*e),
    "PosZ"=>pos[2]=value_float(bytes,*e),
    "Rotation"=>{
     if let Some(s)=value_string(bytes,*e){
      let a:Vec<f32>=s.split_whitespace().filter_map(|p|p.parse::<f32>().ok()).collect();
      if a.len()==9&&a.iter().all(|f|f.is_finite()) {rotation=Some(a.try_into().unwrap());}
     }
    },_=>{}
   }
  }
  if pos.iter().any(|p|p.is_none()) {continue;}
  let category=if filename.to_ascii_lowercase().contains("wheel"){"wheel"}
   else if filename.to_ascii_lowercase().contains("door"){"door"}else{"child"};
  let mut door_axis=None;let mut open_angle=None;
  if category=="door" {
   if let Some(j)=(0..i).rev().find(|&j|tokens[j].1=="Door") {
    let intervening=tokens[j+1..i].iter().any(|(_,key,_)|
      matches!(key.as_str(),"Filename"|"VehicleWheel"|"Wheels"|"Child"|"ModelName"));
    if !intervening && tokens[i].0.saturating_sub(tokens[j].0)<2048 {
     let mut axis=[None;3];
     for (_,key,end) in &tokens[j+1..i] {
      match key.as_str(){
       "AxisX"=>axis[0]=value_float(bytes,*end),
       "AxisY"=>axis[1]=value_float(bytes,*end),
       "AxisZ"=>axis[2]=value_float(bytes,*end),
       "OpenAngle"=>open_angle=value_float(bytes,*end).filter(|v|v.abs()<=360.0),
       _=>{}
      }
     }
     if let [Some(x),Some(y),Some(z)]=axis {
      let len=(x*x+y*y+z*z).sqrt();
      if len>1e-5&&len.is_finite(){door_axis=Some([x/len,y/len,z/len]);}
     }
    }
   }
  }
  attachments.push(Attachment{prototype:filename,
   position:[pos[0].unwrap(),pos[1].unwrap(),pos[2].unwrap()],
   rotation:rotation.unwrap_or([1.,0.,0.,0.,1.,0.,0.,0.,1.]),
   category:category.to_string(),door_axis,open_angle});
 }
 let mut warnings=Vec::new();
 if body_model.is_none(){warnings.push("Could not locate ModelName V3O reference".into());}
 if attachments.is_empty(){warnings.push("No positioned child attachments decoded".into());}
 Ok(Assembly{body_model,attachments,warnings})
}
pub fn parse_file(path:&Path)->io::Result<Assembly>{
 if fs::metadata(path)?.len()>16*1024*1024{return Err(invalid("E4P exceeds 16 MiB"));}
 parse(&fs::read(path)?)
}
#[cfg(test)]
mod tests{
 use super::*;
 fn strfield(key:&str,val:&str,b:&mut Vec<u8>){
  for s in [key] {b.extend_from_slice(&(s.len() as u32).to_le_bytes());for c in s.encode_utf16(){b.extend_from_slice(&c.to_le_bytes());}}
  b.extend_from_slice(&4u32.to_le_bytes());
  b.extend_from_slice(&(val.len() as u32).to_le_bytes());
  for c in val.encode_utf16(){b.extend_from_slice(&c.to_le_bytes());}
 }
 fn floatfield(key:&str,val:f32,b:&mut Vec<u8>){
  b.extend_from_slice(&(key.len() as u32).to_le_bytes());for c in key.encode_utf16(){b.extend_from_slice(&c.to_le_bytes());}
  b.extend_from_slice(&2u32.to_le_bytes());b.extend_from_slice(&val.to_le_bytes());
 }
 #[test]fn reads_door_axis_and_angle(){
  let mut b=Vec::new();strfield("ModelName","mod:Models/car.V3O",&mut b);
  b.extend_from_slice(&4u32.to_le_bytes());for c in "Door".encode_utf16(){b.extend_from_slice(&c.to_le_bytes());}
  b.extend_from_slice(&0u32.to_le_bytes());
  floatfield("AxisX",0.,&mut b);floatfield("AxisY",1.,&mut b);
  floatfield("AxisZ",0.,&mut b);floatfield("OpenAngle",-125.,&mut b);
  strfield("Filename","mod:Prototypes/car_door.e4p",&mut b);
  floatfield("PosX",1.,&mut b);floatfield("PosY",2.,&mut b);floatfield("PosZ",3.,&mut b);
  let result=parse(&b).unwrap();
  assert_eq!(result.attachments[0].door_axis,Some([0.,1.,0.]));
  assert_eq!(result.attachments[0].open_angle,Some(-125.));
 }
 #[test]fn reads_attachment(){
  let mut b=Vec::new();strfield("ModelName","mod:Models/a.V3O",&mut b);
  strfield("Filename","mod:Prototypes/a_wheel.e4p",&mut b);
  floatfield("PosX",2.,&mut b);floatfield("PosY",3.,&mut b);floatfield("PosZ",4.,&mut b);
  strfield("Rotation","1 0 0 0 1 0 0 0 1",&mut b);
  let a=parse(&b).unwrap();assert_eq!(a.attachments.len(),1);assert_eq!(a.attachments[0].position,[2.,3.,4.]);
 }
}
