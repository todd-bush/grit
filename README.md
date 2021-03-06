# grit
Grit is a git repository analyzer written in [Rust](https://github.com/rust-lang).

[Fame](https://github.com/oleander/git-fame-rb) is based on the "git fame" functionality.


![Rust](https://github.com/todd-bush/grit/workflows/Rust/badge.svg?branch=master)

# Usage
```
Usage:
    grit fame [--sort=<field>] [--start-date=<string>] [--end-date=<string>] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
    grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--html] [--ignore-weekends] [--ignore-gap-fill] [--verbose] [--debug]
    grit byfile [--in-file=<string>] [--file=<string>] [--image] [--html] [--verbose] [--debug]
    grit effort [--start-date=<string>] [--end-date=<string>] [--table] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]

Options:
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --start-date=<string>       start date in YYYY-MM-DD format.
    --end-date=<string>         end date in YYYY-MM-DD format.
    --include=<string>          comma delimited, glob file path to include path1/*,path2/*
    --exclude=<string>          comma delimited, glob file path to exclude path1/*,path2/*
    --file=<string>             output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg
    --in-file=<string>          input file for by_file
    --image                     creates an image for the by_date & by_file graph.  file is required
    --html                      creates a HTML file to help visualize the SVG output
    --table                     display as a table to stdout
    --ignore-weekends           ignore weekends when calculating # of commits
    --ignore-gap-fill           ignore filling empty dates with 0 commits
    -v, --verbose
```

# Output

```grit bydate``` will create a csv of date and commit count to stdout or file.  Option to produce a SVG image.

```grit byfile``` will create a csv of author, date, and commit counts to stdout or file.  Option to produce a SVG image.

```grit fame``` will create a table of metrics per author.  This may take a while for repos with long commit history, consider using date ranges to reduce computation time.

```git effort``` will output the # of commits and # of active dates for each file.  Default is CSV, option for a table.  This may take a while for repos with long commit history, consider using date ranges to reduce computation time.

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
