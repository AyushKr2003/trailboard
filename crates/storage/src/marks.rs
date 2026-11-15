//! Empty files in the data folder that remember a one-time notice was given
//! (for example, that pages run without Chromium's sandbox), so it isn't
//! repeated on every start.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// The sandbox is off for lack of support, and the user has been told.
pub const SANDBOX_OFF_TOLD: &str = "sandbox-off-told";

/// Sets mark `name`. True when it wasn't set before, so the notice is due now.
/// An error (an unwritable folder) counts as not due: better silent than
/// repeating it on every start.
pub fn first_time(data_dir: &Path, name: &str) -> bool {
    if fs::create_dir_all(data_dir).is_err() {
        return false;
    }
    match fs::OpenOptions::new().write(true).create_new(true).open(data_dir.join(name)) {
        Ok(_) => true,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => false,
        Err(e) => {
            log::warn!("marks: {name}: {e}");
            false
        }
    }
}

/// Clears mark `name`, so the notice is given again if its reason comes back.
pub fn clear(data_dir: &Path, name: &str) {
    let _ = fs::remove_file(data_dir.join(name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_until_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        assert!(first_time(&data, SANDBOX_OFF_TOLD), "creates the folder too");
        assert!(!first_time(&data, SANDBOX_OFF_TOLD));
        clear(&data, SANDBOX_OFF_TOLD);
        clear(&data, SANDBOX_OFF_TOLD); // twice is fine
        assert!(first_time(&data, SANDBOX_OFF_TOLD));
    }
}
