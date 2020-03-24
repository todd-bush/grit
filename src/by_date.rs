#[macro_use]
extern crate chrono;

use git2::{BlameOptions, Error, Repository, StatusOptions};
use chrono::{Utc};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ByDate {
    date: String,
    count: int32,
};

fn by_date(repo_path: &str) -> Result(ByDate, Error){

    let repo = Repository::open(repo_path)?;
    let now = Utc::now();

    Ok(())

}
