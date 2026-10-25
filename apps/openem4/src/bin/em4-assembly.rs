use std::{env,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let p=PathBuf::from(env::args_os().nth(1).ok_or("Usage: em4-assembly <vehicle.e4p>")?);
 let a=em4_formats::e4p_assembly::parse_file(&p)?;
 println!("{}",serde_json::to_string_pretty(&a)?);
 Ok(())
}
