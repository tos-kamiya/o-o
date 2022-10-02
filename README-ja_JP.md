![test workflow](https://github.com/tos-kamiya/o-o/workflows/Tests/badge.svg)

o-o
===

標準入出力を想定しているコマンドをコマンドラインに指定されたファイルへと読み書きできるようにする。

## 何でこんなものを？

コマンドを起動するコマンドとリダイレクトが微妙に干渉してこまったことはありませんか？

例えば、次のコマンドラインは、

```sh
ls *.txt | xargs -I {} head -n 3 {} > {}-head.out
```

実行すると、`*.txt`ファイルのそれぞれに`*-head.out`ファイルを生成しません。
すべての`head`コマンドの実行の結果を含む一つの`{}-head.out`を生成します。

このような場合に`o-o`を使ってください。

次のように実行できます。

```sh
ls *.txt | xargs -I {} o-o - {}-head.out - head -3 {}
```

## 使い方

`o-o`の引数は、子プロセスの標準入力、標準出力、標準エラー出力で、それ以降の引数は子プロセスを起動するためのコマンドラインです。

標準入力等のファイル名として`-`を指定したものはリダイレクトしません。ファイル名の前に`+`を付けると追加モードでファイルを開きます。

```
サブプロセスを起動してその標準I/Oをリダイレクトします。

利用法:
  o-o [options] <stdin> <stdout> <stderr> [--] <commandline>...
  o-o --help
  o-o --version

オプション:
  <stdin>       標準入力として扱われるファイルです。 `-` でリダイレクトしません。
  <stdout>      標準出力として使われるファイルです。 `-` でリダイレクトしません。標準入力と同じファイルにする場合は `=` とします。`.`を指定すると/dev/nullになります。
  <stderr>      標準エラー出力として扱われるファイルです。 `-` でリダイレクトしません。標準出力と同じファイルにする場合は `=` とします。`.`を指定すると/dev/nullになります。
                ファイル名の前に `+` を付けると追加モードになります（シェルでは`>>`）。
  -e VAR=VALUE                      環境変数。
  --pipe=STR, -p STR                サブプロセスをつなげるパイプを表す文字列（シェルでは`|`）。デフォルトは`I`です。
  --separator=STR, -s STR           コマンドラインの区切りを表す文字列（シェルでは`;`）。デフォルトは`J`です。
  --tempdir-placeholder=STR, -t STR     一時ディレクトリに展開される文字列。デフォルトは`T`です。
  --force-overwrite, -F             終了ステータスが != 0 のときもファイルを上書きします。<stdout> が `=` のときのみ有効です。
  --working-directory=DIR, -d DIR   作業ディレクトリ。
```

## インストール

Cargoコマンドによりインストールしてください。

```sh
cargo install o-o
```

## サンプル

### エクセルファイルからVBAのコードを抽出する

それぞれの`*.xlsm`ファイルからVBAのコードを収集津市、最初の5行を削除し、拡張子を`.vba`に変更したファイルに保存する。

```
ls *.xlsm | rargs -p '(.*)\.xlsm' o-o - '{1}'.vba - olevba -c '{0}' I sed -e 1,5d
```

上のコマンドラインは、ファイル名が `foo.xlsm`のエクセルファイルの場合は、次のコマンドラインと同様です。

```
olevba -c foo.xlsm | sed -e 1,5d > foo.vba
```

このコマンドラインで、

* [rargs](https://github.com/lotabout/rargs) は指定されたファイル名を与えてコマンドラインを実行するツールです（xargsに類似したツールです）。
* [olevba](https://pypi.org/project/oletools/) はExcelのファイルからVBAのコードを抽出するツールです。

### 動画ファイルの音声を文字起こしする

動画ファイル`amovie.webm`から音声を抽出して一時ファイルに保存し、次に、その音声ファイルから文字起こしを行います。
一時ファイルは一時ディレクトリ上に作成され、処理が終わった時点で削除されます。

```sh
o-o - - - ffmpeg -i amovie.webm T/tmp.wav J whisper T/tmp.wav --model=medium
```

上のコマンドラインは、一時ディレクトリを作成する点以外は、次のコマンドラインと同様です。

```
ffmpeg -i amovie.webm tmp.wav ; whisper tmp.wav --model=medium
```

このコマンドラインで、

* [ffmpeg](https://ffmpeg.org/) は音声ファイルや動画ファイルを加工するツールです。
* [whisper](https://github.com/openai/whisper) 音声ファイルからの文字起こしツールです。

## ライセンス

MIT/Apache-2.0

## Todos

- [x] Rustで再実装
- [x] `--force-overwrite`のテスト
- [x] /dev/nullを扱えるようにする
- [x] 一時ディレクトリ機能 (v0.4.0)
- [x] コマンドラインセパレータ (v0.4.0)
