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

    use chrono::{Date, Datelike, Local, NaiveDateTime, TimeZone};
    use git2::{Repository, StatusOptions, Time};
    use glob::Pattern;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use anyhow::Result;

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

        let includes: Vec<Pattern> = match include {
            Some(e) => e
                .split(',')
                .map(|s| Pattern::new(s).expect(format_tostr!("cannot create new Pattern {} ", s)))
                .collect(),
            None => Vec::new(),
        };

        let excludes: Vec<Pattern> = match exclude {
            Some(e) => e
                .split(',')
                .map(|s| Pattern::new(s).expect(format_tostr!("cannot create new Pattern {} ", s)))
                .collect(),
            None => Vec::new(),
        };

        let file_names: Vec<String> = statuses
            .iter()
            .filter_map(|se| {
                let s = se
                    .path()
                    .expect("Cannot create string from path")
                    .to_string();
                let exclude_s = s.clone();
                let mut result = None;

                if includes.is_empty() {
                    debug!("including {} to the file list", s);
                    result = Some(s);
                } else {
                    for p in &includes {
                        if p.matches(&s) {
                            result = Some(
                                se.path()
                                    .expect("Cannot create string from path")
                                    .to_string(),
                            );
                            break;
                        };
                    }
                }

                if !excludes.is_empty() && result.is_some() {
                    for p in &excludes {
                        if p.matches(&exclude_s) {
                            result = None;
                            debug!("removing {} from the file list", exclude_s);
                            break;
                        }
                    }
                }

                result
            })
            .collect();

        Ok(file_names)
    }

    pub fn convert_git_time(time: &Time) -> Date<Local> {
        Local
            .from_utc_datetime(&NaiveDateTime::from_timestamp(time.seconds(), 0))
            .date()
    }

    pub fn format_date(d: Date<Local>) -> String {
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
        let html_output = format!(
            "<html><head></head><body><img src=\"{}\"/></body></html>",
            filename
        );

        let mut output = File::create(html_file).expect("HTML file creation failed");
        output
            .write_all(html_output.as_bytes())
            .expect("Writing to HTML File failed");

        Ok(())
    }

    pub fn check_file_type(filename: &str, ext: &str) -> bool {
        let file_ext = match get_filename_extension(filename) {
            Some(f) => f,
            None => "",
        };

        ext.eq_ignore_ascii_case(file_ext)
    }

    pub fn find_commit_range(
        repo_path: String,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
    ) -> GenResult<(Option<Vec<u8>>, Option<Vec<u8>>)> {
        let mut earliest_commit = None;
        let mut latest_commit = None;

        let repo = Repository::open(repo_path.clone()).expect(format_tostr!(
            "Could not open repo for path {}",
            repo_path.clone()
        ));

        if let Some(d) = start_date {
            let start_date_sec = d.naive_local().and_hms(0, 0, 0).timestamp();
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
                    earliest_commit = Some(oid.as_bytes().iter().map(|b| *b).collect())
                } else {
                    break;
                }
            }
        }

        if let Some(d) = end_date {
            let end_date_sec = d.naive_local().and_hms(23, 59, 59).timestamp();

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
                    latest_commit = Some(oid.as_bytes().iter().map(|b| *b).collect())
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
        use chrono::NaiveDate;
        use log::Level;
        use tempfile::TempDir;

        const DIR: &str = ".";

        #[test]
        fn test_generate_file_list_all() {
            let result = generate_file_list(DIR, None, None).unwrap();

            assert!(
                result.len() >= 6,
                "test_generate_file_list_all was {}",
                result.len()
            );
        }

        #[test]
        fn test_generate_file_list_rust() {
            let result = generate_file_list(DIR, Some("*.rs".to_string()), None).unwrap();

            assert!(
                result.len() >= 5,
                "test_generate_file_list_all was {}",
                result.len()
            );
        }

        #[test]
        fn test_generate_file_list_exclude_rust() {
            let result = generate_file_list(DIR, None, Some("*.rs".to_string())).unwrap();

            assert!(
                result.len() >= 3,
                "test_generate_file_list_exclude_rust was {}",
                result.len()
            );
        }

        #[test]
        fn test_format_date() {
            let test_date = Local.ymd(2020, 3, 13);

            assert_eq!(format_date(test_date), "2020-03-13");
        }

        #[test]
        fn test_get_filename_extension() {
            assert_eq!(get_filename_extension("test.txt"), Some("txt"));
            assert_eq!(get_filename_extension("test"), None);
        }

        #[test]
        fn test_strip_extension() {
            assert_eq!(strip_extension("test.txt"), Some("test"));
            assert_eq!(strip_extension("src/test.txt"), Some("test"));
        }

        #[test]
        fn test_check_filetype() {
            assert!(check_file_type("test.txt", "txt"));
            assert!(check_file_type("test.rs", "rs"));
            assert!(!check_file_type("test.rs", "txt"));
        }

        #[test]
        fn test_find_commit_range_no() {
            simple_logger::init_with_level(Level::Info).unwrap_or(());

            let td: TempDir = crate::grit_test::init_repo();
            let path = td.path().to_str().unwrap();

            let (early, late) = find_commit_range(path.to_string(), None, None).unwrap();

            assert_eq!(early, None);
            assert_eq!(late, None);
        }

        #[test]
        fn test_find_commit_range_early() {
            simple_logger::init_with_level(Level::Info).unwrap_or(());

            let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

            let ed = Local.from_local_date(&utc_dt).single().unwrap();
            let td: TempDir = crate::grit_test::init_repo();
            let path = td.path().to_str().unwrap();

            let (early, late) = find_commit_range(path.to_string(), Some(ed), None).unwrap();

            //info!("early = {:?}", early.unwrap());

            assert!(early.unwrap().len() > 0);
            assert_eq!(late, None);
        }
    }
}
