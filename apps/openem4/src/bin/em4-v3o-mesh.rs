//! Print geometry counts for supported original V3O models.
use std::{env,fs};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let file=env::args_os().nth(1).ok_or("Usage: em4-v3o-mesh <model.v3o>")?;
 let mesh=em4_formats::v3o_mesh::parse(&fs::read(file)?)?;
 println!("V3O {} | vertices: {} | triangles: {} | surfaces: {}",mesh.version,mesh.vertices.len(),mesh.triangles.len(),mesh.surfaces.len());
 for surface in mesh.surfaces {println!("surface: {} | {}",surface.name,surface.texture);}
 Ok(())
}
