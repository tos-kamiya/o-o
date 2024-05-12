o-o
===

Enables commands that assume the standard input and output to read and write to files specified in the command line.

## What? Why?

Have you ever had trouble with interference between a command-invoking command and redirection?

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
Run a sub-process and customize how it handles standard I/O.

Usage:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

Options:
  <stdin>       File served as the standard input. Use `-` for no redirection.
  <stdout>      File served as the standard output. Use `-` for no redirection, `=` for the same file as the standard input, and `.` for /dev/null.
  <stderr>      File served as the standard error. Use `-` for no redirection, `=` for the same file as the standard output, and `.` for /dev/null.
                Prefix with `+` to append to the file (akin to the `>>` redirection in shell).
  -e VAR=VALUE                      Set environment variables.
  --pipe=STR, -p STR                String for pipe to connect subprocesses (`|` in shell) [default: `I`].
  --separator=STR, -s STR           String for separator of command lines (`;` in shell) [default: `J`].
  --tempdir-placeholder=STR, -t STR     Placeholder string for temporary directory [default: `T`].
  --force-overwrite, -F             Overwrite the file even if subprocess fails (exit status != 0). Valid only when <stdout> is `=`.
  --keep-going, -k                  Only effective when multiple command lines are chained with the separator. Even if one command line fails, subsequent command lines continue to be executed.
  --working-directory=DIR, -d DIR   Working directory.
  --version, -V                     Version information.
  --help, -h                        Shows this help message.
```

## Installation

Use the cargo command to install.

```sh
cargo install o-o
```

## Samples

### 1. Extract the 5th line for each of PDF files

When you know that a particular line of the PDF will contain the information you need, such as the title of a article.

```
ls *.pdf | rargs o-o - - - pdftotext '{0}' - I head -5 I tail -1
```

Here,

* [rargs](https://github.com/lotabout/rargs) is a tool which takes a filename and executes the specified command line, similar to xargs
* [pdftotext](https://github.com/jalan/pdftotext) is a tool to extract text from PDF file.

### 2. Extract vba source code from Excel files

For each of `*.xlsm` files, extract vba source code from it, delete the first 5 lines, and save the code to a file with the same name but with the extension changed to `.vba`.

```
ls *.xlsm | rargs -p '(.*)\.xlsm' o-o - '{1}'.vba - olevba -c '{0}' I sed -e 1,5d
```

The above executes the following command line when there was a file named `foo.xlsm`, for example.

```
olevba -c foo.xlsm | sed -e 1,5d > foo.vba
```

Here,

* [olevba](https://pypi.org/project/oletools/) is a command to extract vba code from an Excel file.

### 3. Transcribing audio from a video file

Extract the audio from the video file `amovie.webm`, save it to a temporary audio file, and then extract text from the temporary audio file.
The temporary file is created on a temporary directory and deleted when the process is finished.

```sh
o-o - - - ffmpeg -i amovie.webm T/tmp.wav J whisper T/tmp.wav --model=medium
```

The above command line is similar to the following command line, except for creating a temporary directory.

```
ffmpeg -i amovie.webm tmp.wav ; whisper tmp.wav --model=medium
```

Here,

* [ffmpeg](https://ffmpeg.org/) is a tool for processing audio files and video files.
* [whisper](https://github.com/openai/whisper) is a tool for transcribing text from audio files.

## License

MIT/Apache-2.0
