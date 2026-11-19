//! A file on its way to Linear's storage: held in a folder of its own that only this program can
//! enter, as a file only it can read, and gone when this is dropped. What is sent is always a copy
//! made here, so that what was looked at (its size, its first bytes) is what is sent, and so that the
//! terminal's handle or the person's path is not what `curl` is given.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static MADE: AtomicU64 = AtomicU64::new(0);

/// The folder and the name of the file that will be in it.
#[derive(Debug)]
pub struct Staged {
    folder: PathBuf,
    file: PathBuf,
}

/// Why a copy was not made.
#[derive(Debug, PartialEq, Eq)]
pub enum CopyFailed {
    /// There were more bytes than were allowed.
    TooLarge,
    /// The source or the place could not be read or written.
    Io,
}

impl Staged {
    /// A folder (mode 0700) under the temporary folder, named for this run and this moment; the file
    /// in it is not made yet.
    ///
    /// # Errors
    ///
    /// The folder could not be made.
    pub fn new() -> std::io::Result<Self> {
        let folder = std::env::temp_dir().join(format!(
            "linearpen-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&folder)?;
        let file = folder.join("file");
        Ok(Self { folder, file })
    }

    /// Where the file is, or will be.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.file
    }

    /// Takes from the file what any other has of it: the rights of a file that something else made in
    /// the folder (the terminal's receiving) are made 0600.
    ///
    /// # Errors
    ///
    /// The file is not there.
    pub fn private(&self) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&self.file, fs::Permissions::from_mode(0o600))
    }

    /// Makes the file (mode 0600, and not over another) of what `from` gives, at most `most` bytes:
    /// how many bytes.
    ///
    /// # Errors
    ///
    /// `TooLarge` where `from` has more than `most` bytes (the file is then not kept), `Io` where it
    /// cannot be read or the copy cannot be written.
    pub fn copy_in(&self, from: &mut dyn Read, most: u64) -> Result<u64, CopyFailed> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.file)
            .map_err(|_| CopyFailed::Io)?;
        let mut size: u64 = 0;
        let mut piece = vec![0u8; 1 << 16];
        loop {
            let got = from.read(&mut piece).map_err(|_| CopyFailed::Io)?;
            if got == 0 {
                break;
            }
            size += got as u64;
            if size > most {
                return Err(CopyFailed::TooLarge);
            }
            out.write_all(&piece[..got]).map_err(|_| CopyFailed::Io)?;
        }
        out.flush().map_err(|_| CopyFailed::Io)?;
        Ok(size)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_copy_is_made_where_only_this_program_can_look_and_is_gone_with_it() {
        let staged = Staged::new().unwrap();
        let folder = staged.path().parent().unwrap().to_owned();
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o700);
        let copied = staged
            .copy_in(&mut &b"%PDF-1.7 and more words"[..], 100)
            .unwrap();
        assert_eq!(copied, 23);
        assert_eq!(mode(staged.path()), 0o600);
        assert_eq!(fs::read(staged.path()).unwrap(), b"%PDF-1.7 and more words");
        drop(staged);
        assert!(!folder.exists());
    }

    #[test]
    fn more_than_is_allowed_is_not_kept_and_a_name_is_not_used_twice() {
        let staged = Staged::new().unwrap();
        assert_eq!(
            staged.copy_in(&mut &[0u8; 11][..], 10).unwrap_err(),
            CopyFailed::TooLarge
        );
        let other = Staged::new().unwrap();
        assert_ne!(staged.path(), other.path());
        // The file is made once: a second copy into the same place is refused, not written over.
        let again = other.copy_in(&mut &b"a"[..], 10);
        assert!(again.is_ok());
        assert_eq!(
            other.copy_in(&mut &b"b"[..], 10).unwrap_err(),
            CopyFailed::Io
        );
        assert_eq!(fs::read(other.path()).unwrap(), b"a");
    }
}
