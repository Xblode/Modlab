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
//!
//! Writes are streamed and hashed in one pass (never load a whole file into
//! RAM — real Skyrim archives run into the gigabytes) and land via a temp
//! file + atomic rename, so a crash or a kill mid-import can never leave a
//! half-written blob claiming a hash it doesn't actually have.

use std::fs;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

const COPY_BUFFER_SIZE: usize = 64 * 1024;

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
    ///
    /// Streams the source in fixed-size chunks (hashing and writing each
    /// chunk as it's read) rather than buffering the whole file, and writes
    /// through a temp file that's only renamed into place — atomically, on
    /// the same volume — once the content and its hash are both known. A
    /// blob path either doesn't exist yet or is a complete, correct copy;
    /// there is no in-between state a reader could observe.
    pub fn put_file(&self, source: &Path) -> io::Result<String> {
        let mut reader = BufReader::new(fs::File::open(source)?);
        let mut tmp = tempfile::NamedTempFile::new_in(&self.root)?;
        let mut hasher = blake3::Hasher::new();

        let mut buf = [0u8; COPY_BUFFER_SIZE];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            tmp.write_all(&buf[..n])?;
        }

        let hash = hasher.finalize().to_hex().to_string();
        let dest = self.blob_path(&hash);
        if !dest.exists() {
            fs::create_dir_all(dest.parent().expect("blob path always has a parent"))?;
            tmp.persist(&dest).map_err(|e| e.error)?;
        }
        // If `dest` already exists, `tmp` is dropped here and cleans itself
        // up — we already have this content, the freshly-hashed copy was
        // redundant the moment the hash matched.
        Ok(hash)
    }

    /// Whether a blob with this hash is already in the store.
    #[must_use]
    pub fn contains(&self, hash: &str) -> bool {
        self.blob_path(hash).exists()
    }

    /// Path to a blob on disk, given its hash. Two-level sharding
    /// (`ab/abcdef...`) keeps any one directory from holding too many
    /// entries once a real modlist (thousands of files) lands in here.
    #[must_use]
    pub fn blob_path(&self, hash: &str) -> PathBuf {
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        self.root.join(prefix).join(rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
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

        assert_eq!(
            hash_a, hash_b,
            "identical content must hash the same and dedup"
        );
        assert_ne!(hash_a, hash_c);
        assert!(store.contains(&hash_a));
        assert!(store.blob_path(&hash_a).exists());
    }

    #[test]
    fn streams_content_spanning_multiple_buffer_chunks() {
        // 3.5x the copy buffer size, so put_file's read loop must run
        // several iterations — proves the streaming tee (hash + write per
        // chunk) reassembles identical content to a direct in-memory hash,
        // not just for single-chunk toy inputs.
        let content: Vec<u8> = (0..COPY_BUFFER_SIZE * 7 / 2)
            .map(|i| (i % 256) as u8)
            .collect();
        let expected_hash = blake3::hash(&content).to_hex().to_string();

        let store_dir = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let store = Store::open(store_dir.path()).unwrap();
        let file = write_temp_file(src_dir.path(), "big.bin", &content);

        let hash = store.put_file(&file).unwrap();

        assert_eq!(hash, expected_hash);
        let stored = fs::read(store.blob_path(&hash)).unwrap();
        assert_eq!(stored, content);
    }

    #[test]
    fn reimporting_identical_content_leaves_no_stray_temp_files() {
        let store_dir = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let store = Store::open(store_dir.path()).unwrap();
        let file = write_temp_file(src_dir.path(), "dup.txt", b"dedup me");

        store.put_file(&file).unwrap();
        store.put_file(&file).unwrap();

        // Only the two-level blob dir (e.g. "ab") should exist directly
        // under the store root — no leftover ".tmp*" siblings from the
        // discarded second temp file.
        let entries: Vec<_> = fs::read_dir(store_dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "expected only the sharded blob dir, found: {entries:?}"
        );
    }
}
