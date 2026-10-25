//! Geometry decoder for unpacked V3O 1.43 text, including packed sources.
//! Coordinates and UV normalization are experimental; no rendering claim.
use std::io::{self,ErrorKind};
use serde::Serialize;
fn bad(s:&str)->io::Error {io::Error::new(ErrorKind::InvalidData,s.to_owned())}
#[derive(Debug,Serialize)]
pub struct Vertex {pub position:[f32;3],pub normal:[f32;3],pub uv:[f32;2]}
#[derive(Debug,Serialize)]
pub struct Surface {pub name:String,pub texture:String}
#[derive(Debug,Serialize)]
pub struct Triangle {pub indices:[u32;3],pub surface_index:usize}
#[derive(Debug,Serialize)]
pub struct Mesh {pub version:String,pub vertices:Vec<Vertex>,pub triangles:Vec<Triangle>,pub surfaces:Vec<Surface>}
fn number(s:&str)->io::Result<i32>{s.trim().parse().map_err(|_|bad("invalid V3O integer"))}
fn count(s:&str,tag:&str)->Option<usize>{
 s.trim().strip_prefix(tag)?.strip_suffix(']')?.parse().ok()
}
pub fn parse(bytes:&[u8])->io::Result<Mesh>{
 let unpacked=crate::v3o_packed::unpack(bytes)?;
 let s=std::str::from_utf8(&unpacked).map_err(|_|bad("V3O is not UTF-8 text"))?;
 let mut version=String::new();
 let (mut nv,mut np,mut ns)=(None,None,None);
 let (mut vertices,mut triangles,mut surfaces)=(Vec::new(),Vec::new(),Vec::new());
 for line in s.lines() {
  let line=line.trim();
  if let Some(v)=line.strip_prefix("[VNUM=").and_then(|x|x.strip_suffix(']')){version=v.into();}
  if let Some(n)=count(line,"[NUMVERTICES="){nv=Some(n);}
  if let Some(n)=count(line,"[NUMPOLYGONS="){np=Some(n);}
  if let Some(n)=count(line,"[NUMSURFACES="){ns=Some(n);}
  if !matches!(line.split(',').next(),Some("D"|"P"|"SRF")){continue;}
  let parts:Vec<_>=line.split(',').map(str::trim).collect();
  match parts[0] {
   "SRF"=>{
    if parts.len()<6{return Err(bad("truncated V3O surface"));}
    surfaces.push(Surface{name:parts[1].into(),texture:parts[5].into()});
   },
   "D"=>{
    if parts.len()<9{return Err(bad("truncated V3O vertex"));}
    let mut n=[0i32;8];for (i,v) in n.iter_mut().enumerate(){*v=number(parts[i+1])?;}
    vertices.push(Vertex{position:[n[0] as f32/100.0,n[1] as f32/100.0,n[2] as f32/100.0],
     normal:[n[3] as f32/256.0,n[4] as f32/256.0,n[5] as f32/256.0],
     uv:[n[6] as f32/1024.0,n[7] as f32/1024.0]});
   },
   "P"=>{
    if parts.len()<10{return Err(bad("truncated V3O polygon"));}
    let size=number(parts[1])?;
    if size!=3{return Err(bad("only V3O triangular polygons supported"));}
    let mut indices=[0u32;3];
    for (i,index) in indices.iter_mut().enumerate(){
     let n=number(parts[i+2])?;
     if n<=0{return Err(bad("V3O indices must be 1-based positive"));}
     *index=(n-1) as u32;
    }
    let mat=number(parts[9])?;
    if mat<=0{return Err(bad("invalid V3O surface index"));}
    triangles.push(Triangle{indices,surface_index:(mat-1) as usize});
   },_=>{}
  }
  if vertices.len()>2_000_000 || triangles.len()>2_000_000 || surfaces.len()>1024 {
   return Err(bad("V3O geometry exceeds safety limits"));
  }
 }
 if version.is_empty(){return Err(bad("missing V3O version"));}
 if nv!=Some(vertices.len())||np!=Some(triangles.len())||ns!=Some(surfaces.len()){return Err(bad("V3O record count mismatch"));}
 if triangles.iter().any(|t|t.indices.iter().any(|&i|i as usize>=vertices.len())||t.surface_index>=surfaces.len()){
  return Err(bad("V3O face references invalid vertex or surface"));
 }
 Ok(Mesh{version,vertices,triangles,surfaces})
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn parses_triangle(){
  let s=b"[VNUM=1.43]\n[NUMSURFACES=1]\n[NUMVERTICES=3]\n[NUMPOLYGONS=1]\nSRF, body, 0,0,0,body.dds\nD,0,0,0,0,256,0,0,0\nD,100,0,0,0,256,0,1024,0\nD,0,100,0,0,256,0,0,1024\nP,3,1,2,3,0,0,0,0,1\n";
  let m=parse(s).unwrap();assert_eq!(m.triangles[0].indices,[0,1,2]);assert_eq!(m.vertices[1].position[0],1.0);
 }
 #[test] fn missing_vertex_rejected(){
  let s=b"[VNUM=1.43]\n[NUMSURFACES=0]\n[NUMVERTICES=1]\n[NUMPOLYGONS=0]\n";
  assert!(parse(s).is_err());
 }
}
