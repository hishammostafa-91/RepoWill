// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::fs;
use std::io::Write;
use std::path::Path;

use fd_lock::RwLock;

pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(contents).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn backup_file(path: &Path) -> Result<Option<std::path::PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bak = path.with_extension("bak");
    fs::copy(path, &bak).map_err(|e| e.to_string())?;
    Ok(Some(bak))
}

pub fn with_lock<R, F: FnOnce(&mut std::fs::File) -> R>(path: &Path, f: F) -> Result<R, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut lock = RwLock::new(file);
    let mut guard = lock.write().map_err(|e| e.to_string())?;
    Ok(f(&mut guard))
}

pub fn set_restrictive_permissions(_path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(_path, perms).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("a.txt");
        atomic_write(&p, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "hello");
    }

    #[test]
    fn backup_file_creates_bak() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, "x").unwrap();
        let bak = backup_file(&p).unwrap();
        assert!(bak.unwrap().exists());
    }
}
