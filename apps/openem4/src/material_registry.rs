//! Reusable content-addressed GPU material registry.
//! A registry belongs to one wgpu device, material layout, and sampler.
//! Future scene renderers can share a registry among their objects.
use std::{collections::HashMap,sync::Arc};
pub struct MaterialRegistry {
 entries:HashMap<blake3::Hash,Arc<wgpu::BindGroup>>,
 bytes:u64,
 reuses:usize,
}
impl MaterialRegistry {
 pub fn new()->Self {Self{entries:HashMap::new(),bytes:0,reuses:0}}
 pub fn unique_count(&self)->usize{self.entries.len()}
 pub fn reused_count(&self)->usize{self.reuses}
 pub fn estimated_bytes(&self)->u64{self.bytes}
 pub fn get_or_upload<F>(&mut self,w:u32,h:u32,pixels:Vec<u8>,upload:F)->Result<Arc<wgpu::BindGroup>,String>
 where F:FnOnce(u32,u32,Vec<u8>)->Result<wgpu::BindGroup,String> {
  if w==0||h==0||pixels.len()!=(w as usize).saturating_mul(h as usize).saturating_mul(4) {
   return Err("invalid RGBA texture dimensions".into());
  }
  let key=material_key(w,h,&pixels);
  if let Some(group)=self.entries.get(&key){self.reuses+=1;return Ok(group.clone());}
  let group=Arc::new(upload(w,h,pixels)?);
  self.bytes=self.bytes.saturating_add(mip_bytes(w,h));
  self.entries.insert(key,group.clone());
  Ok(group)
 }
}
pub fn material_key(w:u32,h:u32,pixels:&[u8])->blake3::Hash{
 let mut hash=blake3::Hasher::new();
 hash.update(b"openem4-rgba8-srgb-material-v1");
 hash.update(&w.to_le_bytes());hash.update(&h.to_le_bytes());hash.update(pixels);
 hash.finalize()
}
pub fn mip_bytes(w:u32,h:u32)->u64{
 let (mut w,mut h)=(w.max(1) as u64,h.max(1) as u64);let mut bytes=0u64;
 loop{bytes=bytes.saturating_add(w.saturating_mul(h).saturating_mul(4));if w==1&&h==1{break;}w=(w/2).max(1);h=(h/2).max(1);}
 bytes
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn fingerprints_include_dimensions_and_contents(){
  assert_ne!(material_key(1,4,&[0;16]),material_key(2,2,&[0;16]));
  assert_ne!(material_key(2,2,&[0;16]),material_key(2,2,&[1;16]));
 }
 #[test]fn mip_chain_bytes(){assert_eq!(mip_bytes(4,4),84);}
}
