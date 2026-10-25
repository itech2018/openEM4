use std::{env,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let p=PathBuf::from(env::args_os().nth(1).ok_or("Usage: em4-e4p-inspect <prototype.e4p>")?);
 let report=em4_formats::e4p::inspect_path(&p)?;
 println!("{}",serde_json::to_string_pretty(&report)?);
 Ok(())
}
