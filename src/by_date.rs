#[macro_use]
use git2::{Error, Repository, Time};
use chrono::{Datelike, NaiveDateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ByDate {
    date: String,
    count: i32,
}

fn by_date(repo_path: &str) -> Result<(), Error> {
    let repo = Repository::open(repo_path).expect("Could not open repository");
    let now = Utc::now();

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::NONE | git2::Sort::TIME);
    revwalk.push_head()?;

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
        debug!("commit id {}", id);
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
        let commit_time = &commit.time();
        let dt = convert_git_time(commit_time);
        info!("commit time {}-{}-{}", dt.year(), dt.month(), dt.day());
    }

    Ok(())
}

fn convert_git_time(time: &Time) -> NaiveDateTime {
    NaiveDateTime::from_timestamp(time.seconds(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::time::Instant;

    #[test]
    fn test_by_date() {
        simple_logger::init_with_level(Level::Info).unwrap();
        let start = Instant::now();

        let _result = match by_date(".") {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_by_date in {:?}", start.elapsed());
    }
}
