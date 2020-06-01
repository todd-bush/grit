use chrono::{Date, Local};

struct ByFileArgs {
    path: String,
}

struct ByFile {
    name: String,
    day: Date<Local>,
    loc: usize,
}
