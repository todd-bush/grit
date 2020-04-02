use git2::{BlameOptions, Error, Repository, StatusOptions};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;

pub struct FameArgs {
    path: String,
    sort: Option<String>,
    threads: usize,
}

impl FameArgs {
    pub fn new(path: String, sort: Option<String>, threads: usize) -> Self {
        FameArgs {
            path,
            sort,
            threads,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct BlameOutput {
    author: String,
    commit_id: String,
    lines: usize,
}

impl BlameOutput {
    fn new(author: String, commit_id: String) -> Self {
        BlameOutput {
            author,
            commit_id,
            lines: 0,
        }
    }
}

pub fn process_fame(args: FameArgs) -> Result<(), Error> {
    let repo_path: &str = args.path.as_ref();
    let repo = Repository::open(repo_path)?;

    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(false);
    status_opts.include_unmodified(true);
    status_opts.include_ignored(false);
    status_opts.include_unreadable(false);
    status_opts.exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut status_opts))?;

    let file_names: Vec<String> = statuses
        .iter()
        .map(|se| se.path().unwrap().to_string())
        .collect();

    let mut per_file: HashMap<String, Vec<BlameOutput>> = HashMap::new();

    for file_name in file_names {
        let fne = &file_name;

        per_file.insert(fne.to_string(), process_file(repo_path, fne).unwrap());
    }

    Ok(())
}

fn process_file(repo_path: &str, file_name: &str) -> Result<Vec<BlameOutput>, Error> {
    let repo = Repository::open(repo_path)?;
    let mut bo = BlameOptions::new();
    let path = Path::new(file_name);
    let blame = repo.blame_file(path, Some(&mut bo))?;

    let mut blame_map: HashMap<BlameOutput, usize> = HashMap::new();

    for hunk in blame.iter() {
        let sig = hunk.final_signature();
        let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
        let file_blame = BlameOutput::new(signame, hunk.final_commit_id().to_string());

        let v = match blame_map.entry(file_blame) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };

        *v += hunk.lines_in_hunk();
    }

    let result: Vec<BlameOutput> = blame_map
        .iter()
        .map(|(k, v)| {
            let mut key = k.clone();
            key.lines = *v;
            key
        })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::time::Instant;

    #[test]
    fn test_process_file() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let start = Instant::now();

        let result = process_file(".", "README.md").unwrap();

        let duration = start.elapsed();

        println!("completed test_process_file in {:?}", duration);

        assert!(
            result.len() >= 9,
            "test_process_file result was {}",
            result.len()
        );
    }

    #[test]
    fn test_process_fame() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let args = FameArgs::new(".".to_string(), Some("loc".to_string()), 15);

        let start = Instant::now();

        let result = process_fame(args).unwrap();

        let duration = start.elapsed();

        println!("completed test_process_fame in {:?}", duration);
    }
}
