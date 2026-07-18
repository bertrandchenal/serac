use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    CollectionIndex, Commit, DatasetIndex, Validate, compress_raw, encode,
    hash_raw,
};

pub const DEFAULT_REPO_ROOT: &str = ".risotto";

#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
}

impl Repo {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn column_path(&self, hex: &str) -> PathBuf {
        let (a, b) = (&hex[..2], &hex[2..4]);
        let rest = &hex[4..];
        self.root.join(a).join(b).join(rest)
    }

    pub fn dataset_index_path(&self) -> PathBuf {
        self.root.join("index")
    }

    pub fn collection_index_path(&self) -> PathBuf {
        self.root.join("collections")
    }

    pub(crate) fn decode_column<T: serde::de::DeserializeOwned>(
        &self,
        hex: &str,
    ) -> Result<T> {
        let path = self.column_path(hex);
        log::debug!("read: {}", path.display());
        let raw = zstd::decode_all(&*fs::read(path)?)?;
        Ok(bincode::deserialize(&raw)?)
    }

    pub(crate) fn write_encoded_column(&self, raw: &[u8]) -> Result<String> {
        let hex = hash_raw(raw);
        let out = self.column_path(&hex);
        if !out.exists() {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("creating directory {:?}", parent)
                })?;
            }
            log::debug!("write: {}", out.display());
            fs::write(&out, compress_raw(raw)?)
                .with_context(|| format!("writing column {hex}"))?;
        }
        Ok(hex)
    }

    pub fn read_dataset_index(&self) -> Result<Option<DatasetIndex>> {
        let p = self.dataset_index_path();
        if !p.exists() {
            return Ok(None);
        }
        log::debug!("read: {}", p.display());
        let content = fs::read_to_string(&p)?;
        let mut lines = content.trim().lines();
        let (Some(name_hash), Some(collections_hash), Some(commits_hash), None) =
            (lines.next(), lines.next(), lines.next(), lines.next())
        else {
            bail!("invalid index file: expected three hex hashes");
        };

        let name: Vec<String> = self.decode_column(name_hash)?;
        let collection_name: Vec<String> =
            self.decode_column(collections_hash)?;
        let commit_hash: Vec<String> = self.decode_column(commits_hash)?;

        Ok(Some(DatasetIndex {
            name,
            collection_name,
            commit_hash,
        }))
    }

    pub fn write_dataset_index(&self, index: &DatasetIndex) -> Result<()> {
        index.validate()?;

        let names_hex = self.write_encoded_column(&encode(&index.name))?;
        let collections_hex =
            self.write_encoded_column(&encode(&index.collection_name))?;
        let commits_hex =
            self.write_encoded_column(&encode(&index.commit_hash))?;

        let index_file = self.dataset_index_path();
        if let Some(parent) = index_file.parent() {
            fs::create_dir_all(parent)?;
        }
        log::debug!("write: {}", index_file.display());
        fs::write(
            index_file,
            format!("{names_hex}\n{collections_hex}\n{commits_hex}\n"),
        )?;

        Ok(())
    }

    pub fn read_collection_index(&self) -> Result<Option<CollectionIndex>> {
        let p = self.collection_index_path();
        if !p.exists() {
            return Ok(None);
        }
        log::debug!("read: {}", p.display());
        let content = fs::read_to_string(&p)?;
        let mut lines = content.trim().lines();
        let (Some(name_hash), Some(headers_hash), Some(types_hash), None) =
            (lines.next(), lines.next(), lines.next(), lines.next())
        else {
            bail!("invalid collections file: expected three hex hashes");
        };

        let name: Vec<String> = self.decode_column(name_hash)?;
        let headers: Vec<Vec<String>> = self.decode_column(headers_hash)?;
        let types: Vec<Vec<String>> = self.decode_column(types_hash)?;

        Ok(Some(CollectionIndex {
            name,
            headers,
            types,
        }))
    }

    pub fn write_collection_index(
        &self,
        index: &CollectionIndex,
    ) -> Result<()> {
        index.validate()?;

        let names_hex = self.write_encoded_column(&encode(&index.name))?;
        let headers_hex = self.write_encoded_column(&encode(&index.headers))?;
        let types_hex = self.write_encoded_column(&encode(&index.types))?;

        let collections_file = self.collection_index_path();
        if let Some(parent) = collections_file.parent() {
            fs::create_dir_all(parent)?;
        }
        log::debug!("write: {}", collections_file.display());
        fs::write(
            collections_file,
            format!("{names_hex}\n{headers_hex}\n{types_hex}\n"),
        )?;

        Ok(())
    }

    pub fn dataset_index(&self) -> Result<Option<DatasetIndex>> {
        self.read_dataset_index()
    }

    pub fn collection_index(&self) -> Result<Option<CollectionIndex>> {
        self.read_collection_index()
    }

    pub fn read_commit(&self, commit_hash: &str) -> Result<Commit> {
        let content = self.decode_column::<Vec<String>>(commit_hash)?;
        if content.len() != 4 {
            bail!("invalid commit file: expected four column hashes");
        }

        let updated_at: Vec<u64> = self.decode_column(&content[0])?;
        let min_value: Vec<String> = self.decode_column(&content[1])?;
        let max_value: Vec<String> = self.decode_column(&content[2])?;
        let hashes: Vec<Vec<String>> = self.decode_column(&content[3])?;

        let commit = Commit {
            updated_at,
            min_value,
            max_value,
            hashes,
        };
        commit.validate()?;
        Ok(commit)
    }

    pub fn write_commit(&self, commit: &Commit) -> Result<String> {
        commit.validate()?;

        let updated_at_hex =
            self.write_encoded_column(&encode(&commit.updated_at))?;
        let min_values_hex =
            self.write_encoded_column(&encode(&commit.min_value))?;
        let max_values_hex =
            self.write_encoded_column(&encode(&commit.max_value))?;
        let hashes_hex = self.write_encoded_column(&encode(&commit.hashes))?;

        let commit_pointer =
            vec![updated_at_hex, min_values_hex, max_values_hex, hashes_hex];
        let commit_raw = encode(&commit_pointer);
        self.write_encoded_column(&commit_raw)
    }
}

