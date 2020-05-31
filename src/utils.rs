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

    use git2::{Repository, StatusOptions};
    use glob::Pattern;

    type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
                    info!("including {} to the file list", s);
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
                            info!("removing {} from the file list", exclude_s);
                            break;
                        }
                    }
                }

                result
            })
            .collect();

        Ok(file_names)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_generate_file_list_all() {
        let result = grit_utils::generate_file_list(".", None, None).unwrap();

        assert!(
            result.len() >= 6,
            "test_generate_file_list_all was {}",
            result.len()
        );
    }

    #[test]
    fn test_generate_file_list_rust() {
        let result = grit_utils::generate_file_list(".", Some("*.rs".to_string()), None).unwrap();

        assert!(
            result.len() >= 6,
            "test_generate_file_list_all was {}",
            result.len()
        );
    }

    #[test]
    fn test_generate_file_list_exclude_rust() {
        let result = grit_utils::generate_file_list(".", None, Some("*.rs".to_string())).unwrap();

        assert!(
            result.len() >= 3,
            "test_generate_file_list_exclude_rust was {}",
            result.len()
        );
    }
}
