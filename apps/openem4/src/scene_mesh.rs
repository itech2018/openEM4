//! Minimal multi-instance preview scene assembly.
//! Input geometry remains in original EM4 coordinates; placement is done
//! before the viewer's Z-up to Y-up conversion.
use em4_formats::v3o_mesh::{Mesh,Vertex,Triangle};
pub fn repeated_v3o(mut mesh:Mesh,count:usize)->Result<Mesh,String>{
 if !(1..=16).contains(&count){return Err("instance count must be 1..=16".into())}
 if count==1{return Ok(mesh)}
 let source_vertices=mesh.vertices.iter().map(|v|(v.position,v.normal,v.uv)).collect::<Vec<_>>();
 let source_triangles=mesh.triangles.iter().map(|t|(t.indices,t.surface_index)).collect::<Vec<_>>();
 if source_vertices.is_empty(){return Err("cannot instance empty mesh".into())}
 if source_vertices.len().checked_mul(count).filter(|&n|n<=2_000_000).is_none()
 ||source_triangles.len().checked_mul(count).filter(|&n|n<=2_000_000).is_none(){
  return Err("scene instance geometry limit exceeded".into());
 }
 let min_x=source_vertices.iter().map(|v|v.0[0]).fold(f32::INFINITY,f32::min);
 let max_x=source_vertices.iter().map(|v|v.0[0]).fold(f32::NEG_INFINITY,f32::max);
 let min_y=source_vertices.iter().map(|v|v.0[1]).fold(f32::INFINITY,f32::min);
 let max_y=source_vertices.iter().map(|v|v.0[1]).fold(f32::NEG_INFINITY,f32::max);
 let spacing=((max_x-min_x).max(max_y-min_y)*1.4).max(2.0);
 let columns=(count as f32).sqrt().ceil() as usize;
 let original_count=source_vertices.len();
 for instance in 1..count{
  let offset_x=(instance%columns) as f32*spacing;
  let offset_y=(instance/columns) as f32*spacing;
  let base=original_count.checked_mul(instance).ok_or("vertex offset overflow")?;
  let base=u32::try_from(base).map_err(|_|"vertex index overflow")?;
  for &(p,n,uv) in &source_vertices{
   mesh.vertices.push(Vertex{position:[p[0]+offset_x,p[1]+offset_y,p[2]],normal:n,uv});
  }
  for &(indices,surface_index) in &source_triangles{
   mesh.triangles.push(Triangle{indices:[
    indices[0].checked_add(base).ok_or("index overflow")?,
    indices[1].checked_add(base).ok_or("index overflow")?,
    indices[2].checked_add(base).ok_or("index overflow")?
   ],surface_index});
  }
 }
 Ok(mesh)
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn repeats_geometry_without_duplication_of_materials(){
  let m=Mesh{version:"1.43".into(),vertices:vec![
   Vertex{position:[0.,0.,0.],normal:[0.,0.,1.],uv:[0.,0.]},
   Vertex{position:[1.,0.,0.],normal:[0.,0.,1.],uv:[1.,0.]},
   Vertex{position:[0.,1.,0.],normal:[0.,0.,1.],uv:[0.,1.]}],
   triangles:vec![Triangle{indices:[0,1,2],surface_index:0}],
   surfaces:vec![em4_formats::v3o_mesh::Surface{name:"body".into(),texture:"body.dds".into()}]};
  let scene=repeated_v3o(m,4).unwrap();
  assert_eq!(scene.vertices.len(),12);
  assert_eq!(scene.triangles.len(),4);
  assert_eq!(scene.surfaces.len(),1);
  assert_eq!(scene.triangles[3].indices,[9,10,11]);
  assert!(scene.vertices[3].position[0]>scene.vertices[0].position[0]);
 }
}
