use super::Processable;
use crate::utils::grit_utils;
use anyhow::Result;
use chrono::{Date, Local};
use git2::{BlameOptions, Oid, Repository};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;

pub struct FameArgs {
    path: String,
    sort: Option<String>,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    include: Option<String>,
    exclude: Option<String>,
    restrict_authors: Option<String>,
}

impl FameArgs {
    pub fn new(
        path: String,
        sort: Option<String>,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        include: Option<String>,
        exclude: Option<String>,
        restrict_authors: Option<String>,
    ) -> FameArgs {
        FameArgs {
            path: path,
            sort: sort,
            start_date: start_date,
            end_date: end_date,
            include: include,
            exclude: exclude,
            restrict_authors: restrict_authors,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct BlameOutput {
    author: String,
    commit_id: String,
    lines: i32,
}

impl BlameOutput {
    fn new(author: String, commit_id: String) -> BlameOutput {
        BlameOutput {
            author: author,
            commit_id: commit_id,
            lines: 0,
        }
    }
}

pub struct Fame {
    args: FameArgs,
}

struct BlameProcessor {
    path: String,
    file_name: String,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
}

impl BlameProcessor {
    fn new(
        path: String,
        file_name: String,
        earliest_commit: Option<Vec<u8>>,
        latest_commit: Option<Vec<u8>>,
    ) -> BlameProcessor {
        BlameProcessor {
            path: path,
            file_name: file_name,
            earliest_commit: earliest_commit,
            latest_commit: latest_commit,
        }
    }

    fn process(&self) -> Result<Vec<BlameOutput>> {
        let repo = Repository::open(&self.path)?;
        let file_path = Path::new(&self.file_name);

        let mut bo = BlameOptions::new();

        if let Some(ev) = &self.earliest_commit {
            let oid: Oid = Oid::from_bytes(&ev)?;
            let commit = repo.find_commit(oid)?;
            bo.oldest_commit(commit.id());
        };

        if let Some(ov) = &self.latest_commit {
            let oid: Oid = Oid::from_bytes(&ov)?;
            let commit = repo.find_commit(oid)?;
            bo.newest_commit(commit.id());
        };

        let blame = repo.blame_file(file_path, Some(&mut bo))?;

        let mut blame_map: HashMap<BlameOutput, i32> = HashMap::new();

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let signame = String::from_utf8_lossy(sig.name_bytes());
            let file_blame =
                BlameOutput::new(signame.to_string(), hunk.final_commit_id().to_string());

            let v = match blame_map.entry(file_blame) {
                Vacant(entry) => entry.insert(0),
                Occupied(entry) => entry.into_mut(),
            };

            *v += hunk.lines_in_hunk() as i32;
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
}

impl Fame {
    pub fn new(args: FameArgs) -> Self {
        Fame { args: args }
    }
}

impl Processable<()> for Fame {
    fn process(&self) -> Result<()> {
        let (earliest_commit, latest_commit) = grit_utils::find_commit_range(
            self.args.path.clone(),
            self.args.start_date,
            self.args.end_date,
        )?;

        info!("Early, Late: {:?}, {:?}", earliest_commit, latest_commit);

        let restrict_authors: Option<Vec<String>> =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());

        let file_names: Vec<String> = grit_utils::generate_file_list(
            &self.args.path,
            self.args.include.clone(),
            self.args.exclude.clone(),
        )?;

        let mut per_file: HashMap<String, Vec<BlameOutput>> = HashMap::new();

        for file_name in file_names.iter() {
            let bp = BlameProcessor::new(
                self.args.path.clone(),
                String::from(file_name),
                earliest_commit.clone(),
                latest_commit.clone(),
            );

            info!("processing file {}", file_name);

            let bp_result = bp.process()?;

            per_file.insert(String::from(file_name), bp_result);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::LevelFilter;
    use tempfile::TempDir;

    const LOG_LEVEL: LevelFilter = LevelFilter::Info;

    #[test]
    fn test_process_fame() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            String::from(path),
            Some("loc".to_string()),
            None,
            None,
            None,
            None,
            None,
        );

        let f = Fame::new(args);

        let result = match f.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        assert!(result, "test_process_file result was {}", result);
    }
}
