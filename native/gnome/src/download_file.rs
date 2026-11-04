//! Download staging and atomic publication, independent of GTK and the engine.
//! The engine never receives the final path, so its failure cleanup cannot delete
//! an existing user file. Keep the stage on the destination filesystem.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[derive(Debug)]
pub struct Target {
    destination: PathBuf,
    stage: tempfile::TempPath,
}
impl Target {
    pub fn prepare(destination: PathBuf, cancelled: &AtomicBool) -> Result<Self, String> {
        check(cancelled)?;
        if !destination.is_absolute() {
            return Err("Choose an absolute local download path".into());
        }
        let name = destination
            .file_name()
            .ok_or("Choose a download filename")?;
        let parent = destination
            .parent()
            .ok_or("Choose a download folder")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let destination = parent.join(name);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err("The download destination already exists".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let stage = tempfile::Builder::new()
            .prefix(".studio-download-")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?
            .into_temp_path();
        let target = Self { destination, stage };
        check(cancelled)?;
        Ok(target)
    }
    pub fn staging_path(&self) -> &Path {
        &self.stage
    }
    pub fn filename(&self) -> String {
        self.destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
    pub fn publish(self, cancelled: &AtomicBool) -> Result<u64, String> {
        check(cancelled)?;
        let file = fs::File::open(&self.stage).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("The staged download is not a regular file".into());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        check(cancelled)?;
        self.stage
            .persist_noclobber(&self.destination)
            .map_err(|e| e.to_string())?;
        Ok(metadata.len())
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("Download cancelled".into())
    } else {
        Ok(())
    }
}
pub fn cancellation() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stages_and_publishes_exact_unicode_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("download 中文.bin");
        let flag = AtomicBool::new(false);
        let target = Target::prepare(path.clone(), &flag).unwrap();
        let stage = target.staging_path().to_owned();
        let bytes = "Exact 中文😀\r\n\0".as_bytes();
        fs::write(&stage, bytes).unwrap();
        assert!(!path.exists());
        assert_eq!(target.publish(&flag).unwrap(), bytes.len() as u64);
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert!(!stage.exists());
    }
    #[test]
    fn refuses_existing_files_and_late_creation_without_removing_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing");
        let flag = AtomicBool::new(false);
        fs::write(&path, b"original").unwrap();
        assert!(Target::prepare(path.clone(), &flag).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_file(&path).unwrap();
        let target = Target::prepare(path.clone(), &flag).unwrap();
        let stage = target.staging_path().to_owned();
        fs::write(&stage, b"download").unwrap();
        fs::write(&path, b"external creation").unwrap();
        assert!(target.publish(&flag).is_err());
        assert_eq!(fs::read(path).unwrap(), b"external creation");
        assert!(!stage.exists());
    }
    #[test]
    fn cancellation_cleans_the_owned_stage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cancelled");
        let flag = AtomicBool::new(false);
        let target = Target::prepare(path.clone(), &flag).unwrap();
        let stage = target.staging_path().to_owned();
        fs::write(&stage, b"partial").unwrap();
        flag.store(true, Ordering::Release);
        assert!(target.publish(&flag).is_err());
        assert!(!path.exists());
        assert!(!stage.exists());
        assert!(Target::prepare(path, &flag).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn existing_symlinks_are_not_followed_or_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("link");
        let original = dir.path().join("original");
        fs::write(&original, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&original, &path).unwrap();
        assert!(Target::prepare(path.clone(), &AtomicBool::new(false)).is_err());
        assert!(path.is_symlink());
        assert_eq!(fs::read(original).unwrap(), b"unchanged");
    }
}
