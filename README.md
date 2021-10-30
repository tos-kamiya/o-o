o-o
===

Extends the command line for commands that assume standard input/output to allow input/output to files.

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
  -e VAR=VALUE      Environment variables.
  --force-overwrite, -F             Overwrite the file even when exit status != 0. Valid only when <stdout> is `=`.
  --working-directory=DIR, -d DIR   Working directory.
```

## Installation

To build, use `cargo build --release`, which will create the executable `target/release/o-o`.

To install, copy a file `o-o` in any directory on PATH, e.g. `~/bin`.

To uninstall, remove the file `o-o`.

## License

Unlicense (Public Domain).

## Todos

- [x] Reimplemented in Rust
- [ ] Testing `--force-overwrite`.
