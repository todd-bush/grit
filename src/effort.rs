use crate::utils::grit_utils;
use chrono::offset::Local;
use chrono::Date;
use git2::{BlameOptions, Oid, Repository};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

pub struct EffortArgs {
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    table: bool,
}

impl EffortArgs {
    pub fn new(
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        table: bool,
    ) -> Self {
        EffortArgs {
            start_date,
            end_date,
            table,
        }
    }
}

#[derive(Debug)]
struct EffortOutput {
    file: String,
    commits: usize,
    active_days: usize,
}

impl EffortOutput {
    pub fn new(file: String) -> Self {
        EffortOutput {
            file,
            commits: 0,
            active_days: 0,
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn effort(repo_path: &str, args: EffortArgs) -> GenResult<()> {
    let _results = process_effort(repo_path, args.start_date, args.end_date);

    Ok(())
}

fn process_effort(
    repo_path: &str,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
) -> GenResult<Vec<EffortOutput>> {
    let (earliest_commit, latest_commit) =
        grit_utils::find_commit_range(repo_path.to_string(), start_date, end_date)?;

    let file_names: Vec<String> = grit_utils::generate_file_list(repo_path, None, None)?;

    let mut result: Vec<EffortOutput> = vec![];

    for file_name in file_names {
        let er = process_effort_file( repo_path, &file_name, earliest_commit.clone(),latest_commit.clone())?;
        result.push(er);
    }

    Ok(result)
}

fn process_effort_file(
    repo_path: &str,
    file_name: &str,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
) -> GenResult<EffortOutput> {
    let mut result = EffortOutput::new(file_name.to_string());

    let path = Path::new(file_name);

    let repo = Repository::open(repo_path)?;
    let mut blame_ops = BlameOptions::new();
    let mut effort_commits: HashSet<String> = HashSet::new();
    let mut effort_dates: HashSet<Date<Local>> = HashSet::new();

    if let Some(ev) = earliest_commit {
        let oid: Oid = Oid::from_bytes(&ev)?;
        let commit = repo.find_commit(oid)?;
        blame_ops.oldest_commit(commit.id());
    };

    if let Some(ov) = latest_commit {
        let oid: Oid = Oid::from_bytes(&ov)?;
        let commit = repo.find_commit(oid)?;
        blame_ops.newest_commit(commit.id());
    };

    let start = Instant::now();

    let blame = repo.blame_file(path, Some(&mut blame_ops))?;

    info!("Blame executed in {:?}", start.elapsed());

    for hunk in blame.iter() {
        let commit_id = hunk.final_commit_id();
        let commit = repo.find_commit(commit_id)?;
        let commit_date = grit_utils::convert_git_time(&commit.time());

        effort_commits.insert(commit_id.to_string());
        effort_dates.insert(commit_date);
    }

    result.commits = effort_commits.len();
    result.active_days = effort_dates.len();

    Ok(result)
}

#[cfg(test)]
mod test {
    use super::*;
    use log::Level;
    use tempfile::TempDir;

    #[test]
    fn test_process_effort_file() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let result = process_effort_file(path, "README.md", None,None).unwrap();

        info!("results: {:?}", result);
        assert!(result.commits>20);
        assert!(result.active_days>15);

    }

    #[test]
    fn test_process_effort() {

        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let result = process_effort(path, None, None);

        info!("results: {:?}", result);

    }
}
