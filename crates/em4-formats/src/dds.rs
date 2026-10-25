//! Bounded DDS BC1/BC2/BC3 (DXT1/3/5) CPU decoder for preview tools.
use std::io::{self, ErrorKind};
#[derive(Debug,Clone)]
pub struct RgbaImage { pub width:u32,pub height:u32,pub pixels:Vec<u8> }
fn bad(message:&str)->io::Error { io::Error::new(ErrorKind::InvalidData,message.to_owned()) }
fn le16(b:&[u8])->u16 {u16::from_le_bytes([b[0],b[1]])}
fn le32(b:&[u8])->u32 {u32::from_le_bytes([b[0],b[1],b[2],b[3]])}
fn rgb565(c:u16)->[u8;3] {
 let r=((c>>11)&31) as u32;let g=((c>>5)&63) as u32;let b=(c&31) as u32;
 [((r*255+15)/31) as u8,((g*255+31)/63) as u8,((b*255+15)/31) as u8]
}
fn colors(block:&[u8], allow_transparent:bool)->[[u8;4];4] {
 let c0=le16(&block[0..2]);let c1=le16(&block[2..4]);
 let a=rgb565(c0);let b=rgb565(c1);
 let mut p=[[0;4];4];p[0]=[a[0],a[1],a[2],255];p[1]=[b[0],b[1],b[2],255];
 if c0>c1 || !allow_transparent {
  for i in 0..3 { p[2][i]=((2*a[i] as u16+b[i] as u16)/3) as u8;p[3][i]=((a[i] as u16+2*b[i] as u16)/3) as u8; }
  p[2][3]=255;p[3][3]=255;
 } else {
  for i in 0..3 {p[2][i]=((a[i] as u16+b[i] as u16)/2) as u8;}
  p[2][3]=255;
 }
 p
}
pub fn decode_dds(bytes:&[u8])->io::Result<RgbaImage> {
 if bytes.len()<128 || &bytes[..4]!=b"DDS " || le32(&bytes[4..8])!=124 || le32(&bytes[76..80])!=32 {
  return Err(bad("invalid DDS header"));
 }
 let height=le32(&bytes[12..16]);let width=le32(&bytes[16..20]);
 if width==0 || height==0 || width>16384 || height>16384 || (width as u64)*(height as u64)>16_777_216 {
  return Err(bad("DDS texture dimensions exceed preview limits"));
 }
 let fourcc=&bytes[84..88];
 let bytes_per_block=match fourcc {b"DXT1"=>8usize,b"DXT3"|b"DXT5"=>16usize,_=>return Err(bad("unsupported DDS FourCC (preview supports DXT1/3/5)"))};
 let bx=(width as usize).div_ceil(4);let by=(height as usize).div_ceil(4);
 let need=bx.checked_mul(by).and_then(|n|n.checked_mul(bytes_per_block)).and_then(|n|n.checked_add(128)).ok_or_else(||bad("overflow"))?;
 if bytes.len()<need {return Err(bad("truncated DDS pixel data"));}
 let mut pixels=vec![0u8;width as usize*height as usize*4];
 for yb in 0..by {for xb in 0..bx {
  let offset=128+(yb*bx+xb)*bytes_per_block;let block=&bytes[offset..offset+bytes_per_block];
  let cb=if bytes_per_block==8 {block} else {&block[8..]};
  let palette=colors(cb,fourcc==b"DXT1");
  let indices=le32(&cb[4..8]);
  let alpha_indices=if fourcc==b"DXT5" {
   let mut v=0u64;for i in 0..6 {v|=(block[2+i] as u64)<<(8*i);}v
  } else {0};
  let mut alpha=[255u8;8];
  if fourcc==b"DXT5" {
   alpha[0]=block[0];alpha[1]=block[1];
   if alpha[0]>alpha[1] {
    for i in 2..8 {alpha[i]=(((8-i) as u16*alpha[0] as u16+(i-1) as u16*alpha[1] as u16)/7) as u8;}
   } else {
    for i in 2..6 {alpha[i]=(((6-i) as u16*alpha[0] as u16+(i-1) as u16*alpha[1] as u16)/5) as u8;}
    alpha[6]=0;alpha[7]=255;
   }
  }
  for py in 0..4usize {for px in 0..4usize {
   let x=xb*4+px;let y=yb*4+py;if x>=width as usize || y>=height as usize {continue;}
   let pi=py*4+px;let mut color=palette[((indices>>(2*pi))&3) as usize];
   if fourcc==b"DXT3" {
    let bits=le16(&block[(py*2)..(py*2+2)]);
    color[3]=(((bits>>(px*4))&15) as u8)*17;
   } else if fourcc==b"DXT5" {
    color[3]=alpha[((alpha_indices>>(3*pi))&7) as usize];
   }
   let dest=(y*width as usize+x)*4;pixels[dest..dest+4].copy_from_slice(&color);
  }}
 }}
 Ok(RgbaImage {width,height,pixels})
}
#[cfg(test)]
mod tests {
 use super::*;
 fn fixture(fourcc:&[u8;4],size:usize)->Vec<u8>{
  let mut b=vec![0;128+size];b[0..4].copy_from_slice(b"DDS ");b[4..8].copy_from_slice(&124u32.to_le_bytes());
  b[12..16].copy_from_slice(&4u32.to_le_bytes());b[16..20].copy_from_slice(&4u32.to_le_bytes());
  b[76..80].copy_from_slice(&32u32.to_le_bytes());b[84..88].copy_from_slice(fourcc);b
 }
 #[test] fn bc1_red_block() {
  let mut b=fixture(b"DXT1",8);
  b[128..130].copy_from_slice(&0xf800u16.to_le_bytes());
  let out=decode_dds(&b).unwrap();assert_eq!(&out.pixels[..4], &[255,0,0,255]);
 }
 #[test] fn truncated_fails() {assert!(decode_dds(&fixture(b"DXT5",8)).is_err());}
 #[test] fn unsupported_fails() {assert!(decode_dds(&fixture(b"DX10",16)).is_err());}
}
