use std::{env, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
 let mut args=env::args().skip(1);
 let file=args.next().ok_or("Usage: em4-inspect <asset-file>")?;
 let report=em4_formats::inspect(&PathBuf::from(file))?;
 println!("{}",serde_json::to_string_pretty(&report)?);
 Ok(())
}
