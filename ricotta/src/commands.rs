use anyhow::{Result, anyhow, bail};
use csv::WriterBuilder;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    CollectionIndex, Commit, DatasetIndex, Repo, default_types_for_headers,
    encode, parse_dataset_key, read_csv, validate_schema, verify_sorted,
};

fn current_unix_seconds() -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock before UNIX epoch: {err}"))?;
    Ok(now.as_secs())
}

fn put_dataset(
    repo: &Repo,
    key: String,
    headers: Vec<String>,
    columns: Vec<Vec<String>>,
) -> Result<Vec<(String, String)>> {
    let dataset_key = parse_dataset_key(&key)?;

    if let Some(first_col) = columns.first() {
        verify_sorted(first_col)?;
    }

    let mut index = repo.read_dataset_index()?.unwrap_or(DatasetIndex {
        name: Vec::new(),
        collection_name: Vec::new(),
        commit_hash: Vec::new(),
    });

    let collections = repo.read_collection_index()?.ok_or_else(|| {
        anyhow!("collections not found; create a collection first")
    })?;

    let collection_name = dataset_key.collection.clone();
    let collection_pos = collections
        .name
        .binary_search(&collection_name)
        .map_err(|_| anyhow!("collection not found: {collection_name}"))?;

    if collections.headers[collection_pos] != headers {
        bail!(
            "headers mismatch for collection {collection_name}: expected {:?}, got {:?}",
            collections.headers[collection_pos],
            headers
        );
    }

    let mut results = Vec::new();
    let mut col_hashes = Vec::new();

    for (i, col) in columns.iter().enumerate() {
        let raw = encode(col);
        let hex = repo.write_encoded_column(&raw)?;

        println!("{} -> {}", headers[i], hex);
        results.push((headers[i].clone(), hex.clone()));
        col_hashes.push(hex);
    }

    let index_key = dataset_key.as_index_key();
    let now = current_unix_seconds()?;

    let min_value = columns
        .first()
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or_default();
    let max_value = columns
        .first()
        .and_then(|c| c.last())
        .cloned()
        .unwrap_or_default();

    match index.name.binary_search(&index_key) {
        Ok(pos) => {
            let mut commit = repo.read_commit(&index.commit_hash[pos])?;
            if let Some(prev_max) = commit.max_value.last() {
                if !min_value.is_empty() && prev_max >= &min_value {
                    bail!(
                        "first column range overlaps or is unsorted across commits: previous max {:?} >= new min {:?}",
                        prev_max,
                        min_value
                    );
                }
            }

            commit.updated_at.push(now);
            commit.min_value.push(min_value);
            commit.max_value.push(max_value);
            commit.hashes.push(col_hashes);
            let commit_hash = repo.write_commit(&commit)?;

            index.collection_name[pos] = collection_name;
            index.commit_hash[pos] = commit_hash;
        }
        Err(pos) => {
            let commit = Commit {
                updated_at: vec![now],
                min_value: vec![min_value],
                max_value: vec![max_value],
                hashes: vec![col_hashes],
            };
            let commit_hash = repo.write_commit(&commit)?;

            index.name.insert(pos, index_key);
            index.collection_name.insert(pos, collection_name);
            index.commit_hash.insert(pos, commit_hash);
        }
    }
    repo.write_dataset_index(&index)?;

    Ok(results)
}

pub fn put_from_reader<R: Read>(
    reader: R,
    name: &str,
) -> Result<Vec<(String, String)>> {
    put_from_reader_in_repo(&Repo::default(), reader, name)
}

pub fn put_from_reader_in_repo<R: Read>(
    repo: &Repo,
    reader: R,
    name: &str,
) -> Result<Vec<(String, String)>> {
    let (headers, columns) = read_csv(reader)?;
    put_dataset(repo, name.to_string(), headers, columns)
}

pub fn list_datasets() -> Result<Vec<String>> {
    list_datasets_in_repo(&Repo::default())
}

pub fn list_datasets_in_repo(repo: &Repo) -> Result<Vec<String>> {
    let index = repo.read_dataset_index()?;
    Ok(index.map_or(Vec::new(), |idx| idx.name))
}

pub fn list_collections() -> Result<Vec<String>> {
    list_collections_in_repo(&Repo::default())
}

