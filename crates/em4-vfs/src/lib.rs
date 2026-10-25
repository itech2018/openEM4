//! Read-only installation inventory and mod-first content mounting.
use serde::Serialize;
use std::{collections::BTreeMap, fs, io, path::{Component, Path, PathBuf}};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct Mount { pub name: String, pub path: PathBuf }

#[derive(Debug, Default)]
pub struct Vfs { mounts: Vec<Mount> }

impl Vfs {
    pub fn new() -> Self { Self::default() }
    pub fn mount(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> io::Result<()> {
        let path = path.into().canonicalize()?;
        if !path.is_dir() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "mount is not a directory")); }
        self.mounts.push(Mount { name: name.into(), path });
        Ok(())
    }
    pub fn mounts(&self) -> &[Mount] { &self.mounts }
    pub fn resolve(&self, relative: &Path) -> Option<PathBuf> {
        if !safe_relative(relative) { return None; }
        for mount in &self.mounts {
            if let Some(found) = find_case_insensitive(&mount.path, relative) {
                if found.canonicalize().ok()?.starts_with(&mount.path) && found.is_file() { return Some(found); }
            }
        }
        None
    }
    pub fn read(&self, relative: &Path) -> io::Result<Vec<u8>> {
        match self.resolve(relative) {
            Some(file) => fs::read(file),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "asset not found")),
        }
    }
}
fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|x| matches!(x, Component::Normal(_)))
}
fn find_case_insensitive(root: &Path, rel: &Path) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for part in rel.components() {
        let Component::Normal(name) = part else { return None; };
        let direct = current.join(name);
        if direct.exists() { current = direct; continue; }
        let name = name.to_string_lossy();
        current = fs::read_dir(&current).ok()?.flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&name))?.path();
    }
    Some(current)
}
#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub installation: PathBuf,
    pub exists: bool,
    pub probable_game: bool,
    pub mod_names: Vec<String>,
    pub file_counts: BTreeMap<String, usize>,
    pub total_files: usize,
    pub warnings: Vec<String>,
}
pub fn scan_installation(root: &Path) -> ScanReport {
    let exists = root.is_dir();
    let mut report = ScanReport { installation: root.to_path_buf(), exists, probable_game: false,
        mod_names: vec![], file_counts: BTreeMap::new(), total_files: 0, warnings: vec![] };
    if !exists { report.warnings.push("installation path is not a directory".into()); return report; }
    let names: Vec<String> = fs::read_dir(root).ok().into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase()).collect();
    report.probable_game = names.iter().any(|s| s == "data") &&
        names.iter().any(|s| matches!(s.as_str(), "em4.exe" | "emergency4.exe" | "911.exe"));
    if !report.probable_game { report.warnings.push("game structure not confirmed (expected Data and executable)".into()); }
    if let Ok(entries) = fs::read_dir(root.join("Mods")) {
        report.mod_names = entries.flatten().filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        report.mod_names.sort();
    }
    for item in WalkDir::new(root).follow_links(false).max_depth(16).into_iter().filter_map(Result::ok) {
        if !item.file_type().is_file() { continue; }
        report.total_files += 1;
        let ext = item.path().extension().and_then(|e| e.to_str()).unwrap_or("(none)").to_ascii_lowercase();
        *report.file_counts.entry(ext).or_default() += 1;
    }
    report
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn blocks_traversal() {
        let v = Vfs::new();
        assert!(v.resolve(Path::new("../secret")).is_none());
        assert!(v.resolve(Path::new("/absolute")).is_none());
    }
    #[test] fn missing_directory_reports_warning() {
        let s = scan_installation(Path::new("/surely/not/an/em4/install"));
        assert!(!s.exists); assert!(!s.warnings.is_empty());
    }
    #[test] fn mount_precedence_and_case_insensitivity() {
        let base = std::env::temp_dir().join(format!("openem4_vfs_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("vanilla")).unwrap();
        fs::create_dir_all(base.join("mod")).unwrap();
        fs::write(base.join("vanilla/Truck.TXT"), b"vanilla").unwrap();
        fs::write(base.join("mod/Truck.TXT"), b"mod").unwrap();
        let mut vfs=Vfs::new();
        vfs.mount("mod", base.join("mod")).unwrap();
        vfs.mount("base", base.join("vanilla")).unwrap();
        assert_eq!(vfs.read(Path::new("truck.txt")).unwrap(), b"mod");
        fs::remove_dir_all(base).unwrap();
    }
}
