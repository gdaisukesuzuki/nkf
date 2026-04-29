# NKF.rust

Rust port scaffold based on `nkf.c`, `nkf.h`, `utf8tbl.c`, `utf8tbl.h`,
`config.h`, and the original `Makefile`.

This version targets UNIX/Linux only. It uses `encoding_rs` for common Japanese
encodings and falls back to the system `iconv(3)` implementation through a small
FFI layer for encodings that are not covered by `encoding_rs`.

## Build

```sh
cargo build --release
```

## Usage

```sh
target/release/nkf -w input.txt
target/release/nkf -s input.txt > output.sjis
target/release/nkf --ic=EUC-JP --oc=UTF-8 input.txt
```

Supported first-pass options:

- Output encoding: `-j`, `-e`, `-s`, `-w`, `-w8`, `-w16`, `-w16B`, `-w16L`,
  `-w32`, `-w32B`, `-w32L`, `--oc=...`
- Input encoding: `-J`, `-E`, `-S`, `-W`, `-W8`, `-W16`, `-W16B`, `-W16L`,
  `-W32`, `-W32B`, `-W32L`, `--ic=...`
- Guessing: `-g`
- Transparent copy: `-t`
- Base64: `-mB` decodes Base64 input, `-MB` encodes converted output as Base64
- MIME encoded-word: `-m` decodes `=?charset?B/Q?...?=`, `-M` encodes as
  `=?UTF-8?B?...?=`, and `-MQ` encodes as `=?UTF-8?Q?...?=`
- Fullwidth/halfwidth conversion: `-Z`, `-Z0`, `-Z1`, `-Z2`, `-Z3`, `-Z4`,
  `-X`, `-x`
- Encode fallback: `--fb-skip`, `--fb-html`, `--fb-xml`, `--fb-java`,
  `--fb-perl`, `--fb-subchar`, `--fb-subchar=...`
- Folding: `-f`, `-f60`, `-f60-10`, `-F`
- Unicode normalization: `--utf8mac-input` uses the `unicode-normalization`
  crate to compose UTF-8-MAC-style input to NFC
- Newlines: `-Lu`, `-Lw`, `-Lm`, `-d`, `-c`
- File output: `-O`
- Overwrite: `--overwrite[=SUFFIX]`, `--in-place[=SUFFIX]`
- Help/version: `--help`, `--version`

The original C implementation contains detailed MIME handling, JIS X 0213
tables, CP932 compatibility tables, Unicode normalization, and several smaller
conversion flags. Those are not fully ported in this first Rust version.

`--overwrite` preserves the original file permissions and timestamps.
`--in-place` preserves permissions but updates timestamps. If `SUFFIX` contains
`*`, every `*` is replaced with the original file name, matching nkf's backup
suffix behavior.

Conversion priority:

- `encoding_rs`: `UTF-8`, `Shift_JIS`/`CP932`, `EUC-JP`, `ISO-2022-JP`
- `base64`: `-mB`, `-MB`
- `unicode-normalization`: `--utf8mac-input`
- `iconv(3)` fallback: `UTF-16*`, `UTF-32*`, and other system-supported names
