#[macro_export]
macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        }
    };
}

#[macro_export]
macro_rules! format_tostr {
    ($msg:expr, $s:expr) => {
        format!($msg, $s).as_str()
    };
}

pub mod grit_utils {

    use anyhow::{Context, Result};
    use chrono::{DateTime, Datelike, Local, NaiveDateTime, Utc};
    use git2::{Repository, StatusOptions, Time};
    use glob::Pattern;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    type GenResult<T> = Result<T>;

    pub fn generate_file_list(
        path: &str,
        include: Option<String>,
        exclude: Option<String>,
    ) -> GenResult<Vec<String>> {
        let repo = Repository::open(path)?;

        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(false);
        status_opts.include_unmodified(true);
        status_opts.include_ignored(false);
        status_opts.include_unreadable(false);
        status_opts.exclude_submodules(true);

        let statuses = repo.statuses(Some(&mut status_opts))?;

        let includes: Option<Vec<Pattern>> = include
            .as_deref()
            .map(|s| s.split(','))
            .map(|patterns| {
                patterns
                    .map(|s| {
                        Pattern::new(s).with_context(|| format!("Failed to create pattern: {s}"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let excludes: Option<Vec<Pattern>> = exclude
            .as_deref()
            .map(|s| s.split(','))
            .map(|patterns| {
                patterns
                    .map(|s| {
                        Pattern::new(s).with_context(|| format!("Failed to create pattern: {s}"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let file_names: Vec<String> = statuses
            .iter()
            .filter_map(|se| se.path().map(|p| p.to_string()))
            .filter(|s| {
                let include_match = includes
                    .as_ref()
                    .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                    .unwrap_or(true);

                let exclude_match = excludes
                    .as_ref()
                    .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                    .unwrap_or(false);

                include_match && !exclude_match
            })
            .collect();

        Ok(file_names)
    }

    pub fn convert_string_list_to_vec(input: Option<String>) -> Option<Vec<String>> {
        let result: Option<Vec<String>> =
            input.map(|s| s.split(",").map(|e| e.to_string()).collect());

        result
    }

    pub fn convert_git_time(time: &Time) -> DateTime<Local> {
        DateTime::<Utc>::from_timestamp(time.seconds(), 0)
            .unwrap()
            .with_timezone(&Local)
    }

    pub fn format_date(d: DateTime<Local>) -> String {
        format!("{}-{:0>2}-{:0>2}", d.year(), d.month(), d.day())
    }

    pub fn get_filename_extension(filename: &str) -> Option<&str> {
        Path::new(filename).extension().and_then(OsStr::to_str)
    }

    pub fn strip_extension(filename: &str) -> Option<&str> {
        Path::new(filename).file_stem().and_then(OsStr::to_str)
    }

    pub fn create_html(filename: &str) -> GenResult<()> {
        let file_base = match strip_extension(filename) {
            Some(f) => f,
            None => panic!("cannot create html file"),
        };

        let html_file = format!("{}{}", file_base, ".html");
        let html_output =
            format!("<html><head></head><body><img src=\"{filename}\"/></body></html>");

        let mut output = File::create(html_file).expect("HTML file creation failed");
        output
            .write_all(html_output.as_bytes())
            .expect("Writing to HTML File failed");

        Ok(())
    }

    pub fn check_file_type(filename: &str, ext: &str) -> bool {
        let file_ext = get_filename_extension(filename).unwrap_or_default();

        ext.eq_ignore_ascii_case(file_ext)
    }

    type CommitRange = (Option<Vec<u8>>, Option<Vec<u8>>);

    pub fn find_commit_range(
        repo_path: &str,
        start_date: Option<NaiveDateTime>,
        end_date: Option<NaiveDateTime>,
    ) -> GenResult<CommitRange> {
        let mut earliest_commit = None;
        let mut latest_commit = None;

        let repo = Repository::open(repo_path)
            .unwrap_or_else(|_| panic!("Could not open repo for path {repo_path}"));

        if let Some(d) = start_date {
            let start_date_sec = d.and_utc().timestamp();

            info!("finding commits after start_date_sec = {start_date_sec}");

            let mut revwalk = repo.revwalk()?;
            revwalk
                .set_sorting(git2::Sort::NONE | git2::Sort::TIME)
                .expect("Could not sort revwalk");
            revwalk.push_head()?;

            for id in revwalk {
                let oid = id?;
                let commit = repo.find_commit(oid)?;
                let commit_time = commit.time().seconds();

                if commit_time >= start_date_sec {
                    earliest_commit = Some(oid.as_bytes().to_vec())
                } else {
                    break;
                }
            }
        }

        if let Some(d) = end_date {
            let end_date_sec = d.and_utc().timestamp();

            info!("finding commits before end_date_sec = {end_date_sec:?}");

            let mut revwalk = repo.revwalk()?;
            revwalk
                .set_sorting(git2::Sort::REVERSE | git2::Sort::TIME)
                .expect("Could not sort revwalk");
            revwalk.push_head()?;

            for id in revwalk {
                let oid = id?;
                let commit = repo.find_commit(oid)?;
                let commit_time = commit.time().seconds();

                if commit_time <= end_date_sec {
                    latest_commit = Some(oid.as_bytes().to_vec())
                } else {
                    break;
                }
            }
        }

        Ok((earliest_commit, latest_commit))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use chrono::{Local, Months, NaiveDate, NaiveDateTime, TimeZone};
        use log::LevelFilter;
        use tempfile::TempDir;

        const LOG_LEVEL: LevelFilter = LevelFilter::Info;
        const DIR: &str = ".";

        #[test]
        fn test_generate_file_list_all() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            let result = generate_file_list(DIR, None, None).unwrap();

            info!("include all {result:?}");

            assert!(
                result.len() >= 6,
                "test_generate_file_list_all was {}",
                result.len()
            );
        }

        #[test]
        fn test_generate_file_list_rust() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            let result = generate_file_list(DIR, Some("*.rs".to_string()), None).unwrap();

            info!("include *.rs {result:?}");

            assert!(
                result.iter().all(|s| s.ends_with(".rs")),
                "test_generate_file_list_all was {}",
                result.len()
            );
        }

        #[test]
        fn test_generate_file_list_exclude_rust() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            let result = generate_file_list(DIR, None, Some("*.rs".to_string())).unwrap();

            info!("excludes *.rs {result:?}");

            assert!(
                !result.iter().any(|s| s.ends_with(".rs")),
                "test_generate_file_list_exclude_rust was {}",
                result.len()
            );
        }

        #[test]
        fn test_format_date() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            let test_date = Local.with_ymd_and_hms(2020, 3, 13, 0, 0, 0).unwrap();

            assert_eq!(format_date(test_date), "2020-03-13");
        }

        #[test]
        fn test_get_filename_extension() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            assert_eq!(get_filename_extension("test.txt"), Some("txt"));
            assert_eq!(get_filename_extension("test"), None);
        }

        #[test]
        fn test_strip_extension() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            assert_eq!(strip_extension("test.txt"), Some("test"));
            assert_eq!(strip_extension("src/test.txt"), Some("test"));
        }

        #[test]
        fn test_check_filetype() {
            crate::grit_test::set_test_logging(LevelFilter::Info);
            assert!(check_file_type("test.txt", "txt"));
            assert!(check_file_type("test.rs", "rs"));
            assert!(!check_file_type("test.rs", "txt"));
        }

        #[test]
        fn test_find_commit_range_no() {
            crate::grit_test::set_test_logging(LevelFilter::Info);

            let td: TempDir = crate::grit_test::init_repo();
            let path = td.path().to_str().unwrap();

            let (early, late) = find_commit_range(path, None, None).unwrap();

            assert_eq!(early, None);
            assert_eq!(late, None);
        }

        #[test]
        #[ignore]
        fn test_find_commit_range_here() {
            crate::grit_test::set_test_logging(LevelFilter::Info);

            let et: NaiveDateTime = NaiveDateTime::parse_from_str(
                "2025-04-20 21:02:20.346474121 +0400",
                "%Y-%m-%d %H:%M:%S%.f %z",
            )
            .unwrap();

            let es: NaiveDateTime = Local::now()
                .naive_local()
                .checked_add_months(Months::new(360))
                .unwrap();

            let (early, late) = find_commit_range(".", Option::Some(es), Option::Some(et)).unwrap();

            info!("late = {:?}", late.as_ref());

            assert_eq!(early, None);
            assert_eq!(
                late.as_ref(),
                Some(&vec![
                    230u8, 147, 145, 157, 203, 250, 82, 45, 36, 189, 89, 60, 77, 127, 121, 65, 195,
                    122, 181, 12
                ])
            );
        }

        #[test]
        fn test_convert_string_list_to_vec() {
            let test_vec: Vec<String> =
                vec![String::from("1"), String::from("2"), String::from("3")];

            assert_eq!(convert_string_list_to_vec(None), None);
            assert_eq!(
                convert_string_list_to_vec(Some(String::from("1,2,3"))),
                Some(test_vec)
            );
        }

        #[test]
        fn test_find_commit_range_early() {
            crate::grit_test::set_test_logging(LOG_LEVEL);

            let ed = NaiveDate::from_ymd_opt(2020, 3, 26).unwrap();
            let ed = ed.and_hms_opt(0, 0, 0).unwrap();

            let td: TempDir = crate::grit_test::init_repo();
            let path = td.path().to_str().unwrap();

            let (early, late) = find_commit_range(path, Some(ed), None).unwrap();

            info!("early = {:?}", early.as_ref());

            assert!(!early.unwrap().is_empty());
            assert_eq!(late, None);
        }
    }
}
