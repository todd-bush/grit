pub mod git_utils {
    use anyhow::{Context, Result};
    use chrono::{DateTime, Local};
    use git2::Repository;
    use git2::StatusOptions;
    use glob::Pattern;
    use std::path::Path;

    pub fn get_repo(path: &str) -> Result<Repository, git2::Error> {
        Repository::open(path)
    }

    pub fn get_repo_status_for_file(
        repo: &Repository,
        file: &str,
    ) -> Result<git2::Status, git2::Error> {
        let status = repo.status_file(Path::new(file))?;
        Ok(status)
    }

    pub fn get_file_list(
        repo: &Repository,
        include: Option<&str>,
        exclude: Option<&str>,
    ) -> Result<Vec<String>> {
        let include_patterns = include
            .map(|s| s.split(","))
            .map(|patterns| {
                patterns
                    .map(|s| {
                        Pattern::new(s).with_context(|| format!("Failed to create pattern: {s}"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let exclude_patterns = exclude
            .map(|s| s.split(","))
            .map(|patterns| {
                patterns
                    .map(|s| {
                        Pattern::new(s).with_context(|| format!("Failed to create pattern: {s}"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(false);
        status_opts.include_unmodified(true);
        status_opts.include_ignored(false);
        status_opts.include_unreadable(false);
        status_opts.exclude_submodules(true);

        let statuses = repo.statuses(Some(&mut status_opts))?;

        let files_names = statuses
            .iter()
            .filter_map(|se| se.path().map(|p| p.to_string()))
            .filter(|s| {
                let include_match = include_patterns
                    .as_ref()
                    .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                    .unwrap_or(true);
                let exclude_match = exclude_patterns
                    .as_ref()
                    .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                    .unwrap_or(false);
                include_match && !exclude_match
            })
            .collect();

        Ok(files_names)
    }

    pub fn date_to_git_date(date: DateTime<Local>) -> git2::Time {
        let naive = date.naive_utc();
        git2::Time::new(naive.and_utc().timestamp(), 0)
    }

    #[derive(Debug)]
    pub struct CommitRange {
        earliest: Option<Vec<u8>>,
        latest: Option<Vec<u8>>,
    }

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
                    if commit_time >= start_date_sec
                        && commit_range.earliest.is_none() {
                            commit_range.earliest =
                                Some(oid.as_bytes().to_vec());
                        }
                }

                // Check if this commit is before the end_date
                if let Some(d) = end_date {
                    let end_date_sec = d.timestamp();
                    if commit_time <= end_date_sec
                        && commit_range.latest.is_none() {
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
        fn test_get_file_list() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, None, None).unwrap();
            info!("files: {files:?}");
            assert!(files.len() >= 13);
        }

        #[test]
        fn test_get_file_list_with_include() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, Some("*.rs"), None).unwrap();
            info!("files: {files:?}");
            assert!(files.len() >= 8);
        }

        #[test]
        fn test_get_file_list_with_exclude() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, None, Some("*.rs")).unwrap();
            info!("files: {files:?}");
            assert!(files.len() >= 5);
        }

        #[test]
        fn test_get_file_list_with_include_and_exclude() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, Some("*.toml"), Some("*.rs")).unwrap();
            assert_eq!(files.len(), 1);
        }

        #[test]
        fn test_date_to_git_date() {
            let date = Local::now();
            let git_date = date_to_git_date(date);
            assert_eq!(git_date.seconds(), date.timestamp());
        }

        #[test]
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
            let commit_range = find_commit_range(&repo, Some(Local::now()), None).unwrap();

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
            let commit_range =
                find_commit_range(&repo, Some(start_date), Some(Local::now())).unwrap();

            info!("commit_range: {commit_range:?}");

            assert!(commit_range.earliest.is_some());
            assert!(commit_range.latest.is_some());
        }
    }
}
