mod commands;
mod repo;

use anyhow::{Context, Result, bail};
use csv::ReaderBuilder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;

pub use commands::*;
pub use repo::{DEFAULT_REPO_ROOT, Repo};

trait Validate {
    fn validate(&self) -> Result<()>;
}

fn validate_same_len(
    left_name: &str,
    left_len: usize,
    right_name: &str,
    right_len: usize,
    context: &str,
) -> Result<()> {
    if left_len != right_len {
        bail!(
            "invalid {context}: {left_name}/{right_name} length mismatch: {left_len} != {right_len}"
        );
    }
    Ok(())
}

fn validate_vecvec_pair_lengths(
    left_name: &str,
    left: &[Vec<String>],
    right_name: &str,
    right: &[Vec<String>],
    context: &str,
) -> Result<()> {
    validate_same_len(left_name, left.len(), right_name, right.len(), context)?;

    for (i, (l, r)) in left.iter().zip(right.iter()).enumerate() {
        validate_same_len(
            left_name,
            l.len(),
            right_name,
            r.len(),
            &format!("{context} at position {i}"),
        )?;
    }

    Ok(())
}

pub const STRING_COLUMN_TYPE: &str = "string";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetKey {
    pub collection: String,
    pub dataset: String,
}

impl DatasetKey {
    pub fn as_index_key(&self) -> String {
        format!("{}/{}", self.collection, self.dataset)
    }
}

pub fn parse_dataset_key(input: &str) -> Result<DatasetKey> {
    let mut parts = input.split('/');
    let collection = parts.next().unwrap_or("");
    let dataset = parts.next().unwrap_or("");

    if collection.is_empty() || dataset.is_empty() || parts.next().is_some() {
        bail!("invalid dataset key {input:?}; expected <collection>/<dataset>");
    }

    Ok(DatasetKey {
        collection: collection.to_string(),
        dataset: dataset.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionIndex {
    pub name: Vec<String>,
    pub headers: Vec<Vec<String>>,
    pub types: Vec<Vec<String>>,
}

impl Validate for CollectionIndex {
    fn validate(&self) -> Result<()> {
        validate_same_len(
            "names",
            self.name.len(),
            "headers",
            self.headers.len(),
            "collection index",
        )?;
        validate_same_len(
            "names",
            self.name.len(),
            "types",
            self.types.len(),
            "collection index",
        )?;
        validate_vecvec_pair_lengths(
            "headers",
            &self.headers,
            "types",
            &self.types,
            "collection index",
        )?;

        for names in self.name.windows(2) {
            if names[0] >= names[1] {
                bail!(
                    "invalid collection index: names are not strictly sorted"
                );
            }
        }

        for (i, (headers, types)) in
            self.headers.iter().zip(self.types.iter()).enumerate()
        {
            validate_schema(headers, types).with_context(|| {
                format!("invalid schema for collection {}", self.name[i])
            })?;
        }

        Ok(())
    }
}

pub fn default_types_for_headers(headers: &[String]) -> Vec<String> {
    headers
        .iter()
        .map(|_| STRING_COLUMN_TYPE.to_string())
        .collect()
}

pub fn validate_schema(headers: &[String], types: &[String]) -> Result<()> {
    if headers.len() != types.len() {
        bail!(
            "invalid schema: headers/types length mismatch: {} != {}",
            headers.len(),
            types.len()
        );
    }

    for ty in types {
        if ty != STRING_COLUMN_TYPE {
            bail!(
                "unsupported column type {ty:?}; only {STRING_COLUMN_TYPE:?} is supported"
            );
        }
    }

    Ok(())
}

pub(crate) fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).expect("bincode serialization is infallible")
}

pub fn read_csv<R: Read>(reader: R) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(reader);

    let headers: Vec<String> =
        reader.headers()?.iter().map(String::from).collect();
    let ncols = headers.len();

    let mut columns: Vec<Vec<String>> = vec![Vec::new(); ncols];

    for result in reader.records() {
        let record = result?;
        for (i, field) in record.iter().enumerate() {
            columns[i].push(field.to_string());
        }
    }

    Ok((headers, columns))
}

pub fn hash_raw(raw: &[u8]) -> String {
    let hash = Sha256::digest(raw);
    hex::encode(hash)
}

pub fn compress_raw(raw: &[u8]) -> Result<Vec<u8>> {
    zstd::encode_all(raw, 0).context("compressing column")
}

