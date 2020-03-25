#[macro_use]
use git2::{Error, Repository};
use chrono::Utc;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ByDate {
    date: String,
    count: i32,
}

fn by_date(repo_path: &str) -> Result<(), Error> {
    let repo = Repository::open(repo_path).expect("Could not open repository");
    let now = Utc::now();

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME);

    macro_rules! filter_try {
        ($e:expr) => {
            match $e {
                Ok(t) => t,
                Err(e) => return Some(Err(e)),
            }
        };
    }

    debug!("filtering revwalk");

    let revwalk = revwalk.filter_map(|id| {
        let id = filter_try!(id);
        let commit = filter_try!(repo.find_commit(id));
        let commit_time = commit.time().seconds();

        if commit_time > now.timestamp() {
            return None;
        }

        Some(Ok(commit))
    });

    debug!("filtering completed");

    for commit in revwalk {
        let commit = commit?;
        let commit_time = commit.time();
        info!("commit time {}", commit_time.seconds());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;

    #[test]
    fn test_by_date() {
        simple_logger::init_with_level(Level::Debug).unwrap();
        let start = Instant::now();

        let result = match by_date(".") {
            Ok(()) => true,
            Err(_e) => false,
        };
    }
}
