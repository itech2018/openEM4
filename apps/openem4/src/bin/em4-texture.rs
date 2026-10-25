//! Export the first mip of a supported original DDS to a PNG preview.
use std::{env,fs,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let mut args=env::args_os().skip(1);
 let input=PathBuf::from(args.next().ok_or("Usage: em4-texture <input.dds> <output.png>")?);
 let output=PathBuf::from(args.next().ok_or("Usage: em4-texture <input.dds> <output.png>")?);
 if args.next().is_some() {return Err("Too many arguments".into());}
 let data=fs::read(input)?;
 let decoded=em4_formats::dds::decode_dds(&data)?;
 image::save_buffer_with_format(output,&decoded.pixels,decoded.width,decoded.height,image::ColorType::Rgba8,image::ImageFormat::Png)?;
 println!("Exported {}x{} DDS preview",decoded.width,decoded.height);
 Ok(())
}
