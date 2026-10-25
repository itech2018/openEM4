use std::{env,path::PathBuf,fs};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let path=PathBuf::from(env::args_os().nth(1).ok_or("Usage: em4-e4p-inspect <prototype.e4p>")?);
 let bytes=fs::read(&path)?;
 if bytes.iter().take(512).any(|x|*x==0){
  let report=em4_formats::e4p_binary::inspect(&bytes)?;
  println!("{}",serde_json::to_string_pretty(&report)?);
 }else{
  let report=em4_formats::e4p::inspect(&bytes)?;
  println!("{}",serde_json::to_string_pretty(&report)?);
 }
 Ok(())
}
