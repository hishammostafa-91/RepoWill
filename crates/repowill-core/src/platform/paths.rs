// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub struct AppDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
}

pub fn repowill_home() -> Option<PathBuf> {
    std::env::var_os("REPOWILL_HOME").map(PathBuf::from)
}

pub fn app_dirs() -> Result<AppDirs, String> {
    if let Some(home) = repowill_home() {
        return Ok(AppDirs {
            config: home.join("config"),
            data: home.join("data"),
            cache: home.join("cache"),
            state: home.join("state"),
        });
    }
    let pd = ProjectDirs::from("org", "AfterLeads", "RepoWill")
        .ok_or_else(|| "could not resolve project directories".to_string())?;
    Ok(AppDirs {
        config: pd.config_dir().to_path_buf(),
        data: pd.data_dir().to_path_buf(),
        cache: pd.cache_dir().to_path_buf(),
        state: pd
            .state_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| pd.data_dir().join("state")),
    })
}

pub fn normalize(path: &Path) -> Utf8PathBuf {
    let simplified = dunce::simplified(path);
    let p = simplified.to_owned();
    Utf8PathBuf::from_path_buf(p)
        .unwrap_or_else(|p| Utf8PathBuf::from(p.to_string_lossy().to_string()))
}

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    let na = normalize(a);
    let nb = normalize(b);
    if cfg!(windows) {
        na.as_str().eq_ignore_ascii_case(nb.as_str())
    } else {
        na == nb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn repowill_home_callable() {
        let _ = repowill_home();
    }

    #[test]
    fn normalize_simple() {
        let p = PathBuf::from("a/b/../c");
        let n = normalize(&p);
        assert!(n.as_str().contains("c"));
    }

    #[test]
    fn paths_equal_case_insensitive_on_windows() {
        let a = PathBuf::from("Foo/Bar");
        let b = PathBuf::from("foo/bar");
        if cfg!(windows) {
            assert!(paths_equal(&a, &b));
        } else {
            assert!(!paths_equal(&a, &b));
        }
    }
}
