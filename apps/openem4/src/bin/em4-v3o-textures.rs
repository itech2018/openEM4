//! List texture references in an unpacked textual V3O.
use std::{env,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let mut args=env::args_os().skip(1);
 let path=PathBuf::from(args.next().ok_or("Usage: em4-v3o-textures <unpacked.v3o>")?);
 if args.next().is_some(){return Err("Expected only one V3O path".into());}
 let refs=em4_formats::v3o_text::inspect_file(&path)?;
 for item in &refs {println!("{item}");}
 eprintln!("Found {} texture reference(s). Geometry decoding is not yet implemented.",refs.len());
 Ok(())
}
