#[macro_export]
macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        }
    };
}

#[macro_export]
macro_rules! format_tostr {
    ($msg:expr, $s:expr) => {
        format!($msg, $s).as_str()
    };
}
