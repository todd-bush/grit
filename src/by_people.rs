use std::collections::HashMap;

pub struct ByPeopleArgs {}

struct ByPeople {
    name: String,
    date_count: HashMap<String, usize>,
}

impl ByPeople {
    pub fn new(name: String) -> Self {
        ByPeople {
            name: name,
            date_count: HashMap::new(),
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn process_people(args: ByPeopleArgs) -> GenResult<()> {
    Ok(())
}
