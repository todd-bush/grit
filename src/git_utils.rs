pub mod git_utils {
    use git2::Repository;
    use git2::StatusOptions;
    use glob::Pattern;
    use anyhow::{Context, Result};
    use std::path::Path;


    pub fn get_repo(path: &str) -> Result<Repository, git2::Error> {
        Repository::open(path)
    }
    
    pub fn get_repo_status_for_file(repo: &Repository, file: &str) -> Result<git2::Status, git2::Error> {
        let status = repo.status_file(Path::new(file))?;
        Ok(status)
    }

    pub fn get_file_list(repo: &Repository, include: Option<&str>, exclude: Option<&str>) -> Result<Vec<String>> {

        let include_patterns = include
            .as_deref()
            .map(|s | s.split(","))
            .map(|patterns| {
                patterns
                    .map(|s| Pattern::new(s ).with_context(|| format!("Failed to create pattern: {}", s)))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let exclude_patterns = exclude
            .as_deref()
            .map(|s | s.split(","))
            .map(|patterns| {
                patterns
                    .map(|s| Pattern::new(s ).with_context(|| format!("Failed to create pattern: {}", s)))
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
            let include_match = include_patterns.as_ref()
                .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                .unwrap_or(true);
            let exclude_match = exclude_patterns.as_ref()
                .map(|patterns| patterns.iter().any(|p| p.matches(s)))
                .unwrap_or(false);
            include_match && !exclude_match
        })  
        .collect();

        Ok(files_names)
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
            info!("files: {:?}", files);
            assert!(files.len() >= 13);
        }

        #[test]
        fn test_get_file_list_with_include() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, Some("*.rs"), None).unwrap();
            info!("files: {:?}", files);
            assert!(files.len() >= 8);
        }

        #[test]
        fn test_get_file_list_with_exclude() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo, None, Some("*.rs")).unwrap();
            info!("files: {:?}", files);
            assert!(files.len() >= 5);
        }

        #[test]
        fn test_get_file_list_with_include_and_exclude() {
            crate::grit_test::set_test_logging(LOG_LEVEL);
            let repo = get_repo(DIR).unwrap();
            let files = get_file_list(&repo,  Some("*.toml"), Some("*.rs")).unwrap();
            assert_eq!(files.len(), 1);
        }   
    }
    
}