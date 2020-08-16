use anyhow::Result;
use chrono::offset::Local;
use chrono::Date;

pub struct DevsArgs {
    path: String,
    pairs: bool,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
}

impl DevsArgs {
    pub fn new(
        path: String,
        pairs: bool,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
    ) -> Self {
        DevsArgs {
            path,
            pairs,
            start_date,
            end_date,
        }
    }
}

pub fn devs(args: DevsArgs) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use tempfile::TempDir;

    const LOG_LEVEL: Level = Level::Info;

    #[test]
    fn test_devs() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = DevsArgs::new(path.to_string(), false, None, None);

        let _result = devs(args);
    }
}
