use crate::utils::grit_utils;
use chrono::{Date, Local};
use git2::{BlameOptions, Repository};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

pub struct ByFileArgs {
    repo_path: String,
    full_path_filename: String,
}

impl ByFileArgs {
    fn new(repo_path: String, full_path_filename: String) -> Self {
        ByFileArgs {
            repo_path,
            full_path_filename,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct ByFile {
    name: String,
    day: Date<Local>,
    loc: usize,
}

impl ByFile {
    fn new(name: String, day: Date<Local>) -> Self {
        ByFile { name, day, loc: 0 }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn by_file(args: ByFileArgs) -> GenResult<()> {
    let results = process_file(args);

    Ok(())
}

fn process_file(args: ByFileArgs) -> GenResult<Vec<ByFile>> {
    info!("Beginning to process file {}", args.full_path_filename);
    let start = Instant::now();

    let repo = Repository::open(args.repo_path)?;
    let mut bo = BlameOptions::new();

    let path = Path::new(args.full_path_filename.as_str());

    let blame = repo.blame_file(path, Some(&mut bo))?;
    let mut auth_to_loc: HashMap<ByFile, usize> = HashMap::new();

    for hunk in blame.iter() {
        let sig = hunk.final_signature();
        let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
        let commit = repo.find_commit(hunk.final_commit_id())?;
        let commit_date = grit_utils::convert_git_time(&commit.time());

        let key = ByFile::new(signame, commit_date);

        let v = match auth_to_loc.entry(key) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };
        *v += hunk.lines_in_hunk();
    }

    let results = auth_to_loc
        .iter()
        .map(|(k, v)| {
            let mut r = k.clone();
            r.loc = *v;
            r
        })
        .collect();

    info!(
        "Completed process_file for file {} in {:?}",
        args.full_path_filename,
        start.elapsed()
    );

    Ok(results)
}

#[cfg(test)]
mod tests {

    use super::*;
    use log::Level;
    use tempfile::TempDir;

    const LOG_LEVEL: Level = Level::Info;

    #[test]
    fn test_process_file() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "src/by_date.rs".to_string(),
        );

        let results: Vec<ByFile> = process_file(args).unwrap();

        assert!(results.len() > 0, "Results length was 0 len");

        info!("results: {:?}", results);
    }
}
