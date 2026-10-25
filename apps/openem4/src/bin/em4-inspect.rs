use std::{env, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("Usage: em4-inspect <asset-file>")?;
    if args.next().is_some() { return Err("Usage: em4-inspect <asset-file>".into()); }
    let result = em4_formats::inspect(&PathBuf::from(path))?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
