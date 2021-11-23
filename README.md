![test workflow](https://github.com/tos-kamiya/o-o/workflows/Tests/badge.svg)

o-o
===

Enables commands that assume the standard input and output to read and write to files specified in the command line.

## What? Why?

Have you ever had trouble with interference between command-invoking command and redirect?

For example, a command line:

```sh
ls *.txt | xargs -I {} head -n 3 {} > {}-head.out
```

does NOT create `*-head.out` file for each of the `*.txt` files but creates one file `{}-head.out` containing outputs of all `head` command executions.

The command `o-o` is here to help!

You can now run as follows:

```sh
ls *.txt | xargs -I {} o-o - {}-head.out - head -3 {}
```

## Usage

The `o-o` arguments are the standard input, standard output, and standard error output of the child process, and the subsequent arguments are the command line to start the child process.

If you specify `-` as the file name for standard input, etc., it will not be redirected. Putting `+` in front of a file name will open the file in append mode.

```
Redirect subprocess's standard i/o's.

Usage:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. `-` for no redirection.
  <stdout>      File served as the standard output. `-` for no redirection. `=` for the same file as the standard input.
  <stderr>      File served as the standard error. `-` for no redirection. `=` for the same file as the standard output.
                Adding `+` before a file name means appending to the file (like `>>` in redirects).
  -e VAR=VALUE                      Environment variables.
  --pipe=STR, -p STR                Use the string for connecting sub-processes by pipe (that is, `|`).
  --force-overwrite, -F             Overwrite the file even when exit status != 0. Valid only when <stdout> is `=`.
  --working-directory=DIR, -d DIR   Working directory.
```

## Installation

Use the cargo command to install.

```sh
cargo install o-o
```

## License

MIT/Apache-2.0

## Todos

- [x] Reimplemented in Rust
- [ ] Testing `--force-overwrite`.
