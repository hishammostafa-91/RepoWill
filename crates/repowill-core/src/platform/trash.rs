// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::path::Path;

pub fn delete(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn delete_missing_file_errors() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("nope.txt");
        assert!(delete(&p).is_err());
    }
}
