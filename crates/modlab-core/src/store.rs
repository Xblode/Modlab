//! Content-addressed store (CAS) — see docs/01-architecture-decidee.md §5.
//!
//! Every unique file content is stored exactly once, named by its hash.
//! Two mods that ship the same texture share one blob. This is the
//! foundation dedup, provenance and instant snapshots all build on later.
//!
//! Hashing choice: BLAKE3 for now (fast, single dependency, no unsafe FFI).
//! The architecture doc mentions xxHash3/xxHash64 for Wabbajack-manifest
//! compatibility — that can be layered on as an additional hash column if
//! that interop is ever actually needed; not worth the extra dependency
//! before there's a real reason for it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Store {
    root: PathBuf,
}

impl Store {
    /// `root` is the store's blob directory, e.g. `F:\ModLab\store\blobs`.
    /// Created if it doesn't exist yet.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Hash a file's content and copy it into the store if it isn't already
    /// there. Returns the content hash (hex-encoded BLAKE3), which is the
    /// blob's permanent identity — callers store this hash, not a path.
    pub fn put_file(&self, source: &Path) -> io::Result<String> {
        let bytes = fs::read(source)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();

        let dest = self.blob_path(&hash);
        if !dest.exists() {
            fs::create_dir_all(dest.parent().expect("blob path always has a parent"))?;
            fs::write(&dest, &bytes)?;
        }
        Ok(hash)
    }

    /// Whether a blob with this hash is already in the store.
    pub fn contains(&self, hash: &str) -> bool {
        self.blob_path(hash).exists()
    }

    /// Path to a blob on disk, given its hash. Two-level sharding
    /// (`ab/abcdef...`) keeps any one directory from holding too many
    /// entries once a real modlist (thousands of files) lands in here.
    pub fn blob_path(&self, hash: &str) -> PathBuf {
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        self.root.join(prefix).join(rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn stores_and_dedups_by_content() {
        let store_dir = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let store = Store::open(store_dir.path()).unwrap();

        let file_a = write_temp_file(src_dir.path(), "a.txt", b"same content");
        let file_b = write_temp_file(src_dir.path(), "b.txt", b"same content");
        let file_c = write_temp_file(src_dir.path(), "c.txt", b"different content");

        let hash_a = store.put_file(&file_a).unwrap();
        let hash_b = store.put_file(&file_b).unwrap();
        let hash_c = store.put_file(&file_c).unwrap();

        assert_eq!(hash_a, hash_b, "identical content must hash the same and dedup");
        assert_ne!(hash_a, hash_c);
        assert!(store.contains(&hash_a));
        assert!(store.blob_path(&hash_a).exists());
    }
}
