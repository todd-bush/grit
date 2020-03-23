#[macro_use]
use git2::{Error, Repository};
use plotlib::line::{Line, Style};
use plotlib::page::Page;
use plotlib::view::ContinuousView;

pub fn commit_graph(repo_path: &str) -> Result<(), Error> {
    collect_commits(repo_path);

    let l1 = Line::new(&[(0., 1.), (2., 1.5), (3., 1.2), (4., 1.1)]);

    let v = ContinuousView::new().add(&l1);

    //Page::single(&v).save("line.svg").expect("saving svg");

    println!("{}", Page::single(&v).to_text().unwrap());

    Ok(())
}

fn collect_commits(repo_path: &str) -> Result<(), Error> {
    let repo = Repository::open(repo_path)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::REVERSE | git2::Sort::TIME);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_collect_commits() {
        // let result = match collect_commits(".") {
        //     Ok(()) => true,
        //     Err(_e) => false,
        // };
        //
        // assert!(
        //     result,
        //     "Test result for test_collect_commits was {}",
        //     result
        // );
    }

    #[test]
    fn test_commit_graph() {
        //commit_graph(".");
    }
}
