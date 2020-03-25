# grit
Grit is a git repository analyzer written in [Rust](https://github.om/rust-lang).

# Options
```
Usage:
    grit fame
    grit bydate

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.

Options:
    --branch=<string>   banch to use, defaults to current HEAD
    --debug             enables debug
    -h, --help          displays help
    --sort=<field>      sort field, either 'commit' (default), 'loc', 'files'
    --threads=<number>  number of concurrent processing threads, default is 10
    --start_date        start date for bydate in YYYY-MM-DD format.
    --end_date          end date for bydate in YYYY-MM-DD format.
    --verbose
```

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
