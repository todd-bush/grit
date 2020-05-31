use git2::{BlameOptions, Repository, StatusOptions};

pub struct ByPeopleArgs {
    path: String,
}

struct ByPeople {
    name: String,
    lines_added: usize,
    lines_deleted: usize,
}

impl ByPeople {
    pub fn new(name: String) -> Self {
        ByPeople {
            name: name,
            lines_added: 0,
            lines_deleted: 0,
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn process_people(args: ByPeopleArgs) -> GenResult<()> {
    Ok(())
}

fn find_people(args: ByPeopleArgs) -> GenResult<Vec<ByPeople>> {
    let result = Vec::new();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;

    #[test]
    fn test_find_people() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());
    }
}
