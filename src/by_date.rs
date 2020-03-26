#[macro_use]
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::{Datelike, NaiveDateTime};
use csv::Writer;
use git2::{Error, Repository, Time};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::io;

#[derive(Ord, Debug, PartialEq, Eq, PartialOrd)]
struct ByDate {
    date: String,
    count: i32,
}

impl ByDate {
    pub fn new(date: String, count: i32) -> Self {
        ByDate { date, count }
    }
}

pub fn by_date(
    repo_path: &str,
    start_date: Option<NaiveDateTime>,
    end_date: Option<NaiveDateTime>,
) -> Result<(), Error> {
    let end_date = match end_date {
        Some(d) => d,
        None => MAX_DATE.and_hms(0, 0, 0),
    };

    let start_date = match start_date {
        Some(d) => d,
        None => MIN_DATE.and_hms(0, 0, 0),
    };

    let mut output_map: HashMap<String, i32> = HashMap::new();

    let repo = Repository::open(repo_path).expect("Could not open repository");

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

        if commit_time < start_date.timestamp() {
            return None;
        }

        if commit_time > end_date.timestamp() {
            return None;
        }

        Some(Ok(commit))
    });

    debug!("filtering completed");

    for commit in revwalk {
        let commit = commit?;
        let commit_time = &commit.time();
        let dt = convert_git_time(commit_time);
        let date_string = format!("{}-{:0>2}-{:0>2}", dt.year(), dt.month(), dt.day());

        info!("commit time {}", date_string);

        let v = match output_map.entry(date_string) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };
        *v += 1;
    }

    let mut output: Vec<ByDate> = output_map
        .iter()
        .map(|(key, val)| ByDate::new(key.to_string(), *val))
        .collect();

    output.sort();

    match display_output(output) {
        Ok(_v) => {}
        Err(e) => error!("Error thrown in display_output {:?}", e),
    };

    Ok(())
}

fn convert_git_time(time: &Time) -> NaiveDateTime {
    NaiveDateTime::from_timestamp(time.seconds(), 0)
}

fn display_output(output: Vec<ByDate>) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_writer(io::stdout());

    wtr.write_record(&["date", "count"])?;

    output.iter().for_each(|r| {
        wtr.serialize((r.date.to_string(), r.count)).unwrap();
    });

    wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::time::Instant;

    const LOG_LEVEL: Level = Level::Warn;

    #[test]
    fn test_by_date_no_ends() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());
        let start = Instant::now();

        let _result = match by_date(".", None, None) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());
    }

    #[test]
    fn test_by_date_end_date_only() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let ed = NaiveDateTime::parse_from_str("2020-03-26 23:59:59", "%Y-%m-%d %H:%M:%S");

        let start = Instant::now();

        let _result = match by_date(".", None, Some(ed.unwrap())) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!(
            "completed test_by_date_end_date_only in {:?}",
            start.elapsed()
        );
    }
}
