//! Non-destructive CPU texture enlargement with an external content-addressed cache.
//! Original Emergency 4 assets are never written to.
use std::{env,fs,path::PathBuf};
use image::{RgbaImage,imageops::FilterType};

pub fn cache_dir()->Option<PathBuf>{
 #[cfg(windows)]
 let base=env::var_os("LOCALAPPDATA").map(PathBuf::from);
 #[cfg(not(windows))]
 let base=env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
  .or_else(||env::var_os("HOME").map(|h|PathBuf::from(h).join(".cache")));
 base.map(|p|p.join("openEM4").join("textures").join("spatial-v1"))
}
pub fn upscale(w:u32,h:u32,pixels:Vec<u8>,factor:u32)->(u32,u32,Vec<u8>){
 if factor!=4||w==0||h==0{return (w,h,pixels);}
 let Some(tw)=w.checked_mul(4) else{return(w,h,pixels)};
 let Some(th)=h.checked_mul(4) else{return(w,h,pixels)};
 // Keep textures bounded; 8192x8192 RGBA is 256 MiB per texture.
 if tw>8192||th>8192||(tw as u64)*(th as u64)>16_777_216 {
  eprintln!("4x texture skipped: {}x{} exceeds 64 MiB decoded limit",tw,th);
  return(w,h,pixels)
 }
 let Some(source)=RgbaImage::from_raw(w,h,pixels.clone())else{return(w,h,pixels)};
 let mut hasher=blake3::Hasher::new();
 hasher.update(b"openem4-spatial-v1-catmullrom-4x");
 hasher.update(&w.to_le_bytes());hasher.update(&h.to_le_bytes());hasher.update(&pixels);
 let name=format!("{}.png",hasher.finalize().to_hex());
 let cache=cache_dir().map(|p|p.join(name));
 if let Some(ref path)=cache {
  if let Ok(image)=image::open(path){
   let rgba=image.to_rgba8();
   if rgba.dimensions()==(tw,th){
    println!("4x texture cache hit: {}",path.display());
    return(tw,th,rgba.into_raw());
   }
  }
 }
 let enhanced=image::imageops::resize(&source,tw,th,FilterType::CatmullRom);
 if let Some(ref path)=cache {
  if let Some(parent)=path.parent(){
   if fs::create_dir_all(parent).is_ok(){
    if let Err(e)=enhanced.save(path){eprintln!("Texture cache write failed: {e}");}
   }
  }
 }
 println!("4x spatial texture generated: {}x{} -> {}x{}",w,h,tw,th);
 (tw,th,enhanced.into_raw())
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn no_scaling_by_default(){assert_eq!(upscale(1,1,vec![3,4,5,6],1),(1,1,vec![3,4,5,6]));}
 #[test]fn fourfold_dimensions(){let (w,h,p)=upscale(2,2,vec![255;16],4);assert_eq!((w,h,p.len()),(8,8,256));}
 #[test]fn invalid_buffer_untouched(){assert_eq!(upscale(4,4,vec![1,2],4),(4,4,vec![1,2]));}
}
