//! The engine was called rust2d before 0.5.0. Folders and files it kept under that name in the
//! user's home (game settings and achievements, the asset library, the editor's recent list) are
//! adopted the first time the new location is needed: copied over, the old ones left as they are.

use std::path::{Path, PathBuf};

/// The engine's former technical name (folder names in the user's home).
pub const OLD_NAME: &str = "rust2d";

/// Copies `old` (a folder or a file) to `new` when `new` doesn't exist yet. A folder is copied
/// to a temporary sibling first and renamed, so an interrupted copy is never taken for the real
/// one. Problems are logged; the caller then simply starts without the old data.
pub fn adopt(new: &Path, old: &Path) {
    if new.exists() || !old.exists() {
        return;
    }
    let res = if old.is_dir() {
        let tmp = tmp_sibling(new);
        let _ = std::fs::remove_dir_all(&tmp);
        copy_dir(old, &tmp).and_then(|_| std::fs::rename(&tmp, new))
    } else {
        new.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::copy(old, new).map(|_| ()))
    };
    match res {
        Ok(()) => eprintln!("[rustpg2d] copied {} to {} (renamed from rust2d)", old.display(), new.display()),
        Err(e) => eprintln!("[rustpg2d] could not copy {} to {}: {e}", old.display(), new.display()),
    }
}

fn tmp_sibling(p: &Path) -> PathBuf {
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(".adopting");
    p.with_file_name(name)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adopts_old_folders_and_files_once() {
        let root = std::env::temp_dir().join(format!("rustpg2d_legacy_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let old = root.join("rust2d/game");
        std::fs::create_dir_all(old.join("sub")).unwrap();
        std::fs::write(old.join("settings.ron"), "(a)").unwrap();
        std::fs::write(old.join("sub/x"), "x").unwrap();
        let new = root.join("rustpg2d/game");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        adopt(&new, &old);
        assert_eq!(std::fs::read_to_string(new.join("settings.ron")).unwrap(), "(a)");
        assert_eq!(std::fs::read_to_string(new.join("sub/x")).unwrap(), "x");
        assert!(old.join("settings.ron").exists(), "the old folder stays");
        assert!(!tmp_sibling(&new).exists());
        // An existing new folder is never overwritten.
        std::fs::write(old.join("settings.ron"), "(b)").unwrap();
        adopt(&new, &old);
        assert_eq!(std::fs::read_to_string(new.join("settings.ron")).unwrap(), "(a)");
        // Files, with missing parent folders.
        std::fs::write(root.join("rust2d/recent.ron"), "[]").unwrap();
        adopt(&root.join("cfg/rustpg2d/recent.ron"), &root.join("rust2d/recent.ron"));
        assert_eq!(std::fs::read_to_string(root.join("cfg/rustpg2d/recent.ron")).unwrap(), "[]");
        // Nothing old: nothing happens.
        adopt(&root.join("none"), &root.join("missing"));
        assert!(!root.join("none").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
