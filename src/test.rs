use git2::build::RepoBuilder;
use tempfile::{Builder, TempDir};

pub fn init_repo() -> TempDir {
    let td = Builder::new().prefix("grit-test").tempdir().unwrap();

    println!("test repo file path {}", td.path().to_str().unwrap());

    RepoBuilder::new()
        .clone(&"https://github.com/todd-bush/grit.git", td.path())
        .unwrap();

    td
}
