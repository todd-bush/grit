use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::offset::{Local, TimeZone};
use chrono::Date;

pub struct EffortArgs {
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    table: bool,
}

impl EffortArgs {
    pub fn new(
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        table: bool,
    ) -> Self {
        EffortArgs {
            start_date,
            end_date,
            table,
        }
    }
}

struct EffortOutput {
    file: String,
    commits: usize,
    active_days: usize,
}

impl EffortOutput {
    pub fn new(file: String) -> Self {
        EffortOutput {
            file,
            commits: 0,
            active_days: 0,
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn effort(repo_path: &str, args: EffortArgs) -> GenResult<()> {
    Ok(())
}

fn process_effort(
    repo_path: &str,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
) -> GenResult<Vec<EffortOutput>> {
    let start_date = match start_date {
        Some(d) => d,
        None => Local
            .from_local_date(&MIN_DATE)
            .single()
            .expect("Cannot unwrap MIN_DATE"),
    };

    let end_date = match end_date {
        Some(d) => d,
        None => Local
            .from_local_date(&MAX_DATE)
            .single()
            .expect("Cannot unwrap MAX_DATE"),
    };

    let mut result: Vec<EffortOutput> = vec![];

    Ok(result)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_effort() {}
}