pub fn list_collections_in_repo(repo: &Repo) -> Result<Vec<String>> {
    let index = repo.read_collection_index()?;
    Ok(index.map_or(Vec::new(), |idx| idx.name))
}

pub fn list_datasets_in_collection(collection: &str) -> Result<Vec<String>> {
    list_datasets_in_collection_in_repo(&Repo::default(), collection)
}

pub fn list_datasets_in_collection_in_repo(
    repo: &Repo,
    collection: &str,
) -> Result<Vec<String>> {
    if collection.is_empty() {
        bail!("collection name cannot be empty");
    }

    let index = repo.read_dataset_index()?;
    let Some(index) = index else {
        return Ok(Vec::new());
    };

    let prefix = format!("{collection}/");
    let datasets = index
        .name
        .iter()
        .filter_map(|name| {
            name.strip_prefix(&prefix)
                .map(std::string::ToString::to_string)
        })
        .collect();

    Ok(datasets)
}

pub fn get_to_writer<W: Write>(name: &str, writer: W) -> Result<()> {
    get_to_writer_in_repo(&Repo::default(), name, writer)
}

pub fn get_to_writer_in_repo<W: Write>(
    repo: &Repo,
    name: &str,
    writer: W,
) -> Result<()> {
    let dataset_key = parse_dataset_key(name)?;

    let index = repo
        .read_dataset_index()?
        .ok_or_else(|| anyhow!("index not found; nothing to get"))?;
    let index_key = dataset_key.as_index_key();
    let pos = index
        .name
        .binary_search(&index_key)
        .map_err(|_| anyhow!("dataset not found: {name}"))?;

    let collection_name = &index.collection_name[pos];
    if collection_name != &dataset_key.collection {
        bail!(
            "dataset index mismatch for {name}: expected collection {}, got {}",
            dataset_key.collection,
            collection_name
        );
    }

    let collections = repo.read_collection_index()?.ok_or_else(|| {
        anyhow!("collections not found; cannot resolve schema")
    })?;
    let collection_pos = collections
        .name
        .binary_search(collection_name)
        .map_err(|_| anyhow!("collection not found: {collection_name}"))?;
    let headers = &collections.headers[collection_pos];

    let commit = repo.read_commit(&index.commit_hash[pos])?;
    let mut columns: Vec<Vec<String>> = Vec::new();
    for hashes in &commit.hashes {
        if hashes.len() != headers.len() {
            bail!(
                "commit row has {} columns but schema expects {}",
                hashes.len(),
                headers.len()
            );
        }

        let mut row_columns: Vec<Vec<String>> =
            Vec::with_capacity(hashes.len());
        for hex in hashes {
            let col: Vec<String> = repo.decode_column(hex)?;
            row_columns.push(col);
        }

        let nrows = row_columns.first().map_or(0, Vec::len);
        for col in &row_columns {
            if col.len() != nrows {
                bail!("stored columns have inconsistent lengths");
            }
        }

        if columns.is_empty() {
            columns = vec![Vec::new(); row_columns.len()];
        }
        for (i, col) in row_columns.into_iter().enumerate() {
            columns[i].extend(col);
        }
    }

    let nrows = columns.first().map_or(0, Vec::len);
    for col in &columns {
        if col.len() != nrows {
            bail!("stored columns have inconsistent lengths");
        }
    }

    let mut wtr = WriterBuilder::new().from_writer(writer);
    wtr.write_record(headers)?;
    for row in 0..nrows {
        let rec: Vec<&str> = columns.iter().map(|c| c[row].as_str()).collect();
        wtr.write_record(rec)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn create_collection(name: &str, headers: Vec<String>) -> Result<()> {
    create_collection_in_repo(&Repo::default(), name, headers)
}

pub fn create_collection_in_repo(
    repo: &Repo,
    name: &str,
    headers: Vec<String>,
) -> Result<()> {
    if name.is_empty() {
        bail!("collection name cannot be empty");
    }
    if headers.is_empty() {
        bail!("collection must define at least one header");
    }

    let types = default_types_for_headers(&headers);
    validate_schema(&headers, &types)?;

    let mut index = repo.read_collection_index()?.unwrap_or(CollectionIndex {
        name: Vec::new(),
        headers: Vec::new(),
        types: Vec::new(),
    });

    match index.name.binary_search(&name.to_string()) {
        Ok(_) => bail!("collection already exists: {name}"),
        Err(pos) => {
            index.name.insert(pos, name.to_string());
            index.headers.insert(pos, headers);
            index.types.insert(pos, types);
        }
    }

    repo.write_collection_index(&index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_temp_csv(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    struct TestRepo {
        _dir: tempfile::TempDir,
        repo: Repo,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = tempfile::Builder::new()
                .prefix(".risotto-")
                .tempdir_in(".")
                .unwrap();
            let repo = Repo::new(dir.path());
            Self { _dir: dir, repo }
        }

        fn repo(&self) -> &Repo {
            &self.repo
        }
    }

    #[test]
    fn test_process_integration() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        let csv = "name,age\nAlice,30\nBob,25\n";
        let f = write_temp_csv(csv);
        let results = put_from_reader_in_repo(
            repo,
            fs::File::open(f.path()).unwrap(),
            "users_collection/users",
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "name");
        assert_eq!(results[1].0, "age");

        for (_, hex) in &results {
            let out = repo.column_path(hex);
            assert!(out.exists(), "output file {out:?} should exist");
        }

        let index_path = repo.dataset_index_path();
        assert!(index_path.exists(), "index should exist");

        let csv_append = "name,age\nCharlie,35\nDiana,40\n";
        let f2 = write_temp_csv(csv_append);
        let results2 = put_from_reader_in_repo(
            repo,
            std::fs::File::open(f2.path()).unwrap(),
            "users_collection/users",
        )
        .unwrap();
        assert_eq!(results2.len(), 2);

        let csv2 = "name,age\nEve,45\nFrank,50\n";
        let f3 = write_temp_csv(csv2);
        let results3 = put_from_reader_in_repo(
            repo,
            std::fs::File::open(f3.path()).unwrap(),
            "users_collection/users2",
        )
        .unwrap();
        assert_eq!(results3.len(), 2);

        let mut out = Vec::new();
        get_to_writer_in_repo(repo, "users_collection/users", &mut out)
            .unwrap();
        let out_csv = String::from_utf8(out).unwrap();
        assert!(out_csv.contains("name,age"));
        assert!(out_csv.contains("Alice,30"));
        assert!(out_csv.contains("Bob,25"));
        assert!(out_csv.contains("Charlie,35"));
        assert!(out_csv.contains("Diana,40"));

        let dataset_index = repo.read_dataset_index().unwrap().unwrap();
        let pos = dataset_index
            .name
            .binary_search(&"users_collection/users".to_string())
            .unwrap();
        let commit = repo.read_commit(&dataset_index.commit_hash[pos]).unwrap();
        assert_eq!(commit.hashes.len(), 2);
    }

    #[test]
    fn test_process_unsorted_rejected() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();
        let csv = "name,age\nBob,25\nAlice,30\n";
        let f = write_temp_csv(csv);
        let result = put_from_reader_in_repo(
            repo,
            std::fs::File::open(f.path()).unwrap(),
            "users_collection/users",
        );
        assert!(result.is_err(), "unsorted first column should be rejected");
    }

    #[test]
    fn test_put_from_reader_requires_name_by_api() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();
        let csv = b"name,age\nAlice,30\nBob,25\n";
        let r = &csv[..];
        let res =
            put_from_reader_in_repo(repo, r, "users_collection/stdin_users");
        assert!(res.is_ok());
    }

    #[test]
    fn test_put_from_reader_requires_existing_collection() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        let csv = b"name,age\nAlice,30\nBob,25\n";
        let r = &csv[..];
        let err =
            put_from_reader_in_repo(repo, r, "users_collection/stdin_users")
                .unwrap_err();
        assert!(
            err.to_string()
                .contains("collections not found; create a collection first")
        );
    }

    #[test]
    fn test_put_from_reader_requires_namespaced_key() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        let csv = b"name,age\nAlice,30\nBob,25\n";
        let r = &csv[..];
        let err = put_from_reader_in_repo(repo, r, "users").unwrap_err();
        assert!(err.to_string().contains(
            "invalid dataset key \"users\"; expected <collection>/<dataset>"
        ));
    }

    #[test]
    fn test_put_from_reader_validates_headers_against_collection() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        let csv = b"name,score\nAlice,30\nBob,25\n";
        let r = &csv[..];
        let err = put_from_reader_in_repo(repo, r, "users_collection/users")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("headers mismatch for collection users_collection")
        );
    }

    #[test]
    fn test_put_from_reader_rejects_unsorted_across_commits() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        put_from_reader_in_repo(
            repo,
            b"name,age\nAlice,30\nBob,25\n".as_slice(),
            "users_collection/users",
        )
        .unwrap();

        let err = put_from_reader_in_repo(
            repo,
            b"name,age\nBob,41\nCarol,22\n".as_slice(),
            "users_collection/users",
        )
        .unwrap_err();

        assert!(err.to_string().contains(
            "first column range overlaps or is unsorted across commits"
        ));
    }

    #[test]
    fn test_get_to_writer_requires_namespaced_key() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        let csv = b"name,age\nAlice,30\nBob,25\n";
        put_from_reader_in_repo(repo, &csv[..], "users_collection/users")
            .unwrap();

        let mut out = Vec::new();
        let err = get_to_writer_in_repo(repo, "users", &mut out).unwrap_err();
        assert!(err.to_string().contains(
            "invalid dataset key \"users\"; expected <collection>/<dataset>"
        ));
    }

    #[test]
    fn test_get_to_writer_requires_collections_pointer() {
        let tr = TestRepo::new();
        let repo = tr.repo();
        create_collection_in_repo(
            repo,
            "users_collection",
            vec!["name".to_string(), "age".to_string()],
        )
        .unwrap();

        let csv = b"name,age\nAlice,30\nBob,25\n";
        put_from_reader_in_repo(repo, &csv[..], "users_collection/users")
            .unwrap();
        fs::remove_file(repo.collection_index_path()).unwrap();

        let mut out = Vec::new();
        let err =
            get_to_writer_in_repo(repo, "users_collection/users", &mut out)
                .unwrap_err();
        assert!(
            err.to_string()
                .contains("collections not found; cannot resolve schema")
        );
    }

    #[test]
    fn test_list_collections_and_datasets_in_collection() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        create_collection_in_repo(
            repo,
            "humidity",
            vec!["city".to_string(), "humidity".to_string()],
        )
        .unwrap();
        create_collection_in_repo(
            repo,
            "temperature",
            vec!["city".to_string(), "temp".to_string()],
        )
        .unwrap();

        put_from_reader_in_repo(
            repo,
            b"city,temp\nLondon,22\nParis,24\n".as_slice(),
            "temperature/europe",
        )
        .unwrap();
        put_from_reader_in_repo(
            repo,
            b"city,temp\nOsaka,30\nTokyo,29\n".as_slice(),
            "temperature/japan",
        )
        .unwrap();
        put_from_reader_in_repo(
            repo,
            b"city,humidity\nLondon,55\nParis,58\n".as_slice(),
            "humidity/europe",
        )
        .unwrap();

        let collections = list_collections_in_repo(repo).unwrap();
        assert_eq!(collections, vec!["humidity", "temperature"]);

        let temperature_sets =
            list_datasets_in_collection_in_repo(repo, "temperature").unwrap();
        assert_eq!(temperature_sets, vec!["europe", "japan"]);

        let humidity_sets =
            list_datasets_in_collection_in_repo(repo, "humidity").unwrap();
        assert_eq!(humidity_sets, vec!["europe"]);

        let none =
            list_datasets_in_collection_in_repo(repo, "unknown").unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_create_collection_roundtrip() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        create_collection_in_repo(
            repo,
            "temperature",
            vec!["city".to_string(), "temp".to_string()],
        )
        .unwrap();

        let collections = repo.read_collection_index().unwrap().unwrap();
        assert_eq!(collections.name, vec!["temperature".to_string()]);
        assert_eq!(
            collections.headers,
            vec![vec!["city".to_string(), "temp".to_string()]]
        );
        assert_eq!(
            collections.types,
            vec![vec!["string".to_string(), "string".to_string()]]
        );
    }

    #[test]
    fn test_create_collection_duplicate_rejected() {
        let tr = TestRepo::new();
        let repo = tr.repo();

        create_collection_in_repo(
            repo,
            "temperature",
            vec!["city".to_string()],
        )
        .unwrap();
        let err = create_collection_in_repo(
            repo,
            "temperature",
            vec!["other".to_string()],
        )
        .unwrap_err();
        assert!(err.to_string().contains("collection already exists"));
    }
}
