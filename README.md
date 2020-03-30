# grit
Grit is a git repository analyzer written in [Rust](https://github.om/rust-lang).

# Usage
```
Usage:
    grit fame [--sort=<field>] [--debug]
    grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--debug]

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.

Options:
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --threads=<number>          number of concurrent processing threads, default is 10
    --start-date=<string>       start date for bydate in YYYY-MM-DD format.
    --end-date=<string>         end date for bydate in YYYY-MM-DD format.
    --file=<string>             output file for the by date file.  Sends to stdout by default
    --image                     creates an image for the by_date graph.  file is required
    --verbose
```

# Output

```grit bydate``` will create a csv of date and commit count to stdout or file.  Option to produce image.

```grit fame``` will create a table of metrics per author.

## Example

```
+-----------+-------+---------+------+-----------------------+
| Author    | Files | Commits | LOC  | Distribution (%)      |
+-----------+-------+---------+------+-----------------------+
| Todd Bush | 5     | 37      | 1178 | 100.0 / 97.4  / 99.9  |
| todd-bush | 1     | 1       | 1    | 20.0  / 2.6   / 0.1   |
+-----------+-------+---------+------+-----------------------+
```

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
