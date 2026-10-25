//! Observed binary E4P light records (read-only, incomplete format coverage).
use serde::Serialize;
#[derive(Debug,Clone,Serialize)]
pub struct Light {
 pub points:Vec<[f32;3]>,pub position:[f32;3],
 pub corona_texture:String,pub flash_frequency:f32,pub flash_offset:f32,
 pub corona_size:f32,pub light_type:u32,pub color:u32,
}
fn u32_at(b:&[u8],i:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(i..i+4)?.try_into().ok()?))}
fn field(b:&[u8],at:usize)->Option<(String,usize)>{
 let n=u32_at(b,at)? as usize;if n==0||n>80{return None}
 let end=at.checked_add(4+n*2)?;
 let data=b.get(at+4..end)?;
 let chars=data.chunks_exact(2).map(|c|u16::from_le_bytes([c[0],c[1]])).collect::<Vec<_>>();
 let s=String::from_utf16(&chars).ok()?;
 if !s.chars().all(|c|!c.is_control()){return None}Some((s,end))
}
fn value(b:&[u8],end:usize)->Option<(u32,usize)>{
 let typ=u32_at(b,end)?;
 let next=end+8;
 match typ {
  0=>Some((0,end+4)),
  1|2=>{u32_at(b,end+4)?;Some((typ,next))},
  4=>{let n=u32_at(b,end+4)? as usize;if n>2048||end+8+n*2>b.len(){None}else{Some((typ,end+8+n*2))}},
  _=>None
 }
}
pub fn parse(bytes:&[u8])->Vec<Light>{
 let mut out=Vec::new();let mut at=0;
 while at+12<bytes.len(){
  let Some((name,end))=field(bytes,at)else{at+=1;continue};
  if name!="light" || u32_at(bytes,end)!=Some(2) {at+=1;continue}
  let mut light=Light{points:Vec::new(),position:[0.;3],corona_texture:String::new(),
   flash_frequency:0.,flash_offset:0.,corona_size:0.,light_type:0,color:0xffffffff};
  let mut pos=[None,None,None];
  let mut cursor=end+8;let limit=(cursor+5000).min(bytes.len());
  while cursor+12<limit {
   let Some((key,kend))=field(bytes,cursor)else{cursor+=1;continue};
   if key=="light" && u32_at(bytes,kend)==Some(2){break}
   if key=="Filename"||key=="Child"||key=="Wheels"{break}
   let Some((typ,next))=value(bytes,kend)else{cursor+=1;continue};
   if typ==2 {
    if let Some(bits)=u32_at(bytes,kend+4) {
     let v=f32::from_bits(bits);
     if v.is_finite(){match key.as_str(){
      "x"=>pos[0]=Some(v),"y"=>pos[1]=Some(v),"z"=>{pos[2]=Some(v);
       if let [Some(x),Some(y),Some(z)]=pos {light.points.push([x,y,z]);}
       pos=[None,None,None];},
      "flashfreq"=>light.flash_frequency=v,"flashoffs"=>light.flash_offset=v,
      "coronasize"=>light.corona_size=v,_=>{}
     }}
    }
   }else if typ==1 {let v=u32_at(bytes,kend+4).unwrap_or(0);match key.as_str(){
     "lighttype"=>light.light_type=v,"color"=>light.color=v,_=>{}
   }}else if typ==4&&key=="coronatex" {
    if let Some((value,_))=field(bytes,kend+4){light.corona_texture=value;}
   }
   cursor=next;
  }
  if !light.points.is_empty(){
   let n=light.points.len() as f32;
   for p in &light.points{for i in 0..3{light.position[i]+=p[i]/n;}}
   out.push(light);
  }
  at=cursor.max(at+1);
 }
 out
}
#[cfg(test)] mod tests{
 use super::*;
 #[test] fn ignores_empty(){assert!(parse(b"not an E4P").is_empty());}
}