impl Default for Repo {
    fn default() -> Self {
        Self::new(DEFAULT_REPO_ROOT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn fs_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_fs() -> MutexGuard<'static, ()> {
        match fs_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    struct TestRepo {
        _lock: MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
        repo: Repo,
    }

    impl TestRepo {
        fn new() -> Self {
            let lock = lock_fs();
            let dir = tempfile::Builder::new()
                .prefix(".risotto-")
                .tempdir_in(".")
                .unwrap();
            let repo = Repo::new(dir.path());
            Self {
                _lock: lock,
                _dir: dir,
                repo,
            }
        }

        fn repo(&self) -> &Repo {
            &self.repo
        }
    }

    #[test]
    fn test_column_path_format() {
        let repo = Repo::default();
        let hash =
            "2e21fa057f7c3d37d8b29c18ab51c0952d72589d6e03ecfd4b77b4cfb7ac00bd";
        let path = repo.column_path(hash);
        assert_eq!(
            path,
            std::path::PathBuf::from(
                ".risotto/2e/21/fa057f7c3d37d8b29c18ab51c0952d72589d6e03ecfd4b77b4cfb7ac00bd"
            )
        );
    }

    #[test]
    fn test_write_dataset_index_validates_at_write_time() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        let invalid = DatasetIndex {
            name: vec!["z".to_string(), "a".to_string()],
            collection_name: vec!["c".to_string(), "c".to_string()],
            commit_hash: vec!["x".to_string(), "y".to_string()],
        };

        assert!(repo.write_dataset_index(&invalid).is_err());
    }

    #[test]
    fn test_collection_index_write_read_roundtrip() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        let index = CollectionIndex {
            name: vec!["humidity".to_string(), "temperature".to_string()],
            headers: vec![
                vec!["city".to_string(), "humidity".to_string()],
                vec!["city".to_string(), "temp".to_string()],
            ],
            types: vec![
                vec!["string".to_string(), "string".to_string()],
                vec!["string".to_string(), "string".to_string()],
            ],
        };

        repo.write_collection_index(&index).unwrap();
        let loaded = repo.read_collection_index().unwrap().unwrap();
        assert_eq!(loaded, index);
    }

    #[test]
    fn test_collection_index_read_none_when_missing() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        assert!(repo.read_collection_index().unwrap().is_none());
    }

    #[test]
    fn test_collection_index_read_invalid_pointer_file() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        fs::create_dir_all(repo.root()).unwrap();
        fs::write(repo.collection_index_path(), "only-one-line\n").unwrap();

        let err = repo.read_collection_index().unwrap_err();
        assert!(
            err.to_string().contains(
                "invalid collections file: expected three hex hashes"
            )
        );
    }

    #[test]
    fn test_commit_write_read_roundtrip() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        let commit = Commit {
            updated_at: vec![1, 2],
            min_value: vec!["a".to_string(), "c".to_string()],
            max_value: vec!["b".to_string(), "d".to_string()],
            hashes: vec![
                vec!["h1".to_string(), "h2".to_string()],
                vec!["h3".to_string(), "h4".to_string()],
            ],
        };

        let commit_hash = repo.write_commit(&commit).unwrap();
        let loaded = repo.read_commit(&commit_hash).unwrap();
        assert_eq!(loaded, commit);
    }
}
