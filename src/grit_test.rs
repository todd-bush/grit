use git2::build::RepoBuilder;
extern crate tempfile;
use tempfile::{Builder, TempDir};
use log::LevelFilter;
use simple_logger::SimpleLogger;

pub fn init_repo() -> TempDir {
    let td = Builder::new().prefix("grit-test").tempdir().unwrap();

    println!("test repo file path {}", td.path().to_str().unwrap());

    RepoBuilder::new()
        .clone(&"https://github.com/todd-bush/grit.git", td.path())
        .unwrap();

    td
}

pub fn set_test_logging(level: LevelFilter) {
    SimpleLogger::new().with_level(level).init().unwrap_or(());
}
