# grit
Grit is a git repository analyzer written in [Rust](https://github.com/rust-lang).

[Fame](https://github.com/oleander/git-fame-rb) is based on the "git fame" functionality.


![Rust](https://github.com/todd-bush/grit/workflows/Rust/badge.svg?branch=master)

# Usage
```
Usage:
    grit fame [--sort=<field>] [--start-date=<string>] [--end-date=<string>] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
    grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--ignore-weekends] [--ignore-gap-fill] [--verbose] [--debug]
    grit byfile [--in-file=<string>] [--file=<string>] [--verbose] [--debug]

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.
    byfile: produces by author commit counts for a specific file

Options:
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --start-date=<string>       start date in YYYY-MM-DD format.
    --end-date=<string>         end date in YYYY-MM-DD format.
    --include=<string>          comma delimited, glob file path to include path1/*,path2/*
    --exclude=<string>          comma delimited, glob file path to exclude path1/*,path2/*
    --file=<string>             output file for the by date file.  Sends to stdout by default
    --in-file=<string>          input file for by_file
    --image                     creates an image for the by_date graph.  file is required
    --ignore-weekends           ignore weekends when calculating # of commits
    --ignore-gap-fill           ignore filling empty dates with 0 commits
    -v, --verbose
```

# Output

```grit bydate``` will create a csv of date and commit count to stdout or file.  Option to produce image.

```grit byfile``` will create a csv of author, date, and commit counts to stdout or file.

```grit fame``` will create a table of metrics per author.  This may take a while for repos with long commit history, consider using date ranges to reduce computation time.

## Fame Example

```
Stats on Repo
Total files: 6
Total commits: 35
Total LOC: 958
+-----------+-------+---------+-----+-----------------------+
| Author    | Files | Commits | LOC | Distribution (%)      |
+-----------+-------+---------+-----+-----------------------+
| Todd Bush | 6     | 34      | 948 | 100.0 / 97.1  / 99.0  |
| todd-bush | 1     | 1       | 10  | 16.7  / 2.9   / 1.0   |
+-----------+-------+---------+-----+-----------------------+
```

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