pub fn verify_sorted(values: &[String]) -> Result<()> {
    for w in values.windows(2) {
        if w[0] >= w[1] {
            bail!("first column is not sorted: {:?} >= {:?}", w[0], w[1]);
        }
    }
    Ok(())
}

pub struct DatasetIndex {
    pub name: Vec<String>,
    pub collection_name: Vec<String>,
    pub commit_hash: Vec<String>,
}

impl Validate for DatasetIndex {
    fn validate(&self) -> Result<()> {
        validate_same_len(
            "names",
            self.name.len(),
            "collection_name",
            self.collection_name.len(),
            "dataset index",
        )?;
        validate_same_len(
            "names",
            self.name.len(),
            "commit_hash",
            self.commit_hash.len(),
            "dataset index",
        )?;

        for names in self.name.windows(2) {
            if names[0] >= names[1] {
                bail!("invalid dataset index: names are not strictly sorted");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub updated_at: Vec<u64>,
    pub min_value: Vec<String>,
    pub max_value: Vec<String>,
    pub hashes: Vec<Vec<String>>,
}

impl Validate for Commit {
    fn validate(&self) -> Result<()> {
        validate_same_len(
            "updated_at",
            self.updated_at.len(),
            "min_value",
            self.min_value.len(),
            "commit log",
        )?;
        validate_same_len(
            "updated_at",
            self.updated_at.len(),
            "max_value",
            self.max_value.len(),
            "commit log",
        )?;
        validate_same_len(
            "updated_at",
            self.updated_at.len(),
            "hashes",
            self.hashes.len(),
            "commit log",
        )?;

        for times in self.updated_at.windows(2) {
            if times[0] > times[1] {
                bail!("invalid commit log: updated_at is not sorted");
            }
        }

        Ok(())
    }
}

pub fn read_collection_index() -> Result<Option<CollectionIndex>> {
    read_collection_index_in_repo(&Repo::default())
}

pub fn read_collection_index_in_repo(
    repo: &Repo,
) -> Result<Option<CollectionIndex>> {
    repo.read_collection_index()
}

pub fn write_collection_index(index: &CollectionIndex) -> Result<()> {
    write_collection_index_in_repo(&Repo::default(), index)
}

pub fn write_collection_index_in_repo(
    repo: &Repo,
    index: &CollectionIndex,
) -> Result<()> {
    repo.write_collection_index(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_temp_csv(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    #[test]
    fn test_hash_raw_known_values() {
        let raw = bincode::serialize(&vec!["alice", "bob", "charlie"]).unwrap();
        assert_eq!(
            hash_raw(&raw),
            "e6f760eb1843d216ce6faf11feb1ded96b1bb716d4496596f7dad83eee902c06"
        );

        let ages_raw = bincode::serialize(&vec!["30", "25", "35"]).unwrap();
        assert_eq!(
            hash_raw(&ages_raw),
            "116d6f8e7545a418b4576a95b0fe1e72091cfa9842f885904b0059bd2810bd14"
        );
    }

    #[test]
    fn test_compress_roundtrip() {
        let col: Vec<String> = vec!["foo".into(), "bar".into(), "baz".into()];
        let raw = bincode::serialize(&col).unwrap();
        let compressed = compress_raw(&raw).unwrap();
        let decompressed: Vec<String> =
            bincode::deserialize(&zstd::decode_all(&compressed[..]).unwrap())
                .unwrap();
        assert_eq!(decompressed, col);
    }

    #[test]
    fn test_read_csv_basic() {
        let csv = "name,age\nAlice,30\nBob,25\n";
        let f = write_temp_csv(csv);
        let (headers, columns) =
            read_csv(fs::File::open(f.path()).unwrap()).unwrap();
        assert_eq!(headers, vec!["name", "age"]);
        assert_eq!(
            columns,
            vec![
                vec!["Alice".to_string(), "Bob".to_string()],
                vec!["30".to_string(), "25".to_string()],
            ]
        );
    }

    #[test]
    fn test_read_csv_single_column() {
        let csv = "x\na\nb\nc\n";
        let f = write_temp_csv(csv);
        let (headers, columns) =
            read_csv(fs::File::open(f.path()).unwrap()).unwrap();
        assert_eq!(headers, vec!["x"]);
        assert_eq!(
            columns,
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn test_read_csv_single_row() {
        let csv = "k,v\n1,2\n";
        let f = write_temp_csv(csv);
        let (_headers, columns) =
            read_csv(fs::File::open(f.path()).unwrap()).unwrap();
        assert_eq!(columns[0], vec!["1"]);
        assert_eq!(columns[1], vec!["2"]);
    }

    #[test]
    fn test_verify_sorted_valid() {
        assert!(verify_sorted(&["a".into(), "b".into(), "c".into()]).is_ok());
        assert!(verify_sorted(&["x".into()]).is_ok());
        assert!(verify_sorted(&[] as &[String]).is_ok());
    }

    #[test]
    fn test_verify_sorted_invalid() {
        assert!(verify_sorted(&["b".into(), "a".into()]).is_err());
        assert!(verify_sorted(&["a".into(), "a".into()]).is_err());
    }

    #[test]
    fn test_parse_dataset_key_valid() {
        let key = parse_dataset_key("temperature/london").unwrap();
        assert_eq!(key.collection, "temperature");
        assert_eq!(key.dataset, "london");
        assert_eq!(key.as_index_key(), "temperature/london");
    }

    #[test]
    fn test_parse_dataset_key_invalid() {
        assert!(parse_dataset_key("temperature").is_err());
        assert!(parse_dataset_key("temperature/").is_err());
        assert!(parse_dataset_key("/london").is_err());
        assert!(parse_dataset_key("temperature/london/2024").is_err());
        assert!(parse_dataset_key("").is_err());
    }

    #[test]
    fn test_default_types_for_headers() {
        let headers = vec!["city".to_string(), "temp".to_string()];
        let types = default_types_for_headers(&headers);
        assert_eq!(types, vec!["string".to_string(), "string".to_string()]);
    }

    #[test]
    fn test_validate_schema() {
        assert!(
            validate_schema(
                &["city".to_string(), "temp".to_string()],
                &["string".to_string(), "string".to_string()]
            )
            .is_ok()
        );

        assert!(validate_schema(&["city".to_string()], &[]).is_err());
        assert!(
            validate_schema(&["city".to_string()], &["int".to_string()])
                .is_err()
        );
    }

    #[test]
    fn test_collection_index_validate() {
        let valid = CollectionIndex {
            name: vec!["temperature".to_string()],
            headers: vec![vec!["city".to_string(), "temp".to_string()]],
            types: vec![vec!["string".to_string(), "string".to_string()]],
        };
        assert!(valid.validate().is_ok());

        let invalid_len = CollectionIndex {
            name: vec!["temperature".to_string()],
            headers: vec![vec!["city".to_string()]],
            types: vec![],
        };
        assert!(invalid_len.validate().is_err());

        let invalid_type = CollectionIndex {
            name: vec!["temperature".to_string()],
            headers: vec![vec!["city".to_string()]],
            types: vec![vec!["float".to_string()]],
        };
        assert!(invalid_type.validate().is_err());
    }

    #[test]
    fn test_dataset_index_validate() {
        let valid = DatasetIndex {
            name: vec!["temperature/london".to_string()],
            collection_name: vec!["temperature".to_string()],
            commit_hash: vec!["a".to_string()],
        };
        assert!(valid.validate().is_ok());

        let invalid_len = DatasetIndex {
            name: vec!["temperature/london".to_string()],
            collection_name: vec![],
            commit_hash: vec![],
        };
        assert!(invalid_len.validate().is_err());
    }

    #[test]
    fn test_commit_validate() {
        let valid = Commit {
            updated_at: vec![1, 2],
            min_value: vec!["a".to_string(), "c".to_string()],
            max_value: vec!["b".to_string(), "d".to_string()],
            hashes: vec![vec!["h1".to_string()], vec!["h2".to_string()]],
        };
        assert!(valid.validate().is_ok());

        let invalid_len = Commit {
            updated_at: vec![1],
            min_value: vec![],
            max_value: vec![],
            hashes: vec![],
        };
        assert!(invalid_len.validate().is_err());

        let invalid_sort = Commit {
            updated_at: vec![2, 1],
            min_value: vec!["a".to_string(), "b".to_string()],
            max_value: vec!["a".to_string(), "b".to_string()],
            hashes: vec![vec!["h1".to_string()], vec!["h2".to_string()]],
        };
        assert!(invalid_sort.validate().is_err());
    }
}
