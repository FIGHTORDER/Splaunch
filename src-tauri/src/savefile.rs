//! Writing a file the author chose, without destroying what is already there.
//!
//! Every save in this editor is over a path a person picked in a dialog, which
//! more often than not is the file they have been working on. `fs::write`
//! truncates first: a failure between that and the last byte - a full disk, a
//! process killed, a network share that went away - leaves the author with a
//! shorter file than they started with and no copy of the old one. For a
//! scenario somebody has spent an evening on, that is the worst outcome the
//! editor can produce.
//!
//! Writing a temporary file beside the target and renaming over it moves the
//! failure to a place where nothing is lost: either the rename happens and the
//! new file is whole, or it does not and the old one is untouched.

use std::path::Path;

/// Write `bytes` to `path`, through a temporary file in the same directory.
///
/// The same directory on purpose: a rename is only atomic within a filesystem,
/// and a temporary directory is frequently on another one.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("splaunch-part");
    std::fs::write(&temp, bytes)
        .map_err(|e| format!("could not write {}: {e}", temp.display()))?;
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Nothing has touched the original, so the only thing left over is
            // ours to take away.
            let _ = std::fs::remove_file(&temp);
            Err(format!("could not write {}: {e}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("splaunch-savefile-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_save_replaces_the_file_and_leaves_nothing_beside_it() {
        let dir = temp_dir("replace");
        let path = dir.join("mission.splaunch");
        std::fs::write(&path, b"the old scenario").unwrap();

        write(&path, b"the new scenario").expect("saves");

        assert_eq!(std::fs::read(&path).unwrap(), b"the new scenario");
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["mission.splaunch".to_string()], "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole point: a save that cannot finish must not eat the old file.
    #[test]
    fn a_save_that_fails_leaves_the_original_alone() {
        let dir = temp_dir("keep");
        let path = dir.join("mission.splaunch");
        std::fs::write(&path, b"an evening of work").unwrap();

        /* A directory where the temporary file wants to be: creating it fails,
           which is the same shape as a full disk or a revoked permission, and
           is the one such failure a test can arrange on either platform. */
        std::fs::create_dir(path.with_extension("splaunch-part")).unwrap();

        assert!(write(&path, b"half a").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"an evening of work");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
