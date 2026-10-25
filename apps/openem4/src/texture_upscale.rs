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
 base.map(|p|p.join("openEM4").join("textures").join("automatic-v3"))
}

/// Deterministic, conservative enhancement policy. No per-mod tuning required.
pub fn automatic_factor(name:&str,w:u32,h:u32,pixels:&[u8])->u32{
 if w==0||h==0||w>2048||h>2048||pixels.len()!=w as usize*h as usize*4{return 1}
 let name=name.to_ascii_lowercase();
 if ["normal","_n.","_nm.","rough","specular","gloss","metal","corona","lightmap","height","bump"].iter().any(|tag|name.contains(tag)){return 1}
 // Preserve alpha-heavy sprites, icons and glass; don't smear cutout edges.
 let alpha_pixels=pixels.chunks_exact(4).filter(|p|p[3]<240).count();
 if alpha_pixels>pixels.len()/4/5{return 1}
 if w.max(h)>=1024{return 1}
 if w.max(h)<=512{4}else{2}
}
/// Automatic enhancement, with content-addressed reuse and original-size fallback.
pub fn enhance_auto(name:&str,w:u32,h:u32,pixels:Vec<u8>)->(u32,u32,Vec<u8>){
 let factor=automatic_factor(name,w,h,&pixels);
 if factor==1{return(w,h,pixels)}
 upscale_with_factor(w,h,pixels,factor)
}

pub fn upscale(w:u32,h:u32,pixels:Vec<u8>,factor:u32)->(u32,u32,Vec<u8>){
 upscale_with_factor(w,h,pixels,factor)
}
fn upscale_with_factor(w:u32,h:u32,pixels:Vec<u8>,factor:u32)->(u32,u32,Vec<u8>){
 if (factor!=2&&factor!=4)||w==0||h==0{return (w,h,pixels);}
 let Some(tw)=w.checked_mul(factor) else{return(w,h,pixels)};
 let Some(th)=h.checked_mul(factor) else{return(w,h,pixels)};
 // Keep textures bounded; 8192x8192 RGBA is 256 MiB per texture.
 if tw>8192||th>8192||(tw as u64)*(th as u64)>16_777_216 {
  eprintln!("Texture enhancement skipped: {}x{} exceeds 64 MiB decoded limit",tw,th);
  return(w,h,pixels)
 }
 let Some(source)=RgbaImage::from_raw(w,h,pixels.clone())else{return(w,h,pixels)};
 let mut hasher=blake3::Hasher::new();
 hasher.update(b"openem4-lanczos3-sharpen25-auto-v3");hasher.update(&factor.to_le_bytes());
 hasher.update(&w.to_le_bytes());hasher.update(&h.to_le_bytes());hasher.update(&pixels);
 let name=format!("{}.png",hasher.finalize().to_hex());
 let cache=cache_dir().map(|p|p.join(name));
 if let Some(ref path)=cache {
  if let Ok(image)=image::open(path){
   let rgba=image.to_rgba8();
   if rgba.dimensions()==(tw,th){
    println!("Texture cache hit: {}",path.display());
    return(tw,th,rgba.into_raw());
   }
  }
 }
 let mut enhanced=image::imageops::resize(&source,tw,th,FilterType::Lanczos3);
 // Mild RGB unsharp masking: amount=0.25. Never sharpen or alter alpha,
 // preventing transparent edges from becoming opaque halos.
 let blurred=image::imageops::blur(&enhanced,0.8);
 for (pixel,soft) in enhanced.pixels_mut().zip(blurred.pixels()){
  for channel in 0..3 {
   let original=pixel[channel] as f32;
   let correction=(original-soft[channel] as f32)*0.25;
   pixel[channel]=(original+correction).round().clamp(0.0,255.0) as u8;
  }
 }
 if let Some(ref path)=cache {
  if let Some(parent)=path.parent(){
   if fs::create_dir_all(parent).is_ok(){
    if let Err(e)=enhanced.save(path){eprintln!("Texture cache write failed: {e}");}
   }
  }
 }
 println!("Automatic Lanczos3 + 25% sharpening generated: {}x{} -> {}x{}",w,h,tw,th);
 (tw,th,enhanced.into_raw())
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn auto_skips_data_and_transparency(){
  let rgba=vec![255u8;64*64*4];
  assert_eq!(automatic_factor("car_normal.dds",64,64,&rgba),1);
  assert_eq!(automatic_factor("car_diffuse.dds",64,64,&rgba),4);
  let mut transparent=rgba.clone();
  for p in transparent.chunks_exact_mut(4){p[3]=0;}
  assert_eq!(automatic_factor("window.dds",64,64,&transparent),1);
 }
 #[test]fn auto_respects_large_source(){
  assert_eq!(automatic_factor("large.dds",1024,1024,&vec![255;1024*1024*4]),1);
 }
 #[test]fn no_scaling_by_default(){assert_eq!(upscale(1,1,vec![3,4,5,6],1),(1,1,vec![3,4,5,6]));}
 #[test]fn fourfold_dimensions(){let (w,h,p)=upscale(2,2,vec![255;16],4);assert_eq!((w,h,p.len()),(8,8,256));}
 #[test]fn preserves_alpha_values_at_corners(){
  let (w,h,p)=upscale(2,2,vec![10,20,30,0, 30,40,50,255, 60,70,80,255, 90,100,110,0],4);
  assert_eq!((w,h),(8,8));
  assert_eq!(p[3],0);
  assert_eq!(p[((7*8+7)*4+3) as usize],0);
 }
 #[test]fn invalid_buffer_untouched(){assert_eq!(upscale(4,4,vec![1,2],4),(4,4,vec![1,2]));}
}
