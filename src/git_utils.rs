use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use git2::Repository;
use git2::StatusOptions;
use glob::Pattern;
use std::path::Path;

#[allow(dead_code)]
pub fn get_repo(path: &str) -> Result<Repository, git2::Error> {
    Repository::open(path)
}

#[allow(dead_code)]
pub fn get_repo_status_for_file(
    repo: &Repository,
    file: &str,
) -> Result<git2::Status, git2::Error> {
    let status = repo.status_file(Path::new(file))?;
    Ok(status)
}

#[allow(dead_code)]
pub fn date_to_git_date(date: DateTime<Local>) -> git2::Time {
    let naive = date.naive_utc();
    git2::Time::new(naive.and_utc().timestamp(), 0)
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CommitRange {
    earliest: Option<Vec<u8>>,
    latest: Option<Vec<u8>>,
}

#[allow(dead_code)]
pub fn find_commit_range(
    repo: &Repository,
    start_date: Option<DateTime<Local>>,
    end_date: Option<DateTime<Local>>,
) -> Result<CommitRange> {
    let mut commit_range = CommitRange {
        earliest: None,
        latest: None,
    };

    // If no start_date is provided, find the earliest commit
    if start_date.is_none() {
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::REVERSE | git2::Sort::TIME)?;
        revwalk.push_head()?;

        if let Some(Ok(oid)) = revwalk.next() {
            commit_range.earliest = Some(oid.as_bytes().to_vec());
        }
    }

    // If no end_date is provided, find the latest commit
    if end_date.is_none() {
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::NONE | git2::Sort::TIME)?;
        revwalk.push_head()?;

        if let Some(Ok(oid)) = revwalk.next() {
            commit_range.latest = Some(oid.as_bytes().to_vec());
        }
    }

    // If we have start_date or end_date, search for commits within the range
    if start_date.is_some() || end_date.is_some() {
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::NONE | git2::Sort::TIME)?;
        revwalk.push_head()?;

        for id in revwalk {
            let oid = id?;
            let commit = repo.find_commit(oid)?;
            let commit_time = commit.time().seconds();

            // Check if this commit is after the start_date
            if let Some(d) = start_date {
                let start_date_sec = d.timestamp();
                if commit_time >= start_date_sec && commit_range.earliest.is_none() {
                    commit_range.earliest = Some(oid.as_bytes().to_vec());
                }
            }

            // Check if this commit is before the end_date
            if let Some(d) = end_date {
                let end_date_sec = d.timestamp();
                if commit_time <= end_date_sec && commit_range.latest.is_none() {
                    commit_range.latest = Some(oid.as_bytes().to_vec());
                }
            }
        }
    }

    Ok(commit_range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::LevelFilter;

    const LOG_LEVEL: LevelFilter = LevelFilter::Debug;
    const DIR: &str = ".";

    #[test]
    #[ignore]
    fn test_date_to_git_date() {
        let date = Local::now();
        let git_date = date_to_git_date(date);
        assert_eq!(git_date.seconds(), date.timestamp());
    }

    #[test]
    #[ignore]
    fn test_find_commit_range() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let repo = get_repo(DIR).unwrap();
        let commit_range = find_commit_range(&repo, None, None).unwrap();
        assert!(commit_range.earliest.is_some());
        assert!(commit_range.latest.is_some());
        info!("commit_range: {commit_range:?}");

        assert_ne!(commit_range.earliest, commit_range.latest);
    }

    #[test]
    fn test_find_commit_range_with_start_date() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let repo = get_repo(DIR).unwrap();
        let start_date = Local::now() - chrono::Duration::days(365);
        let commit_range = find_commit_range(&repo, Some(start_date), None).unwrap();

        info!("commit_range: {commit_range:?}");

        assert!(commit_range.earliest.is_some());
    }

    #[test]
    fn test_find_commit_range_with_end_date() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let repo = get_repo(DIR).unwrap();
        let commit_range = find_commit_range(&repo, None, Some(Local::now())).unwrap();

        info!("commit_range: {commit_range:?}");

        assert!(commit_range.latest.is_some());
    }

    #[test]
    fn test_find_commit_range_with_start_and_end_date() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let start_date_str = "2020-10-10 21:02:20.346474121 UTC";
        let start_date = start_date_str.parse::<DateTime<Local>>().unwrap();
        let repo = get_repo(DIR).unwrap();
        let commit_range = find_commit_range(&repo, Some(start_date), Some(Local::now())).unwrap();

        info!("commit_range: {commit_range:?}");

        assert!(commit_range.earliest.is_some());
        assert!(commit_range.latest.is_some());
    }
}
