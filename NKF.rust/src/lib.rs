use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use encoding_rs::Encoding;
use filetime::{set_file_times, FileTime};
use kanaria::string::UCSStr;
use kanaria::utils::{ConvertTarget, KanaUtils};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_normalization::UnicodeNormalization;

mod utf8_tables;

const NKF_VERSION: &str = "2.1.5-rust";
const DEFAULT_FOLD: usize = 60;
const DEFAULT_FOLD_MARGIN: usize = 10;

const SCORE_L2: u32 = 1;
const SCORE_KANA: u32 = SCORE_L2 << 1;
const SCORE_DEPEND: u32 = SCORE_KANA << 1;
const SCORE_CP932: u32 = SCORE_DEPEND << 1;
const SCORE_X0212: u32 = SCORE_CP932 << 1;
const SCORE_X0213: u32 = SCORE_X0212 << 1;
const SCORE_NO_EXIST: u32 = SCORE_X0213 << 1;
const SCORE_I_MIME: u32 = SCORE_NO_EXIST << 1;
const SCORE_ERROR: u32 = SCORE_I_MIME << 1;
const SCORE_INIT: u32 = SCORE_I_MIME;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    Preserve,
    Unix,
    Windows,
    Mac,
}

// #[derive(Clone, Debug)]
#[derive(Copy, Eq, PartialEq)]
pub enum MimeDecodeMode {
    Off,
    Strict,
    Nonstrict,
    Base64,
    QuotedPrintable,
}

impl Clone for MimeDecodeMode {
    fn clone(&self) -> Self {
        *self
    }
}
impl std::fmt::Debug for MimeDecodeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MimeDecodeMode::Off => write!(f, "Off"),
            MimeDecodeMode::Strict => write!(f, "Strict"),
            MimeDecodeMode::Nonstrict => write!(f, "Nonstrict"),
            MimeDecodeMode::Base64 => write!(f, "Base64"),
            MimeDecodeMode::QuotedPrintable => write!(f, "QuotedPrintable"),
        }
    }
}

#[derive(Copy, Eq, PartialEq)]
pub enum MimeEncodeMode {
    None,
    Base64,
    QuotedPrintable,
    Auto,
}
impl Clone for MimeEncodeMode {
    fn clone(&self) -> Self {
        *self
    }
}
impl std::fmt::Debug for MimeEncodeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MimeEncodeMode::None => write!(f, "None"),
            MimeEncodeMode::Base64 => write!(f, "Base64"),
            MimeEncodeMode::QuotedPrintable => write!(f, "QuotedPrintable"),
            MimeEncodeMode::Auto => write!(f, "Auto"),
        }
    }
}

pub struct Config {
    pub input: Option<String>,
    pub output: String,
    pub guess: u8,
    pub transparent: bool,
    pub no_output: bool,
    pub line_ending: LineEnding,
    pub fold: Option<FoldConfig>,
    pub normalize_utf8_mac: bool,
    pub mime_decode_mode: MimeDecodeMode,
    pub mime_encode_mode: MimeEncodeMode,
    pub z_flags: u8,
    pub x0201_mode: X0201Mode,
    pub kana_flags: u8,
    pub rot: bool,
    pub cap_input: bool,
    pub url_input: bool,
    pub numchar_input: bool,
    pub broken_jis_flags: u8,
    pub iso2022jp_strict: bool,
    pub iso2022_kanji_intro: Option<u8>,
    pub iso2022_ascii_intro: Option<u8>,
    pub cp932_compat: bool,
    pub cp932inv: bool,
    pub no_cp932ext: bool,
    pub no_best_fit_chars: bool,
    pub ms_ucs_map: MsUcsMap,
    pub debug: bool,
    pub buffering: BufferingMode,
    pub exec_mode: Option<ExecMode>,
    pub exec_command: Vec<String>,
    pub prefix_rules: Vec<(u8, u8)>,
    pub encode_fallback: EncodeFallback,
    pub file_output: Option<PathBuf>,
    pub overwrite: Option<OverwriteConfig>,
    pub inputs: Vec<PathBuf>,
}
impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            output: self.output.clone(),
            guess: self.guess,
            transparent: self.transparent,
            no_output: self.no_output,
            line_ending: self.line_ending,
            fold: self.fold,
            normalize_utf8_mac: self.normalize_utf8_mac,
            mime_decode_mode: self.mime_decode_mode,
            mime_encode_mode: self.mime_encode_mode,
            z_flags: self.z_flags,
            x0201_mode: self.x0201_mode,
            kana_flags: self.kana_flags,
            rot: self.rot,
            cap_input: self.cap_input,
            url_input: self.url_input,
            numchar_input: self.numchar_input,
            broken_jis_flags: self.broken_jis_flags,
            iso2022jp_strict: self.iso2022jp_strict,
            iso2022_kanji_intro: self.iso2022_kanji_intro,
            iso2022_ascii_intro: self.iso2022_ascii_intro,
            cp932_compat: self.cp932_compat,
            cp932inv: self.cp932inv,
            no_cp932ext: self.no_cp932ext,
            no_best_fit_chars: self.no_best_fit_chars,
            ms_ucs_map: self.ms_ucs_map,
            debug: self.debug,
            buffering: self.buffering,
            exec_mode: self.exec_mode,
            exec_command: self.exec_command.clone(),
            prefix_rules: self.prefix_rules.clone(),
            encode_fallback: self.encode_fallback.clone(),
            file_output: self.file_output.clone(),
            overwrite: self.overwrite.clone(),
            inputs: self.inputs.clone(),
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("guess", &self.guess)
            .field("transparent", &self.transparent)
            .field("no_output", &self.no_output)
            .field("line_ending", &self.line_ending)
            .field("fold", &self.fold)
            .field("normalize_utf8_mac", &self.normalize_utf8_mac)
            .field("mime_decode_mode", &self.mime_decode_mode)
            .field("mime_encode_mode", &self.mime_encode_mode)
            .field("z_flags", &self.z_flags)
            .field("x0201_mode", &self.x0201_mode)
            .field("kana_flags", &self.kana_flags)
            .field("rot", &self.rot)
            .field("cap_input", &self.cap_input)
            .field("url_input", &self.url_input)
            .field("numchar_input", &self.numchar_input)
            .field("broken_jis_flags", &self.broken_jis_flags)
            .field("iso2022jp_strict", &self.iso2022jp_strict)
            .field("iso2022_kanji_intro", &self.iso2022_kanji_intro)
            .field("iso2022_ascii_intro", &self.iso2022_ascii_intro)
            .field("cp932_compat", &self.cp932_compat)
            .field("cp932inv", &self.cp932inv)
            .field("no_cp932ext", &self.no_cp932ext)
            .field("no_best_fit_chars", &self.no_best_fit_chars)
            .field("ms_ucs_map", &self.ms_ucs_map)
            .field("debug", &self.debug)
            .field("buffering", &self.buffering)
            .field("exec_mode", &self.exec_mode)
            .field("exec_command", &self.exec_command)
            .field("prefix_rules", &self.prefix_rules)
            .field("encode_fallback", &self.encode_fallback)
            .field("file_output", &self.file_output)
            .field("overwrite", &self.overwrite)
            .field("inputs", &self.inputs)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldConfig {
    len: usize,
    margin: usize,
    preserve_newlines: bool,
}

impl FoldConfig {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverwriteConfig {
    pub preserve_time: bool,
    pub backup_suffix: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MimeEncoding {
    Base64,
    QuotedPrintable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X0201Mode {
    Default,
    Preserve,
    Fullwidth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MsUcsMap {
    Ascii,
    Ms,
    Cp932,
    Cp10001,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferingMode {
    Buffered,
    Unbuffered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecMode {
    Input,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeFallback {
    Skip,
    Html,
    Xml,
    Java,
    Perl,
    Subchar(char),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: None,
            output: "UTF-8".to_string(),
            guess: 0,
            transparent: false,
            no_output: false,
            line_ending: LineEnding::Preserve,
            fold: None,
            normalize_utf8_mac: false,
            mime_decode_mode: MimeDecodeMode::Off,
            mime_encode_mode: MimeEncodeMode::None,
            z_flags: 0,
            x0201_mode: X0201Mode::Default,
            kana_flags: 0,
            rot: false,
            cap_input: false,
            url_input: false,
            numchar_input: false,
            broken_jis_flags: 0,
            iso2022jp_strict: false,
            iso2022_kanji_intro: None,
            iso2022_ascii_intro: None,
            cp932_compat: true,
            cp932inv: true,
            no_cp932ext: false,
            no_best_fit_chars: false,
            ms_ucs_map: MsUcsMap::Ascii,
            debug: false,
            buffering: BufferingMode::Buffered,
            exec_mode: None,
            exec_command: Vec::new(),
            prefix_rules: Vec::new(),
            encode_fallback: EncodeFallback::Skip,
            file_output: None,
            overwrite: None,
            inputs: Vec::new(),
        }
    }
}

pub fn run_cli() {
    if let Err(err) = run() {
        eprintln!("nkf-rust: {err}");
        std::process::exit(1);
    }
}

pub fn convert_bytes(from: &str, to: &str, input: &[u8]) -> io::Result<Vec<u8>> {
    convert(from, to, input)
}

pub fn process_with_config(config: &Config, input: &[u8]) -> io::Result<Vec<u8>> {
    process_bytes(config, input)
}

pub fn guess_report_text(input: &[u8], level: u8) -> String {
    guess_report(input, level)
}

pub fn guess_encoding_name(input: &[u8]) -> &'static str {
    guess_encoding(input)
}

pub fn guess_file_encoding(path: impl AsRef<std::path::Path>) -> io::Result<&'static str> {
    let input = fs::read(path)?;
    Ok(guess_encoding(&input))
}

pub fn normalize_encoding_name(name: &str) -> String {
    normalize_encoding(name)
}

fn run() -> io::Result<()> {
    let config = parse_args(env::args().skip(1))?;

    if config.exec_mode.is_some() {
        return run_exec_mode(&config);
    }

    if config.overwrite.is_some() {
        return overwrite_files(&config);
    }

    if config.inputs.is_empty() {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        if config.guess != 0 {
            println!("{}", guess_report(&input, config.guess));
            return Ok(());
        }
        let output = process_bytes(&config, &input)?;
        write_output(&config, &output)?;
        flush_output_if_needed(&config)?;
        return Ok(());
    }

    if config.guess != 0 {
        for path in &config.inputs {
            let input = fs::read(path)?;
            if config.inputs.len() > 1 {
                println!("{}: {}", path.display(), guess_report(&input, config.guess));
            } else {
                println!("{}", guess_report(&input, config.guess));
            }
        }
        return Ok(());
    }

    let mut collected = Vec::new();
    for path in &config.inputs {
        let mut file = File::open(path)?;
        file.read_to_end(&mut collected)?;
        if config.inputs.len() > 1 {
            collected.push(b'\n');
        }
    }

    let output = process_bytes(&config, &collected)?;
    write_output(&config, &output)?;
    flush_output_if_needed(&config)
}

fn parse_args<I>(args: I) -> io::Result<Config>
where
    I: IntoIterator<Item = String>,
{
    let mut config = Config::default();
    let mut iter = args.into_iter().peekable();

    while let Some(arg) = iter.next() {
        if arg == "--" {
            config.inputs.extend(iter.map(PathBuf::from));
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            config.inputs.push(PathBuf::from(arg));
            continue;
        }
        if let Some(value) = arg.strip_prefix("--ic=") {
            config.input = Some(normalize_encoding(value));
            continue;
        }
        if let Some(value) = arg.strip_prefix("--oc=") {
            config.output = normalize_encoding(value);
            continue;
        }
        if arg == "--guess" {
            config.guess = 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--guess=") {
            config.guess = if value == "0" || value == "1" { 1 } else { 2 };
            continue;
        }
        if arg == "--utf8mac-input" {
            config.normalize_utf8_mac = true;
            continue;
        }
        if arg == "--hiragana" {
            config.kana_flags |= 1;
            continue;
        }
        if arg == "--katakana" {
            config.kana_flags |= 2;
            continue;
        }
        if arg == "--katakana-hiragana" {
            config.kana_flags |= 3;
            continue;
        }
        if arg == "--cap-input" {
            config.cap_input = true;
            continue;
        }
        if arg == "--url-input" {
            config.url_input = true;
            continue;
        }
        if arg == "--numchar-input" {
            config.numchar_input = true;
            continue;
        }
        if arg == "--no-output" {
            config.no_output = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--prefix=") {
            add_prefix_rule(&mut config, value);
            continue;
        }
        match arg.as_str() {
            "--cp932" => {
                config.cp932_compat = true;
                config.cp932inv = true;
                config.ms_ucs_map = MsUcsMap::Cp932;
                continue;
            }
            "--no-cp932" => {
                config.cp932_compat = false;
                config.cp932inv = false;
                config.ms_ucs_map = MsUcsMap::Ascii;
                continue;
            }
            "--cp932inv" => {
                config.cp932inv = true;
                continue;
            }
            "--no-cp932ext" => {
                config.no_cp932ext = true;
                continue;
            }
            "--no-best-fit-chars" => {
                config.no_best_fit_chars = true;
                continue;
            }
            "--ms-ucs-map" => {
                config.ms_ucs_map = MsUcsMap::Ms;
                continue;
            }
            "--debug" => {
                config.debug = true;
                continue;
            }
            "--exec-in" => {
                config.exec_mode = Some(ExecMode::Input);
                config.exec_command.extend(iter);
                break;
            }
            "--exec-out" => {
                config.exec_mode = Some(ExecMode::Output);
                config.exec_command.extend(iter);
                break;
            }
            "--x0212" => {
                continue;
            }
            _ => {}
        }
        if arg == "--fb-skip" {
            config.encode_fallback = EncodeFallback::Skip;
            continue;
        }
        if arg == "--fb-html" {
            config.encode_fallback = EncodeFallback::Html;
            continue;
        }
        if arg == "--fb-xml" {
            config.encode_fallback = EncodeFallback::Xml;
            continue;
        }
        if arg == "--fb-java" {
            config.encode_fallback = EncodeFallback::Java;
            continue;
        }
        if arg == "--fb-perl" {
            config.encode_fallback = EncodeFallback::Perl;
            continue;
        }
        if arg == "--fb-subchar" {
            config.encode_fallback = EncodeFallback::Subchar('?');
            continue;
        }
        if let Some(value) = arg.strip_prefix("--fb-subchar=") {
            config.encode_fallback = EncodeFallback::Subchar(parse_subchar(value)?);
            continue;
        }
        if arg == "--overwrite" {
            config.overwrite = Some(OverwriteConfig {
                preserve_time: true,
                backup_suffix: None,
            });
            continue;
        }
        if let Some(value) = arg.strip_prefix("--overwrite=") {
            config.overwrite = Some(OverwriteConfig {
                preserve_time: true,
                backup_suffix: Some(value.to_string()),
            });
            continue;
        }
        if arg == "--in-place" {
            config.overwrite = Some(OverwriteConfig {
                preserve_time: false,
                backup_suffix: None,
            });
            continue;
        }
        if let Some(value) = arg.strip_prefix("--in-place=") {
            config.overwrite = Some(OverwriteConfig {
                preserve_time: false,
                backup_suffix: Some(value.to_string()),
            });
            continue;
        }
        match arg.as_str() {
            "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--version" => {
                println!("{NKF_VERSION}");
                std::process::exit(0);
            }
            _ if arg.starts_with("--") => {
                if let Some(alias) = long_option_alias(&arg) {
                    parse_short_options(alias, &mut config, &mut iter)?;
                    continue;
                }
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported option: {arg}"),
                ));
            }
            _ => parse_short_options(&arg, &mut config, &mut iter)?,
        }
    }

    Ok(config)
}

fn long_option_alias(option: &str) -> Option<&'static str> {
    match option {
        "--base64" => Some("jMB"),
        "--euc" => Some("e"),
        "--euc-input" => Some("E"),
        "--fj" => Some("jm"),
        "--jis" => Some("j"),
        "--jis-input" => Some("J"),
        "--mac" => Some("sLm"),
        "--mime" => Some("jM"),
        "--mime-input" => Some("m"),
        "--msdos" => Some("sLw"),
        "--sjis" => Some("s"),
        "--sjis-input" => Some("S"),
        "--unix" => Some("eLu"),
        "--windows" => Some("sLw"),
        "--utf8" => Some("w"),
        "--utf8-input" => Some("W"),
        "--utf16" => Some("w16"),
        "--utf16-input" => Some("W16"),
        _ => None,
    }
}

fn add_prefix_rule(config: &mut Config, value: &str) {
    let bytes = value.as_bytes();
    if let Some((&prefix, rest)) = bytes.split_first() {
        config
            .prefix_rules
            .extend(rest.iter().copied().map(|target| (target, prefix)));
    }
}

fn parse_short_options<I>(
    arg: &str,
    config: &mut Config,
    iter: &mut std::iter::Peekable<I>,
) -> io::Result<()>
where
    I: Iterator<Item = String>,
{
    let body = arg.trim_start_matches('-');
    let mut i = 0;
    let bytes = body.as_bytes();

    while i < bytes.len() {
        match bytes[i] as char {
            'j' | 'n' => {
                config.output = "ISO-2022-JP".to_string();
                i += 1;
            }
            'e' => {
                config.output = "EUC-JP".to_string();
                i += 1;
            }
            's' => {
                config.output = "SHIFT_JIS".to_string();
                i += 1;
            }
            'w' => {
                let tail = &body[i + 1..];
                let (enc, consumed) = parse_w_family(tail, false);
                config.output = enc;
                i += 1 + consumed;
            }
            'J' => {
                config.input = Some("ISO-2022-JP".to_string());
                i += 1;
            }
            'E' => {
                config.input = Some("EUC-JP".to_string());
                i += 1;
            }
            'S' => {
                config.input = Some("SHIFT_JIS".to_string());
                i += 1;
            }
            'W' => {
                let tail = &body[i + 1..];
                let (enc, consumed) = parse_w_family(tail, true);
                config.input = Some(enc);
                i += 1 + consumed;
            }
            'g' => {
                config.guess = if matches!(bytes.get(i + 1), Some(b'0' | b'1')) {
                    1
                } else if matches!(bytes.get(i + 1), Some(b'2'..=b'9')) {
                    2
                } else {
                    1
                };
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            't' => {
                config.transparent = true;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            'l' => {
                config.input = Some("ISO-8859-1".to_string());
                i += 1;
            }
            'i' => {
                i += 1;
                if matches!(bytes.get(i), Some(b'@' | b'B')) {
                    config.iso2022_kanji_intro = Some(bytes[i]);
                    i += 1;
                }
            }
            'o' => {
                i += 1;
                if matches!(bytes.get(i), Some(b'J' | b'B' | b'H')) {
                    config.iso2022_ascii_intro = Some(bytes[i]);
                    i += 1;
                }
            }
            'I' => {
                config.output = "ISO-2022-JP".to_string();
                config.iso2022jp_strict = true;
                i += 1;
            }
            'T' => {
                i += 1;
            }
            'V' => {
                print_configuration();
                std::process::exit(0);
            }
            'v' => {
                println!("{NKF_VERSION}");
                std::process::exit(0);
            }
            'b' => {
                config.buffering = BufferingMode::Buffered;
                i += 1;
            }
            'u' => {
                config.buffering = BufferingMode::Unbuffered;
                i += 1;
            }
            'L' => {
                let tail = &body[i + 1..];
                if tail.starts_with('u') || tail.starts_with('f') {
                    config.line_ending = LineEnding::Unix;
                    i += 2;
                } else if tail.starts_with('w') {
                    config.line_ending = LineEnding::Windows;
                    i += 2;
                } else if tail.starts_with('m') {
                    config.line_ending = LineEnding::Mac;
                    i += 2;
                } else {
                    config.line_ending = LineEnding::Unix;
                    i += 1;
                }
            }
            'd' => {
                config.line_ending = LineEnding::Unix;
                i += 1;
            }
            'c' => {
                config.line_ending = LineEnding::Windows;
                i += 1;
            }
            'F' => {
                let tail = &body[i + 1..];
                let (fold, consumed) = parse_fold(tail, true);
                config.fold = Some(fold);
                i += 1 + consumed;
            }
            'f' => {
                let tail = &body[i + 1..];
                let (fold, consumed) = parse_fold(tail, false);
                config.fold = Some(fold);
                i += 1 + consumed;
            }
            'O' => {
                let tail = &body[i + 1..];
                let path = if tail.is_empty() {
                    iter.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "-O requires a file name")
                    })?
                } else {
                    tail.to_string()
                };
                config.file_output = Some(PathBuf::from(path));
                break;
            }
            'm' => {
                if matches!(bytes.get(i + 1), Some(b'B')) {
                    config.mime_decode_mode = MimeDecodeMode::Base64;
                    i += 2;
                } else if matches!(bytes.get(i + 1), Some(b'Q')) {
                    config.mime_decode_mode = MimeDecodeMode::QuotedPrintable;
                    i += 2;
                } else if matches!(bytes.get(i + 1), Some(b'S')) {
                    config.mime_decode_mode = MimeDecodeMode::Strict;
                    i += 2;
                } else if matches!(bytes.get(i + 1), Some(b'N')) {
                    config.mime_decode_mode = MimeDecodeMode::Nonstrict;
                    i += 2;
                } else if matches!(bytes.get(i + 1), Some(b'0')) {
                    config.mime_decode_mode = MimeDecodeMode::Off;
                    i += 2;
                } else {
                    config.mime_decode_mode = MimeDecodeMode::Strict;
                    i += 1;
                }
            }
            'M' => {
                if matches!(bytes.get(i + 1), Some(b'B')) {
                    config.mime_encode_mode = MimeEncodeMode::Base64;
                    i += 2;
                } else if matches!(bytes.get(i + 1), Some(b'Q')) {
                    config.mime_encode_mode = MimeEncodeMode::QuotedPrintable;
                    i += 2;
                } else {
                    config.mime_encode_mode = MimeEncodeMode::Auto;
                    i += 1;
                }
            }
            'Z' => {
                let mut saw_digit = false;
                i += 1;
                while i < bytes.len() && matches!(bytes[i], b'0'..=b'4') {
                    saw_digit = true;
                    config.z_flags |= 1 << (bytes[i] - b'0');
                    i += 1;
                }
                config.z_flags |= 1;
                if !saw_digit {
                    config.z_flags |= 1;
                }
            }
            'X' => {
                config.x0201_mode = X0201Mode::Fullwidth;
                i += 1;
            }
            'x' => {
                config.x0201_mode = X0201Mode::Preserve;
                i += 1;
            }
            'h' => {
                if let Some(digit @ b'0'..=b'9') = bytes.get(i + 1).copied() {
                    config.kana_flags |= digit - b'0';
                    i += 2;
                } else {
                    config.kana_flags |= 1;
                    i += 1;
                }
            }
            'r' => {
                config.rot = true;
                i += 1;
            }
            'B' => {
                if let Some(digit @ b'0'..=b'9') = bytes.get(i + 1).copied() {
                    config.broken_jis_flags |= 1 << (digit - b'0');
                    i += 2;
                } else {
                    config.broken_jis_flags |= 1;
                    i += 1;
                }
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported option: -{other}"),
                ));
            }
        }
    }

    Ok(())
}

fn parse_w_family(tail: &str, input_side: bool) -> (String, usize) {
    if input_side {
        if tail.starts_with("32L") {
            return ("UTF-32LE".to_string(), 3);
        }
        if tail.starts_with("32B") {
            return ("UTF-32BE".to_string(), 3);
        }
        if tail.starts_with("32") {
            return ("UTF-32".to_string(), 2);
        }
        if tail.starts_with("16L") {
            return ("UTF-16LE".to_string(), 3);
        }
        if tail.starts_with("16B") {
            return ("UTF-16BE".to_string(), 3);
        }
        if tail.starts_with("16") {
            return ("UTF-16".to_string(), 2);
        }
        if tail.starts_with("8") {
            return ("UTF-8".to_string(), 1);
        }
        return ("UTF-8".to_string(), 0);
    }

    if tail.starts_with("32L0") {
        return ("UTF-32LE".to_string(), 4);
    }
    if tail.starts_with("32B0") {
        return ("UTF-32BE".to_string(), 4);
    }
    if tail.starts_with("32L") {
        return ("UTF-32LE-BOM".to_string(), 3);
    }
    if tail.starts_with("32B") {
        return ("UTF-32BE-BOM".to_string(), 3);
    }
    if tail.starts_with("32") {
        return ("UTF-32".to_string(), 2);
    }
    if tail.starts_with("16L0") {
        return ("UTF-16LE".to_string(), 4);
    }
    if tail.starts_with("16B0") {
        return ("UTF-16BE".to_string(), 4);
    }
    if tail.starts_with("16L") {
        return ("UTF-16LE-BOM".to_string(), 3);
    }
    if tail.starts_with("16B") {
        return ("UTF-16BE-BOM".to_string(), 3);
    }
    if tail.starts_with("16") {
        return ("UTF-16".to_string(), 2);
    }
    if tail.starts_with("80") {
        return ("UTF-8".to_string(), 2);
    }
    if tail.starts_with("8") {
        return ("UTF-8-BOM".to_string(), 1);
    }
    ("UTF-8".to_string(), 0)
}

fn parse_fold(tail: &str, preserve_newlines: bool) -> (FoldConfig, usize) {
    let bytes = tail.as_bytes();
    let mut i = 0;
    let mut len = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        len = len * 10 + usize::from(bytes[i] - b'0');
        i += 1;
    }
    if len == 0 || len >= 8192 {
        len = DEFAULT_FOLD;
    }

    let mut margin = DEFAULT_FOLD_MARGIN;
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
        margin = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            margin = margin * 10 + usize::from(bytes[i] - b'0');
            i += 1;
        }
    }

    (
        FoldConfig {
            len,
            margin,
            preserve_newlines,
        },
        i,
    )
}

fn process_bytes(config: &Config, input: &[u8]) -> io::Result<Vec<u8>> {
    let broken_jis_input;
    let input = if config.broken_jis_flags != 0 {
        broken_jis_input = repair_broken_jis(input, config.broken_jis_flags);
        broken_jis_input.as_slice()
    } else {
        input
    };

    let preprocessed_input;
    let input = if config.cap_input || config.url_input || config.numchar_input {
        preprocessed_input = apply_input_preprocessors(config, input)?;
        preprocessed_input.as_slice()
    } else {
        input
    };

    // MIME decode (strict/nonstrict/base64/quoted-printable)
    let decoded_input;
    let input = match config.mime_decode_mode {
        MimeDecodeMode::Base64 => {
            decoded_input = decode_base64_input(input)?;
            decoded_input.as_slice()
        }
        MimeDecodeMode::QuotedPrintable => {
            decoded_input = decode_mime_q(input)?;
            decoded_input.as_slice()
        }
        MimeDecodeMode::Strict => {
            decoded_input = decode_mime_encoded_words_strict(input, true)?;
            decoded_input.as_slice()
        }
        MimeDecodeMode::Nonstrict => {
            decoded_input = decode_mime_encoded_words_strict(input, false)?;
            decoded_input.as_slice()
        }
        MimeDecodeMode::Off => input,
    };

    if config.transparent {
        let mut bytes = input.to_vec();
        if let Some(fold) = config.fold {
            bytes = fold_utf8_lossy(&bytes, fold).into_bytes();
        }
        bytes = apply_text_filters(config, &bytes);
        if config.rot {
            bytes = apply_rot_filter(&bytes);
        }
        bytes = apply_line_endings(bytes, config.line_ending);
        bytes = apply_prefix_rules(config, &bytes);
        if config.mime_encode_mode == MimeEncodeMode::Base64 {
            bytes = encode_mime_output(&bytes, MimeEncoding::Base64, &config.output);
        } else if config.mime_encode_mode == MimeEncodeMode::QuotedPrintable {
            bytes = encode_mime_output(&bytes, MimeEncoding::QuotedPrintable, &config.output);
        } else if config.mime_encode_mode == MimeEncodeMode::Auto {
            // Auto: use Base64 if non-ascii, else QP
            if bytes.iter().any(|&b| b > 0x7F) {
                bytes = encode_mime_output(&bytes, MimeEncoding::Base64, &config.output);
            } else {
                bytes = encode_mime_output(&bytes, MimeEncoding::QuotedPrintable, &config.output);
            }
        }
        return Ok(bytes);
    }

    let from = config
        .input
        .clone()
        .unwrap_or_else(|| guess_encoding(input).to_string());
    if config.debug {
        eprint!("{}", debug_report(config, Some(&from)));
    }

    let mut converted = if config.normalize_utf8_mac {
        let utf8 = convert(&from, "UTF-8", input)?;
        let normalized = String::from_utf8_lossy(&utf8).nfc().collect::<String>();
        if let Some(fold) = config.fold {
            let folded = fold_utf8_lossy(normalized.as_bytes(), fold);
            let filtered = apply_text_filters(config, folded.as_bytes());
            let filtered = apply_iso2022jp_strict_if_needed(config, &filtered);
            convert_for_config(config, "UTF-8", &config.output, &filtered)?
        } else {
            let filtered = apply_text_filters(config, normalized.as_bytes());
            let filtered = apply_iso2022jp_strict_if_needed(config, &filtered);
            convert_for_config(config, "UTF-8", &config.output, &filtered)?
        }
    } else if let Some(fold) = config.fold {
        let utf8 = convert(&from, "UTF-8", input)?;
        let folded = fold_utf8_lossy(&utf8, fold);
        let filtered = apply_text_filters(config, folded.as_bytes());
        let filtered = apply_iso2022jp_strict_if_needed(config, &filtered);
        convert_for_config(config, "UTF-8", &config.output, &filtered)?
    } else if config.z_flags != 0
        || config.x0201_mode == X0201Mode::Fullwidth
        || config.kana_flags != 0
        || should_apply_iso2022jp_strict(config)
    {
        let utf8 = convert(&from, "UTF-8", input)?;
        let filtered = apply_text_filters(config, &utf8);
        let filtered = apply_iso2022jp_strict_if_needed(config, &filtered);
        convert_for_config(config, "UTF-8", &config.output, &filtered)?
    } else {
        convert_for_config(config, &from, &config.output, input)?
    };
    if config.rot {
        converted = apply_rot_filter(&converted);
    }
    converted = apply_iso2022_intro_overrides(config, &converted);
    converted = apply_line_endings(converted, config.line_ending);
    converted = apply_prefix_rules(config, &converted);
    if config.mime_encode_mode == MimeEncodeMode::Base64 {
        converted = encode_mime_output(&converted, MimeEncoding::Base64, &config.output);
        return Ok(converted);
    } else if config.mime_encode_mode == MimeEncodeMode::QuotedPrintable {
        converted = encode_mime_output(&converted, MimeEncoding::QuotedPrintable, &config.output);
        return Ok(converted);
    } else if config.mime_encode_mode == MimeEncodeMode::Auto {
        if converted.iter().any(|&b| b > 0x7F) {
            converted = encode_mime_output(&converted, MimeEncoding::Base64, &config.output);
        } else {
            converted =
                encode_mime_output(&converted, MimeEncoding::QuotedPrintable, &config.output);
        }
        return Ok(converted);
    }
    Ok(converted)
}
// Strict/nonstrict MIME decode (RFC 2047/2822)
fn decode_mime_encoded_words_strict(input: &[u8], strict: bool) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if i + 2 <= input.len() && input[i] == b'=' && input.get(i + 1) == Some(&b'?') {
            if (!strict || is_strict_mime_word_prefix(&input[i..]))
                && decode_one_mime_word(&input[i..])?.is_some()
            {
                let (decoded, consumed) = decode_one_mime_word(&input[i..])?.expect("checked");
                out.extend_from_slice(&decoded);
                i += consumed;
                if let Some(lwsp) = consume_mime_lwsp(&input[i..]) {
                    let next = &input[i + lwsp..];
                    if next.starts_with(b"=?")
                        && (!strict || is_strict_mime_word_prefix(next))
                        && decode_one_mime_word(next)?.is_some()
                    {
                        i += lwsp;
                    }
                }
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    Ok(out)
}

// Quoted-printable decode (RFC 2045)
// (decode_mime_qは1つだけ定義)

fn decode_base64_input(input: &[u8]) -> io::Result<Vec<u8>> {
    let compact: Vec<u8> = input
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    BASE64_STANDARD
        .decode(compact)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("invalid base64: {err}")))
}

#[allow(dead_code)]
fn decode_mime_encoded_words(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if i + 2 <= input.len() && input[i] == b'=' && input.get(i + 1) == Some(&b'?') {
            if let Some((decoded, consumed)) = decode_one_mime_word(&input[i..])? {
                out.extend_from_slice(&decoded);
                i += consumed;
                while let Some(next) = consume_mime_lwsp(&input[i..]) {
                    if input.get(i + next) == Some(&b'=')
                        && input.get(i + next + 1) == Some(&b'?')
                        && decode_one_mime_word(&input[i + next..])?.is_some()
                    {
                        i += next;
                        break;
                    }
                    break;
                }
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    Ok(out)
}

fn decode_one_mime_word(input: &[u8]) -> io::Result<Option<(Vec<u8>, usize)>> {
    if !input.starts_with(b"=?") {
        return Ok(None);
    }
    let Some(charset_end) = find_byte(&input[2..], b'?') else {
        return Ok(None);
    };
    let charset = &input[2..2 + charset_end];
    let method_index = 2 + charset_end + 1;
    let Some(&method) = input.get(method_index) else {
        return Ok(None);
    };
    if input.get(method_index + 1) != Some(&b'?') {
        return Ok(None);
    }
    let encoded_start = method_index + 2;
    let Some(encoded_end_rel) = find_subslice(&input[encoded_start..], b"?=") else {
        return Ok(None);
    };
    let encoded = &input[encoded_start..encoded_start + encoded_end_rel];
    let raw = match method.to_ascii_uppercase() {
        b'B' => {
            let compact: Vec<u8> = encoded
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_whitespace())
                .collect();
            BASE64_STANDARD.decode(compact).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid MIME base64: {err}"),
                )
            })?
        }
        b'Q' => decode_mime_q(encoded)?,
        _ => return Ok(None),
    };
    let charset = std::str::from_utf8(charset)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let decoded = convert(charset, "UTF-8", &raw)?;
    Ok(Some((decoded, encoded_start + encoded_end_rel + 2)))
}

fn decode_mime_q(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'_' => {
                out.push(b' ');
                i += 1;
            }
            b'=' if matches!(input.get(i + 1), Some(b'\n')) => {
                i += 2;
            }
            b'=' if matches!(input.get(i + 1..i + 3), Some(b"\r\n")) => {
                i += 3;
            }
            b'=' if i + 2 < input.len() => {
                if let (Some(hi), Some(lo)) = (hex_value(input[i + 1]), hex_value(input[i + 2])) {
                    out.push((hi << 4) | lo);
                    i += 3;
                } else {
                    // =の直後2文字が16進でなければ=をそのまま出力
                    out.push(b'=');
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[allow(dead_code)]
fn encode_mime_word_output(input: &[u8], method: MimeEncoding) -> Vec<u8> {
    encode_mime_output(input, method, "UTF-8")
}

fn encode_mime_output(input: &[u8], method: MimeEncoding, charset: &str) -> Vec<u8> {
    let charset = mime_charset_name(charset);
    let marker = match method {
        MimeEncoding::Base64 => "B",
        MimeEncoding::QuotedPrintable => "Q",
    };
    let max_payload_len = 75usize.saturating_sub(charset.len() + marker.len() + 7);
    let max_payload_len = max_payload_len.max(4);

    let words = match method {
        MimeEncoding::Base64 => encode_mime_base64_words(input, charset, marker, max_payload_len),
        MimeEncoding::QuotedPrintable => {
            encode_mime_q_words(input, charset, marker, max_payload_len)
        }
    };
    words.join("\n ").into_bytes()
}

fn encode_mime_base64_words(
    input: &[u8],
    charset: &str,
    marker: &str,
    max_payload_len: usize,
) -> Vec<String> {
    let chunk_len = ((max_payload_len / 4).max(1) * 3).max(1);
    input
        .chunks(chunk_len)
        .map(|chunk| format!("=?{charset}?{marker}?{}?=", BASE64_STANDARD.encode(chunk)))
        .collect()
}

fn encode_mime_q_words(
    input: &[u8],
    charset: &str,
    marker: &str,
    max_payload_len: usize,
) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for &b in input {
        let encoded = mime_q_byte(b);
        if !current.is_empty() && current.len() + encoded.len() > max_payload_len {
            words.push(format!("=?{charset}?{marker}?{current}?="));
            current.clear();
        }
        current.push_str(&encoded);
    }
    if !current.is_empty() || input.is_empty() {
        words.push(format!("=?{charset}?{marker}?{current}?="));
    }
    words
}

fn mime_q_byte(b: u8) -> String {
    if b == b' ' {
        "_".to_string()
    } else if b.is_ascii_alphanumeric() {
        char::from(b).to_string()
    } else {
        let mut out = String::with_capacity(3);
        out.push('=');
        out.push(hex_char(b >> 4));
        out.push(hex_char(b & 0x0f));
        out
    }
}

fn mime_charset_name(charset: &str) -> &'static str {
    match normalize_encoding(charset).as_str() {
        "ISO-2022-JP" => "ISO-2022-JP",
        "EUC-JP" => "EUC-JP",
        "SHIFT_JIS" => "SHIFT_JIS",
        "CP932" => "CP932",
        "ISO-8859-1" => "ISO-8859-1",
        _ => "UTF-8",
    }
}

fn is_strict_mime_word_prefix(input: &[u8]) -> bool {
    let Some((charset, method)) = parse_mime_word_header(input) else {
        return false;
    };
    let charset = charset.replace('_', "-").to_ascii_uppercase();
    matches!(
        (charset.as_str(), method.to_ascii_uppercase()),
        ("EUC-JP", b'B')
            | ("SHIFT-JIS", b'B')
            | ("SHIFT_JIS", b'B')
            | ("ISO-8859-1", b'Q' | b'B')
            | ("ISO-2022-JP", b'B' | b'Q')
            | ("UTF-8", b'B' | b'Q')
            | ("US-ASCII", b'Q')
    )
}

fn parse_mime_word_header(input: &[u8]) -> Option<(String, u8)> {
    if !input.starts_with(b"=?") {
        return None;
    }
    let charset_end = find_byte(&input[2..], b'?')?;
    let charset = std::str::from_utf8(&input[2..2 + charset_end]).ok()?;
    let method_index = 2 + charset_end + 1;
    let method = *input.get(method_index)?;
    (input.get(method_index + 1) == Some(&b'?')).then(|| (charset.to_string(), method))
}

#[allow(dead_code)]
fn encode_mime_q(input: &[u8]) -> String {
    let mut out = String::new();
    for &b in input {
        if b == b' ' {
            out.push('_');
        } else if b.is_ascii_alphanumeric() {
            out.push(char::from(b));
        } else {
            out.push('=');
            out.push(hex_char(b >> 4));
            out.push(hex_char(b & 0x0f));
        }
    }
    out
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JisState {
    Ascii,
    Jis0208,
    Kana,
}

fn repair_broken_jis(input: &[u8], flags: u8) -> Vec<u8> {
    let repair_missing_escape = flags & 1 != 0;
    let allow_any_designator = flags & 2 != 0;
    let reset_on_newline = flags & 4 != 0;

    let mut out = Vec::with_capacity(input.len() + input.len() / 16);
    let mut state = JisState::Ascii;
    let mut last_was_escape = false;
    let mut i = 0;

    while i < input.len() {
        if input[i] == 0x1b && i + 2 < input.len() {
            let introducer = input[i + 1];
            let designator = input[i + 2];
            if introducer == b'$' && (designator == b'@' || designator == b'B') {
                out.extend_from_slice(&input[i..i + 3]);
                state = JisState::Jis0208;
                last_was_escape = true;
                i += 3;
                continue;
            }
            if introducer == b'(' && matches!(designator, b'B' | b'J' | b'I') {
                out.extend_from_slice(&input[i..i + 3]);
                state = if designator == b'I' {
                    JisState::Kana
                } else {
                    JisState::Ascii
                };
                last_was_escape = true;
                i += 3;
                continue;
            }
            if allow_any_designator && matches!(introducer, b'$' | b'(') {
                out.push(0x1b);
                out.push(introducer);
                out.push(b'B');
                state = if introducer == b'$' {
                    JisState::Jis0208
                } else {
                    JisState::Ascii
                };
                last_was_escape = true;
                i += 3;
                continue;
            }
        }

        if repair_missing_escape
            && input[i] == b'$'
            && !last_was_escape
            && matches!(state, JisState::Ascii | JisState::Kana)
            && matches!(input.get(i + 1), Some(b'@' | b'B'))
        {
            out.push(0x1b);
            out.push(input[i]);
            out.push(input[i + 1]);
            state = JisState::Jis0208;
            last_was_escape = false;
            i += 2;
            continue;
        }

        if repair_missing_escape
            && input[i] == b'('
            && !last_was_escape
            && matches!(state, JisState::Jis0208 | JisState::Kana)
            && matches!(input.get(i + 1), Some(b'B' | b'J'))
        {
            out.push(0x1b);
            out.push(input[i]);
            out.push(input[i + 1]);
            state = JisState::Ascii;
            last_was_escape = false;
            i += 2;
            continue;
        }

        if reset_on_newline && matches!(input[i], b'\n' | b'\r') && state != JisState::Ascii {
            out.extend_from_slice(b"\x1b(B");
            state = JisState::Ascii;
        }

        out.push(input[i]);
        last_was_escape = input[i] == 0x1b;
        i += 1;
    }

    out
}

fn apply_input_preprocessors(config: &Config, input: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = input.to_vec();
    if config.cap_input {
        out = decode_hex_marker_input(&out, b':');
    }
    if config.url_input {
        out = decode_hex_marker_input(&out, b'%');
    }
    if config.numchar_input {
        out = decode_numchar_input(&out)?;
    }
    Ok(out)
}

fn decode_hex_marker_input(input: &[u8], marker: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == marker && i + 2 < input.len() {
            if let (Some(hi), Some(lo)) = (hex_value(input[i + 1]), hex_value(input[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

fn decode_numchar_input(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'&' && input.get(i + 1) == Some(&b'#') {
            if let Some((ch, consumed)) = decode_one_numchar(&input[i..])? {
                let mut buf = [0_u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                i += consumed;
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    Ok(out)
}

fn decode_one_numchar(input: &[u8]) -> io::Result<Option<(char, usize)>> {
    if !input.starts_with(b"&#") {
        return Ok(None);
    }

    let mut i = 2;
    let radix = if matches!(input.get(i), Some(b'x' | b'X')) {
        i += 1;
        16
    } else {
        10
    };
    let digit_start = i;
    let max_digits = if radix == 16 { 7 } else { 8 };
    let mut value = 0_u32;
    while i < input.len() && i - digit_start < max_digits {
        let digit = if radix == 16 {
            hex_value(input[i])
        } else {
            input[i].is_ascii_digit().then_some(input[i] - b'0')
        };
        let Some(digit) = digit else {
            break;
        };
        value = value
            .checked_mul(radix)
            .and_then(|value| value.checked_add(u32::from(digit)))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "numeric character reference overflow",
                )
            })?;
        i += 1;
    }
    if i == digit_start {
        return Ok(None);
    }
    if matches!(input.get(i), Some(b';')) {
        i += 1;
    } else if i < input.len() && (input[i].is_ascii_alphanumeric() || input[i] == b'#') {
        return Ok(None);
    }

    Ok(char::from_u32(value).map(|ch| (ch, i)))
}

fn apply_text_filters(config: &Config, input: &[u8]) -> Vec<u8> {
    let mut bytes = apply_z_x_filters(config, input);
    if config.kana_flags != 0 {
        bytes = apply_hiragana_katakana_filter(config.kana_flags, &bytes).into_bytes();
    }
    bytes
}

fn should_apply_iso2022jp_strict(config: &Config) -> bool {
    config.iso2022jp_strict && normalize_encoding(&config.output) == "ISO-2022-JP"
}

fn apply_iso2022jp_strict_if_needed(config: &Config, input: &[u8]) -> Vec<u8> {
    if should_apply_iso2022jp_strict(config) {
        apply_iso2022jp_strict_filter(input)
    } else {
        input.to_vec()
    }
}

fn apply_iso2022jp_strict_filter(input: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(input);
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if iso2022jp_strict_safe_char(ch) {
            out.push(ch);
        } else {
            out.push('\u{3013}');
        }
    }
    out.into_bytes()
}

fn iso2022jp_strict_safe_char(ch: char) -> bool {
    if ch.is_ascii() {
        return true;
    }

    let mut buf = [0_u8; 4];
    let s = ch.encode_utf8(&mut buf);
    let Some(encoding) = encoding_rs_for_name("ISO-2022-JP") else {
        return false;
    };
    let (encoded, _, had_encode_errors) = encoding.encode(s);
    if had_encode_errors {
        return false;
    }
    iso2022jp_encoded_bytes_are_strict(&encoded)
}

fn iso2022jp_encoded_bytes_are_strict(bytes: &[u8]) -> bool {
    let mut i = 0;
    let mut in_jis_x0208 = false;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            match bytes.get(i + 1..i + 3) {
                Some(b"$B") | Some(b"$@") => {
                    in_jis_x0208 = true;
                    i += 3;
                    continue;
                }
                Some(b"(B") | Some(b"(J") => {
                    in_jis_x0208 = false;
                    i += 3;
                    continue;
                }
                _ => return false,
            }
        }

        if in_jis_x0208 {
            let Some((&c2, rest)) = bytes[i..].split_first() else {
                return false;
            };
            let Some(&c1) = rest.first() else {
                return false;
            };
            if !iso2022jp_strict_safe_jis_pair(c2, c1) {
                return false;
            }
            i += 2;
        } else if bytes[i] <= 0x7f {
            i += 1;
        } else {
            return false;
        }
    }
    true
}

fn iso2022jp_strict_safe_jis_pair(c2: u8, c1: u8) -> bool {
    if !(0x21..=0x74).contains(&c2) || !(0x21..=0x7e).contains(&c1) {
        return false;
    }
    let c = u16::from(c2) << 8 | u16::from(c1);
    !ISO2022JP_STRICT_REJECT_RANGES
        .iter()
        .any(|&(start, end)| start <= c && c <= end)
}

const ISO2022JP_STRICT_REJECT_RANGES: &[(u16, u16)] = &[
    (0x222f, 0x2239),
    (0x2242, 0x2249),
    (0x2251, 0x225b),
    (0x226b, 0x2271),
    (0x227a, 0x227d),
    (0x2321, 0x232f),
    (0x233a, 0x2340),
    (0x235b, 0x2360),
    (0x237b, 0x237e),
    (0x2474, 0x247e),
    (0x2577, 0x257e),
    (0x2639, 0x2640),
    (0x2659, 0x267e),
    (0x2742, 0x2750),
    (0x2772, 0x277e),
    (0x2841, 0x287e),
    (0x4f54, 0x4f7e),
    (0x7425, 0x747e),
];

fn apply_prefix_rules(config: &Config, input: &[u8]) -> Vec<u8> {
    if config.prefix_rules.is_empty() {
        return input.to_vec();
    }

    let mut out = Vec::with_capacity(input.len());
    for &byte in input {
        for &(target, prefix) in &config.prefix_rules {
            if byte == target {
                out.push(prefix);
                break;
            }
        }
        out.push(byte);
    }
    out
}

fn apply_iso2022_intro_overrides(config: &Config, input: &[u8]) -> Vec<u8> {
    if normalize_encoding(&config.output) != "ISO-2022-JP"
        || (config.iso2022_kanji_intro.is_none() && config.iso2022_ascii_intro.is_none())
    {
        return input.to_vec();
    }

    let mut out = input.to_vec();
    let kanji_intro = config.iso2022_kanji_intro.unwrap_or(b'B');
    let ascii_intro = config.iso2022_ascii_intro.unwrap_or(b'B');
    let mut i = 0;
    while i + 2 < out.len() {
        if out[i] == 0x1b {
            if out[i + 1] == b'$' && matches!(out[i + 2], b'@' | b'B') {
                out[i + 2] = kanji_intro;
                i += 3;
                continue;
            }
            if out[i + 1] == b'(' && matches!(out[i + 2], b'J' | b'B' | b'H') {
                out[i + 2] = ascii_intro;
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn apply_z_x_filters(config: &Config, input: &[u8]) -> Vec<u8> {
    if config.z_flags == 0 && config.x0201_mode != X0201Mode::Fullwidth {
        return input.to_vec();
    }

    let mut text = String::from_utf8_lossy(input).into_owned();
    if config.x0201_mode == X0201Mode::Fullwidth {
        text = kanaria_wide_katakana(&text);
    }

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if config.z_flags & 16 != 0 {
            if let Some(symbol) = fullwidth_kana_symbol_to_halfwidth(ch) {
                out.push_str(symbol);
                continue;
            }
            let narrowed = kanaria_narrow_katakana_char(ch);
            if narrowed != ch.to_string() {
                out.push_str(&narrowed);
                continue;
            }
        }

        if config.z_flags & 1 != 0 {
            if ch == '\u{3000}' {
                if config.z_flags & 4 != 0 {
                    out.push_str("  ");
                } else if config.z_flags & 2 != 0 {
                    out.push(' ');
                } else {
                    out.push(ch);
                }
                continue;
            }
            if let Some(ascii) = fullwidth_ascii_to_ascii(ch) {
                if config.z_flags & 8 != 0 {
                    match ascii {
                        '>' => out.push_str("&gt;"),
                        '<' => out.push_str("&lt;"),
                        '"' => out.push_str("&quot;"),
                        '&' => out.push_str("&amp;"),
                        _ => out.push(ascii),
                    }
                } else {
                    out.push(ascii);
                }
                continue;
            }
            if let Some(symbol) = fullwidth_symbol_to_ascii(ch) {
                out.push(symbol);
                continue;
            }
        }

        if config.z_flags & 8 != 0 {
            match ch {
                '>' => out.push_str("&gt;"),
                '<' => out.push_str("&lt;"),
                '"' => out.push_str("&quot;"),
                '&' => out.push_str("&amp;"),
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out.into_bytes()
}

fn apply_hiragana_katakana_filter(flags: u8, input: &[u8]) -> String {
    let text = String::from_utf8_lossy(input);
    text.chars()
        .map(|ch| match (flags & 1 != 0, flags & 2 != 0) {
            (true, true) => {
                if KanaUtils::is_can_convert_hiragana(ch) {
                    KanaUtils::convert_to_hiragana(ch)
                } else if KanaUtils::is_hiragana(ch) {
                    KanaUtils::convert_to_katakana(ch)
                } else {
                    ch
                }
            }
            (true, false) => KanaUtils::convert_to_hiragana(ch),
            (false, true) => KanaUtils::convert_to_katakana(ch),
            (false, false) => ch,
        })
        .collect()
}

fn apply_rot_filter(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    let mut jis_x0208 = false;
    while i < input.len() {
        if input[i] == 0x1b && i + 2 < input.len() {
            match (input[i + 1], input[i + 2]) {
                (b'$', b'@' | b'B') => jis_x0208 = true,
                (b'(', _) => jis_x0208 = false,
                _ => {}
            }
            out.extend_from_slice(&input[i..i + 3]);
            i += 3;
            continue;
        }

        let byte = if jis_x0208 {
            rot47_byte(input[i])
        } else {
            rot13_byte(input[i])
        };
        out.push(byte);
        i += 1;
    }
    out
}

fn rot13_byte(byte: u8) -> u8 {
    match byte {
        b'a'..=b'z' => b'a' + (byte - b'a' + 13) % 26,
        b'A'..=b'Z' => b'A' + (byte - b'A' + 13) % 26,
        _ => byte,
    }
}

fn rot47_byte(byte: u8) -> u8 {
    match byte {
        0x21..=0x7e => 0x21 + (byte - 0x21 + 47) % 94,
        _ => byte,
    }
}

fn kanaria_wide_katakana(input: &str) -> String {
    let widened = UCSStr::from_str(input)
        .wide(ConvertTarget::KATAKANA)
        .to_string();
    widened
        .chars()
        .map(|ch| match ch {
            '｡' => '。',
            '｢' => '「',
            '｣' => '」',
            '､' => '、',
            '･' => '・',
            _ => ch,
        })
        .collect()
}

fn kanaria_narrow_katakana_char(ch: char) -> String {
    UCSStr::from_str(&ch.to_string())
        .narrow(ConvertTarget::KATAKANA)
        .to_string()
}

fn fullwidth_ascii_to_ascii(ch: char) -> Option<char> {
    match ch {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0),
        _ => None,
    }
}

fn fullwidth_symbol_to_ascii(ch: char) -> Option<char> {
    match ch {
        '、' => Some(','),
        '。' => Some('.'),
        '，' => Some(','),
        '．' => Some('.'),
        '・' => Some('/'),
        '：' => Some(':'),
        '；' => Some(';'),
        '？' => Some('?'),
        '！' => Some('!'),
        '゛' => Some('`'),
        '゜' => Some('\''),
        '＾' => Some('^'),
        '＿' => Some('_'),
        '―' | '‐' | 'ー' => Some('-'),
        '￥' => Some('\\'),
        '〜' | '｜' => Some('|'),
        '‘' | '’' => Some('\''),
        '“' | '”' => Some('"'),
        '（' => Some('('),
        '）' => Some(')'),
        '［' | '「' => Some('['),
        '］' | '」' => Some(']'),
        '｛' => Some('{'),
        '｝' => Some('}'),
        '〈' => Some('<'),
        '〉' => Some('>'),
        '＋' => Some('+'),
        '−' => Some('-'),
        '＝' => Some('='),
        '＜' => Some('<'),
        '＞' => Some('>'),
        '＄' => Some('$'),
        '％' => Some('%'),
        '＃' => Some('#'),
        '＆' => Some('&'),
        '＊' => Some('*'),
        '＠' => Some('@'),
        _ => None,
    }
}

fn fullwidth_kana_symbol_to_halfwidth(ch: char) -> Option<&'static str> {
    match ch {
        '。' => Some("｡"),
        '「' => Some("｢"),
        '」' => Some("｣"),
        '、' => Some("､"),
        '・' => Some("･"),
        'ー' => Some("ｰ"),
        '゛' => Some("ﾞ"),
        '゜' => Some("ﾟ"),
        _ => None,
    }
}

#[allow(dead_code)]
fn consume_mime_lwsp(input: &[u8]) -> Option<usize> {
    let mut i = 0;
    while matches!(input.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        i += 1;
    }
    (i > 0).then_some(i)
}

fn find_byte(input: &[u8], byte: u8) -> Option<usize> {
    input.iter().position(|&b| b == byte)
}

fn find_subslice(input: &[u8], needle: &[u8]) -> Option<usize> {
    input
        .windows(needle.len())
        .position(|window| window == needle)
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'A' + nibble - 10),
    }
}

fn fallback_string(ch: char, fallback: &EncodeFallback) -> String {
    let code = ch as u32;
    match fallback {
        EncodeFallback::Skip => String::new(),
        EncodeFallback::Html => format!("&#{code};"),
        EncodeFallback::Xml => format!("&#x{code:X};"),
        EncodeFallback::Java => {
            if code <= 0xffff {
                format!("\\u{code:04X}")
            } else {
                let scalar = code - 0x1_0000;
                let high = 0xd800 + (scalar >> 10);
                let low = 0xdc00 + (scalar & 0x3ff);
                format!("\\u{high:04X}\\u{low:04X}")
            }
        }
        EncodeFallback::Perl => format!("\\x{{{code:X}}}"),
        EncodeFallback::Subchar(subchar) => subchar.to_string(),
    }
}

fn parse_subchar(value: &str) -> io::Result<char> {
    let code = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16)
    } else if value.starts_with('0') && value.len() > 1 {
        u32::from_str_radix(&value[1..], 8)
    } else {
        value.parse::<u32>()
    }
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    char::from_u32(code).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Unicode scalar value: {value}"),
        )
    })
}

fn overwrite_files(config: &Config) -> io::Result<()> {
    let overwrite = config.overwrite.as_ref().expect("checked by caller");
    if config.inputs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "overwrite requires at least one file",
        ));
    }
    if config.file_output.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "-O cannot be combined with overwrite",
        ));
    }

    for path in &config.inputs {
        let input = fs::read(path)?;
        let metadata = fs::metadata(path)?;
        let output = process_bytes(config, &input)?;
        let temp_path = make_temp_path(path);

        {
            let mut temp = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            temp.write_all(&output)?;
            temp.sync_all()?;
        }
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(metadata.mode()))?;

        if let Some(suffix) = &overwrite.backup_suffix {
            let backup = backup_filename(suffix, &path.to_string_lossy());
            fs::rename(path, backup)?;
            fs::rename(&temp_path, path)?;
        } else {
            fs::rename(&temp_path, path)?;
        }

        if overwrite.preserve_time {
            preserve_times(path, &metadata)?;
        }
    }

    Ok(())
}

fn make_temp_path(path: &PathBuf) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "nkf-rust".into());
    name.push(format!(".nkftmp.{pid}.{stamp}"));
    path.with_file_name(name)
}

fn backup_filename(suffix: &str, filename: &str) -> String {
    if suffix.contains('*') {
        suffix.replace('*', filename)
    } else {
        format!("{filename}{suffix}")
    }
}

fn preserve_times(path: &PathBuf, metadata: &fs::Metadata) -> io::Result<()> {
    let atime = FileTime::from_last_access_time(metadata);
    let mtime = FileTime::from_last_modification_time(metadata);
    set_file_times(path, atime, mtime)
}

fn fold_utf8_lossy(bytes: &[u8], config: FoldConfig) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    let mut line_width: usize = 0;
    let mut pending_space = false;
    let mut previous_was_newline = false;

    for ch in text.chars() {
        match ch {
            '\r' => continue,
            '\n' => {
                if config.preserve_newlines {
                    trim_trailing_space(&mut out);
                    out.push('\n');
                    line_width = 0;
                    pending_space = false;
                    previous_was_newline = true;
                } else if previous_was_newline {
                    trim_trailing_space(&mut out);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push('\n');
                    line_width = 0;
                    pending_space = false;
                } else {
                    pending_space = line_width > 0;
                    previous_was_newline = true;
                }
                continue;
            }
            '\x08' => {
                line_width = line_width.saturating_sub(1);
                out.push(ch);
                previous_was_newline = false;
                continue;
            }
            '\x0c' => {
                trim_trailing_space(&mut out);
                out.push('\n');
                line_width = 0;
                pending_space = false;
                previous_was_newline = true;
                continue;
            }
            ch if ch.is_whitespace() => {
                pending_space = line_width > 0;
                previous_was_newline = false;
                continue;
            }
            _ => {}
        }

        let width = display_width(ch);
        if pending_space {
            if line_width + 1 > config.len {
                trim_trailing_space(&mut out);
                out.push('\n');
                line_width = 0;
            } else if !out.ends_with('\n') && !out.is_empty() {
                out.push(' ');
                line_width += 1;
            }
            pending_space = false;
        }

        if line_width > 0 && line_width + width > config.len + config.margin {
            trim_trailing_space(&mut out);
            out.push('\n');
            line_width = 0;
        } else if line_width > 0 && line_width + width > config.len && !is_no_start_punctuation(ch)
        {
            trim_trailing_space(&mut out);
            out.push('\n');
            line_width = 0;
        }

        out.push(ch);
        line_width += width;
        previous_was_newline = false;
    }

    trim_trailing_space(&mut out);
    out
}

fn trim_trailing_space(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

fn display_width(ch: char) -> usize {
    if ch.is_ascii() {
        1
    } else {
        2
    }
}

fn is_no_start_punctuation(ch: char) -> bool {
    matches!(
        ch,
        ')' | ']'
            | '}'
            | '.'
            | ','
            | '!'
            | '?'
            | '/'
            | ':'
            | ';'
            | '、'
            | '。'
            | '，'
            | '．'
            | '」'
            | '』'
            | '）'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | 'ー'
    )
}

fn write_output(config: &Config, output: &[u8]) -> io::Result<()> {
    if config.no_output {
        return Ok(());
    }
    if let Some(path) = &config.file_output {
        let mut file = File::create(path)?;
        file.write_all(output)
    } else {
        io::stdout().write_all(output)
    }
}

fn flush_output_if_needed(config: &Config) -> io::Result<()> {
    if config.buffering == BufferingMode::Unbuffered {
        io::stdout().flush()?;
    }
    Ok(())
}

fn run_exec_mode(config: &Config) -> io::Result<()> {
    if config.exec_command.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--exec-in/--exec-out requires a command",
        ));
    }

    match config.exec_mode {
        Some(ExecMode::Input) => run_exec_in(config),
        Some(ExecMode::Output) => run_exec_out(config),
        None => Ok(()),
    }
}

fn run_exec_in(config: &Config) -> io::Result<()> {
    let output = Command::new(&config.exec_command[0])
        .args(&config.exec_command[1..])
        .output()
        .map_err(|err| io::Error::new(err.kind(), format!("exec-in failed to start: {err}")))?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("exec-in command exited with {}", output.status),
        ));
    }

    let converted = process_bytes(config, &output.stdout)?;
    write_output(config, &converted)?;
    flush_output_if_needed(config)
}

fn run_exec_out(config: &Config) -> io::Result<()> {
    let mut input = Vec::new();
    if config.inputs.is_empty() {
        io::stdin().read_to_end(&mut input)?;
    } else {
        for path in &config.inputs {
            let mut file = File::open(path)?;
            file.read_to_end(&mut input)?;
            if config.inputs.len() > 1 {
                input.push(b'\n');
            }
        }
    }

    let converted = process_bytes(config, &input)?;
    if config.no_output {
        return Ok(());
    }

    let mut child = Command::new(&config.exec_command[0])
        .args(&config.exec_command[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| io::Error::new(err.kind(), format!("exec-out failed to start: {err}")))?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "exec-out command stdin is closed",
            )
        })?;
        stdin.write_all(&converted)?;
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("exec-out command exited with {status}"),
        ));
    }
    flush_output_if_needed(config)
}

fn convert(from: &str, to: &str, input: &[u8]) -> io::Result<Vec<u8>> {
    if same_encoding(from, to) {
        return Ok(input.to_vec());
    }

    if let Some(output) = convert_with_nkf_euc_tables(from, to, input, None)? {
        return Ok(output);
    }

    if let Some(output) = convert_with_encoding_rs(from, to, input)? {
        return Ok(output);
    }

    if let Some(output) = convert_with_unicode_encoding(from, to, input)? {
        return Ok(output);
    }

    if let Some(output) = convert_with_jis_x0213(from, to, input)? {
        return Ok(output);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "unsupported conversion: {} to {}",
            normalize_encoding(from),
            normalize_encoding(to)
        ),
    ))
}

type EucToUtf8Table = [Option<&'static [u16; 94]>; 94];
type Utf8ToEuc2Table = [Option<&'static [u16; 64]>; 112];
type Utf8ToEuc3Table = [Option<&'static [Option<&'static [u16; 64]>; 64]>; 16];

fn convert_with_nkf_euc_tables(
    from: &str,
    to: &str,
    input: &[u8],
    config: Option<&Config>,
) -> io::Result<Option<Vec<u8>>> {
    let from = normalize_encoding(from);
    let to = normalize_encoding(to);

    match (is_euc_table_encoding(&from), is_euc_table_encoding(&to)) {
        (true, false) if to == "UTF-8" => {
            let Some(text) = decode_euc_jp_with_nkf_tables(&from, input, config) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid byte sequence for {from}"),
                ));
            };
            Ok(Some(text.into_bytes()))
        }
        (false, true) if from == "UTF-8" => {
            let text = std::str::from_utf8(input).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid byte sequence for UTF-8: {err}"),
                )
            })?;
            let Some(output) = encode_euc_jp_with_nkf_tables(&to, text, config) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode some characters as {to}"),
                ));
            };
            Ok(Some(output))
        }
        (true, true) => {
            let Some(text) = decode_euc_jp_with_nkf_tables(&from, input, config) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid byte sequence for {from}"),
                ));
            };
            let Some(output) = encode_euc_jp_with_nkf_tables(&to, &text, config) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode some characters as {to}"),
                ));
            };
            Ok(Some(output))
        }
        _ => Ok(None),
    }
}

fn is_euc_table_encoding(name: &str) -> bool {
    matches!(
        normalize_encoding(name).as_str(),
        "EUC-JP"
            | "CP51932"
            | "EUCJP-MS"
            | "EUCJP-ASCII"
            | "CP10001"
            | "EUC-JISX0213"
            | "EUC-JIS-2004"
    )
}

fn decode_euc_jp_with_nkf_tables(
    encoding: &str,
    input: &[u8],
    config: Option<&Config>,
) -> Option<String> {
    let kanji_table = euc_decode_table_for(encoding, config);
    let x0212_table = x0212_decode_table_for(encoding);
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            byte if byte <= 0x7f => {
                out.push(char::from(byte));
                i += 1;
            }
            0x8e => {
                let byte = *input.get(i + 1)?;
                out.push(utf8_tables::jis_x0201_kana_to_unicode(byte)?);
                i += 2;
            }
            0x8f => {
                let lead = *input.get(i + 1)?;
                let trail = *input.get(i + 2)?;
                if !(0xa1..=0xfe).contains(&lead) || !(0xa1..=0xfe).contains(&trail) {
                    return None;
                }
                out.push(utf8_tables::euc_to_unicode_with_table(
                    x0212_table,
                    lead,
                    trail,
                )?);
                i += 3;
            }
            0xa1..=0xfe => {
                let trail = *input.get(i + 1)?;
                if !(0xa1..=0xfe).contains(&trail) {
                    return None;
                }
                out.push(utf8_tables::euc_to_unicode_with_table(
                    kanji_table,
                    input[i],
                    trail,
                )?);
                i += 2;
            }
            _ => return None,
        }
    }
    Some(out)
}

fn encode_euc_jp_with_nkf_tables(
    encoding: &str,
    text: &str,
    config: Option<&Config>,
) -> Option<Vec<u8>> {
    let (two_byte_table, three_byte_table) = euc_encode_tables_for(encoding, config);
    let mut out = Vec::new();
    for ch in text.chars() {
        if ch.is_ascii() {
            out.push(ch as u8);
        } else if let Some(kana) = utf8_tables::unicode_to_jis_x0201_kana(ch) {
            out.extend_from_slice(&[0x8e, kana]);
        } else if let Some(euc) =
            utf8_tables::unicode_to_euc_with_tables(two_byte_table, three_byte_table, ch)
        {
            append_jis_table_value_as_euc(euc, &mut out)?;
        } else {
            return None;
        }
    }
    Some(out)
}

fn euc_decode_table_for(encoding: &str, config: Option<&Config>) -> &'static EucToUtf8Table {
    match normalize_encoding(encoding).as_str() {
        "EUC-JISX0213" | "EUC-JIS-2004" => &utf8_tables::EUC_TO_UTF8_2BYTES_X0213,
        "CP10001" => &utf8_tables::EUC_TO_UTF8_2BYTES_MAC,
        "EUCJP-MS" | "CP51932" => &utf8_tables::EUC_TO_UTF8_2BYTES_MS,
        "EUCJP-ASCII" => &utf8_tables::EUC_TO_UTF8_2BYTES,
        _ => match config.map(|config| config.ms_ucs_map) {
            Some(MsUcsMap::Ms | MsUcsMap::Cp932) => &utf8_tables::EUC_TO_UTF8_2BYTES_MS,
            Some(MsUcsMap::Cp10001) => &utf8_tables::EUC_TO_UTF8_2BYTES_MAC,
            _ => &utf8_tables::EUC_TO_UTF8_2BYTES,
        },
    }
}

fn x0212_decode_table_for(encoding: &str) -> &'static EucToUtf8Table {
    match normalize_encoding(encoding).as_str() {
        "EUC-JISX0213" | "EUC-JIS-2004" => &utf8_tables::X0212_TO_UTF8_2BYTES_X0213,
        _ => &utf8_tables::X0212_TO_UTF8_2BYTES,
    }
}

fn euc_encode_tables_for(
    encoding: &str,
    config: Option<&Config>,
) -> (&'static Utf8ToEuc2Table, &'static Utf8ToEuc3Table) {
    match normalize_encoding(encoding).as_str() {
        "EUC-JISX0213" | "EUC-JIS-2004" => (
            &utf8_tables::UTF8_TO_EUC_2BYTES_X0213,
            &utf8_tables::UTF8_TO_EUC_3BYTES_X0213,
        ),
        "CP10001" => (
            &utf8_tables::UTF8_TO_EUC_2BYTES_MAC,
            &utf8_tables::UTF8_TO_EUC_3BYTES_MAC,
        ),
        "CP51932" => (
            &utf8_tables::UTF8_TO_EUC_2BYTES_932,
            &utf8_tables::UTF8_TO_EUC_3BYTES_932,
        ),
        "EUCJP-MS" => (
            &utf8_tables::UTF8_TO_EUC_2BYTES_MS,
            &utf8_tables::UTF8_TO_EUC_3BYTES_MS,
        ),
        "EUCJP-ASCII" => (
            &utf8_tables::UTF8_TO_EUC_2BYTES,
            &utf8_tables::UTF8_TO_EUC_3BYTES,
        ),
        _ => match config.map(|config| config.ms_ucs_map) {
            Some(MsUcsMap::Ms) => (
                &utf8_tables::UTF8_TO_EUC_2BYTES_MS,
                &utf8_tables::UTF8_TO_EUC_3BYTES_MS,
            ),
            Some(MsUcsMap::Cp932) => (
                &utf8_tables::UTF8_TO_EUC_2BYTES_932,
                &utf8_tables::UTF8_TO_EUC_3BYTES_932,
            ),
            Some(MsUcsMap::Cp10001) => (
                &utf8_tables::UTF8_TO_EUC_2BYTES_MAC,
                &utf8_tables::UTF8_TO_EUC_3BYTES_MAC,
            ),
            _ => (
                &utf8_tables::UTF8_TO_EUC_2BYTES,
                &utf8_tables::UTF8_TO_EUC_3BYTES,
            ),
        },
    }
}

fn append_jis_table_value_as_euc(value: u16, out: &mut Vec<u8>) -> Option<()> {
    let [lead, trail] = value.to_be_bytes();
    if value > 0x7fff {
        out.extend_from_slice(&[0x8f, (lead & 0x7f) | 0x80, trail | 0x80]);
    } else {
        out.extend_from_slice(&[lead | 0x80, trail | 0x80]);
    }
    Some(())
}

fn convert_for_config(config: &Config, from: &str, to: &str, input: &[u8]) -> io::Result<Vec<u8>> {
    reject_no_best_fit_chars(config, from, to, input)?;

    let remapped_input;
    let input = if let Some(bytes) = remap_shiftjis_cp932_input(config, from, input) {
        remapped_input = bytes;
        remapped_input.as_slice()
    } else {
        input
    };

    let mut output =
        if let Some(output) = convert_with_nkf_euc_tables(from, to, input, Some(config))? {
            output
        } else if config.encode_fallback == EncodeFallback::Skip {
            convert(from, to, input)?
        } else if let Some(output) =
            convert_with_encoding_rs_fallback(from, to, input, &config.encode_fallback)?
        {
            output
        } else {
            convert(from, to, input)?
        };

    remap_shiftjis_cp932_output(config, to, &mut output);
    reject_cp932_extensions(config, to, &output)?;
    Ok(output)
}

fn remap_shiftjis_cp932_input(config: &Config, from: &str, input: &[u8]) -> Option<Vec<u8>> {
    if config.cp932inv || !is_shift_jis_family_output(from) {
        return None;
    }

    remap_shiftjis_pairs(input, |lead, trail| {
        if !(0xfa..=0xfc).contains(&lead) {
            return None;
        }
        let value = *utf8_tables::SHIFTJIS_CP932
            .get(usize::from(lead - 0xfa))?
            .get(usize::from(trail.checked_sub(0x40)?))?;
        nonzero_u16_to_bytes(value)
    })
}

fn remap_shiftjis_cp932_output(config: &Config, to: &str, output: &mut Vec<u8>) {
    if !config.cp932inv || !is_shift_jis_family_output(to) {
        return;
    }

    if let Some(remapped) = remap_shiftjis_pairs(output, |lead, trail| {
        if !(0xed..=0xee).contains(&lead) {
            return None;
        }
        let value = *utf8_tables::CP932INV
            .get(usize::from(lead - 0xed))?
            .get(usize::from(trail.checked_sub(0x40)?))?;
        nonzero_u16_to_bytes(value)
    }) {
        *output = remapped;
    }
}

fn remap_shiftjis_pairs<F>(input: &[u8], mut lookup: F) -> Option<Vec<u8>>
where
    F: FnMut(u8, u8) -> Option<[u8; 2]>,
{
    let mut out = Vec::with_capacity(input.len());
    let mut changed = false;
    let mut i = 0;
    while i < input.len() {
        let lead = input[i];
        if is_shift_jis_lead(lead) {
            let Some(&trail) = input.get(i + 1) else {
                out.push(lead);
                i += 1;
                continue;
            };
            if let Some([mapped_lead, mapped_trail]) = lookup(lead, trail) {
                out.extend_from_slice(&[mapped_lead, mapped_trail]);
                changed = true;
            } else {
                out.extend_from_slice(&[lead, trail]);
            }
            i += 2;
        } else {
            out.push(lead);
            i += 1;
        }
    }

    changed.then_some(out)
}

fn nonzero_u16_to_bytes(value: u16) -> Option<[u8; 2]> {
    (value != 0).then_some(value.to_be_bytes())
}

fn reject_no_best_fit_chars(config: &Config, from: &str, to: &str, input: &[u8]) -> io::Result<()> {
    if !config.no_best_fit_chars || !is_ms_compat_japanese_output(to) {
        return Ok(());
    }

    let text = if normalize_encoding(from) == "UTF-8" {
        String::from_utf8(input.to_vec()).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid byte sequence for UTF-8: {err}"),
            )
        })?
    } else {
        String::from_utf8(convert(from, "UTF-8", input)?).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid UTF-8 after decoding {from}: {err}"),
            )
        })?
    };

    if let Some(ch) = text.chars().find(|&ch| is_no_best_fit_char(ch)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("best-fit mapping disabled for {ch:?}"),
        ));
    }

    Ok(())
}

fn is_no_best_fit_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00a2}'
            | '\u{00a3}'
            | '\u{00a5}'
            | '\u{00a6}'
            | '\u{00ac}'
            | '\u{00af}'
            | '\u{00b8}'
            | '\u{2014}'
            | '\u{2016}'
            | '\u{203e}'
            | '\u{2212}'
            | '\u{301c}'
    )
}

fn reject_cp932_extensions(config: &Config, to: &str, output: &[u8]) -> io::Result<()> {
    if !is_shift_jis_family_output(to) || (config.cp932_compat && !config.no_cp932ext) {
        return Ok(());
    }

    if let Some((lead, trail)) = first_cp932_extension(output) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CP932 extension byte sequence disabled: {lead:02X} {trail:02X}"),
        ));
    }

    Ok(())
}

fn first_cp932_extension(input: &[u8]) -> Option<(u8, u8)> {
    let mut i = 0;
    while i < input.len() {
        let lead = input[i];
        if is_shift_jis_lead(lead) {
            let Some(&trail) = input.get(i + 1) else {
                return None;
            };
            if is_cp932_extension_lead(lead) {
                return Some((lead, trail));
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

fn is_shift_jis_lead(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

fn is_cp932_extension_lead(byte: u8) -> bool {
    matches!(byte, 0x87 | 0xed..=0xee | 0xfa..=0xfc)
}

fn is_ms_compat_japanese_output(name: &str) -> bool {
    matches!(
        normalize_encoding(name).as_str(),
        "CP932"
            | "SHIFT_JIS"
            | "CP51932"
            | "EUC-JP"
            | "EUCJP-MS"
            | "EUCJP-ASCII"
            | "CP50220"
            | "CP50221"
            | "CP50222"
            | "CP10001"
    )
}

fn is_shift_jis_family_output(name: &str) -> bool {
    matches!(normalize_encoding(name).as_str(), "CP932" | "SHIFT_JIS")
}

fn convert_with_encoding_rs(from: &str, to: &str, input: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let Some(from_encoding) = encoding_rs_for_name(from) else {
        return Ok(None);
    };
    let Some(to_encoding) = encoding_rs_for_name(to) else {
        return Ok(None);
    };

    let (decoded, _, had_decode_errors) = from_encoding.decode(input);
    if had_decode_errors {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid byte sequence for {}", from_encoding.name()),
        ));
    }

    let (encoded, _, had_encode_errors) = to_encoding.encode(&decoded);
    if had_encode_errors {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cannot encode some characters as {}", to_encoding.name()),
        ));
    }

    Ok(Some(encoded.into_owned()))
}

fn convert_with_encoding_rs_fallback(
    from: &str,
    to: &str,
    input: &[u8],
    fallback: &EncodeFallback,
) -> io::Result<Option<Vec<u8>>> {
    let Some(from_encoding) = encoding_rs_for_name(from) else {
        return Ok(None);
    };
    let Some(to_encoding) = encoding_rs_for_name(to) else {
        return Ok(None);
    };

    let (decoded, _, had_decode_errors) = from_encoding.decode(input);
    if had_decode_errors {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid byte sequence for {}", from_encoding.name()),
        ));
    }

    let mut out = Vec::new();
    for ch in decoded.chars() {
        let mut buf = [0; 4];
        let s = ch.encode_utf8(&mut buf);
        let (encoded, _, had_encode_errors) = to_encoding.encode(s);
        if had_encode_errors {
            let replacement = fallback_string(ch, fallback);
            if !replacement.is_empty() {
                let (encoded_replacement, _, replacement_errors) = to_encoding.encode(&replacement);
                if replacement_errors {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("fallback cannot be encoded as {}", to_encoding.name()),
                    ));
                }
                out.extend_from_slice(&encoded_replacement);
            }
        } else {
            out.extend_from_slice(&encoded);
        }
    }
    Ok(Some(out))
}

fn encoding_rs_for_name(name: &str) -> Option<&'static Encoding> {
    let normalized = normalize_encoding(name);
    let label = match normalized.as_str() {
        "UTF-8" | "EUC-JP" | "ISO-2022-JP" | "ISO-8859-1" => normalized.as_str(),
        "SHIFT_JIS" | "CP932" | "CP10001" => "windows-31j",
        "CP51932" | "EUCJP-MS" | "EUCJP-ASCII" => "EUC-JP",
        "CP50220" | "CP50221" | "CP50222" => "ISO-2022-JP",
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

fn convert_with_unicode_encoding(
    from: &str,
    to: &str,
    input: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    let from = normalize_encoding(from);
    let to = normalize_encoding(to);

    if !is_unicode_encoding(&from) && !is_unicode_encoding(&to) {
        return Ok(None);
    }

    let decoded = if is_unicode_encoding(&from) {
        decode_unicode_bytes(&from, input)?
    } else if let Some(from_encoding) = encoding_rs_for_name(&from) {
        let (decoded, _, had_decode_errors) = from_encoding.decode(input);
        if had_decode_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid byte sequence for {}", from_encoding.name()),
            ));
        }
        decoded.into_owned()
    } else {
        return Ok(None);
    };

    if is_unicode_encoding(&to) {
        return Ok(Some(encode_unicode_bytes(&to, &decoded)?));
    }

    let Some(to_encoding) = encoding_rs_for_name(&to) else {
        return Ok(None);
    };
    let (encoded, _, had_encode_errors) = to_encoding.encode(&decoded);
    if had_encode_errors {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cannot encode some characters as {}", to_encoding.name()),
        ));
    }
    Ok(Some(encoded.into_owned()))
}

fn convert_with_jis_x0213(from: &str, to: &str, input: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let from = normalize_encoding(from);
    let to = normalize_encoding(to);

    if !is_jis_x0213_encoding(&from) && !is_jis_x0213_encoding(&to) {
        return Ok(None);
    }

    let text = if from == "UTF-8" {
        String::from_utf8(input.to_vec()).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid byte sequence for UTF-8: {err}"),
            )
        })?
    } else if is_jis_x0213_encoding(&from) {
        decode_jis_x0213_subset(&from, input)?
    } else if let Some(from_encoding) = encoding_rs_for_name(&from) {
        let (decoded, _, had_decode_errors) = from_encoding.decode(input);
        if had_decode_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid byte sequence for {}", from_encoding.name()),
            ));
        }
        decoded.into_owned()
    } else {
        return Ok(None);
    };

    let output = if to == "UTF-8" {
        text.into_bytes()
    } else if is_jis_x0213_encoding(&to) {
        encode_jis_x0213_subset(&to, &text)?
    } else if let Some(to_encoding) = encoding_rs_for_name(&to) {
        let (encoded, _, had_encode_errors) = to_encoding.encode(&text);
        if had_encode_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cannot encode some characters as {}", to_encoding.name()),
            ));
        }
        encoded.into_owned()
    } else {
        return Ok(None);
    };

    Ok(Some(output))
}

fn is_jis_x0213_encoding(name: &str) -> bool {
    matches!(
        normalize_encoding(name).as_str(),
        "EUC-JISX0213"
            | "EUC-JIS-2004"
            | "SHIFT_JISX0213"
            | "SHIFT_JIS-2004"
            | "ISO-2022-JP-3"
            | "ISO-2022-JP-2004"
    )
}

fn decode_jis_x0213_subset(encoding: &str, input: &[u8]) -> io::Result<String> {
    match normalize_encoding(encoding).as_str() {
        "EUC-JISX0213" | "EUC-JIS-2004" => decode_euc_jis_x0213_subset(input),
        "SHIFT_JISX0213" | "SHIFT_JIS-2004" => decode_shift_jis_x0213_subset(input),
        "ISO-2022-JP-3" | "ISO-2022-JP-2004" => decode_iso_2022_jp_3_subset(input),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported JIS X 0213 encoding: {encoding}"),
        )),
    }
}

fn encode_jis_x0213_subset(encoding: &str, text: &str) -> io::Result<Vec<u8>> {
    match normalize_encoding(encoding).as_str() {
        "EUC-JISX0213" | "EUC-JIS-2004" => encode_euc_jis_x0213_subset(text),
        "SHIFT_JISX0213" | "SHIFT_JIS-2004" => encode_shift_jis_x0213_subset(text),
        "ISO-2022-JP-3" | "ISO-2022-JP-2004" => encode_iso_2022_jp_3_subset(text),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported JIS X 0213 encoding: {encoding}"),
        )),
    }
}

fn decode_euc_jis_x0213_subset(input: &[u8]) -> io::Result<String> {
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            byte if byte <= 0x7f => {
                out.push(byte as char);
                i += 1;
            }
            0xae if input.get(i + 1) == Some(&0xa1) => {
                out.push('俱');
                i += 2;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported EUC-JISX0213 sequence in pure Rust subset",
                ))
            }
        }
    }
    Ok(out)
}

fn encode_euc_jis_x0213_subset(text: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    for ch in text.chars() {
        match ch {
            '\u{00}'..='\u{7f}' => out.push(ch as u8),
            '俱' => out.extend_from_slice(&[0xae, 0xa1]),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode {ch:?} as EUC-JISX0213 in pure Rust subset"),
                ))
            }
        }
    }
    Ok(out)
}

fn decode_shift_jis_x0213_subset(input: &[u8]) -> io::Result<String> {
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            byte if byte <= 0x7f => {
                out.push(byte as char);
                i += 1;
            }
            0x87 if input.get(i + 1) == Some(&0x9f) => {
                out.push('俱');
                i += 2;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported Shift_JISX0213 sequence in pure Rust subset",
                ))
            }
        }
    }
    Ok(out)
}

fn encode_shift_jis_x0213_subset(text: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    for ch in text.chars() {
        match ch {
            '\u{00}'..='\u{7f}' => out.push(ch as u8),
            '俱' => out.extend_from_slice(&[0x87, 0x9f]),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode {ch:?} as Shift_JISX0213 in pure Rust subset"),
                ))
            }
        }
    }
    Ok(out)
}

fn decode_iso_2022_jp_3_subset(input: &[u8]) -> io::Result<String> {
    let mut out = String::new();
    let mut i = 0;
    let mut in_x0213_plane_1 = false;
    while i < input.len() {
        if input[i] == 0x1b {
            match input.get(i + 1..i + 4) {
                Some(b"$(Q") => {
                    in_x0213_plane_1 = true;
                    i += 4;
                    continue;
                }
                _ => match input.get(i + 1..i + 3) {
                    Some(b"(B") => {
                        in_x0213_plane_1 = false;
                        i += 3;
                        continue;
                    }
                    _ => {}
                },
            }
        }

        if in_x0213_plane_1 {
            if input.get(i..i + 2) == Some(&[0x2e, 0x21]) {
                out.push('俱');
                i += 2;
                continue;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported ISO-2022-JP-3 sequence in pure Rust subset",
            ));
        }

        match input[i] {
            byte if byte <= 0x7f => {
                out.push(byte as char);
                i += 1;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported ISO-2022-JP-3 byte in pure Rust subset",
                ))
            }
        }
    }
    Ok(out)
}

fn encode_iso_2022_jp_3_subset(text: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut in_x0213_plane_1 = false;
    for ch in text.chars() {
        match ch {
            '\u{00}'..='\u{7f}' => {
                if in_x0213_plane_1 {
                    out.extend_from_slice(b"\x1b(B");
                    in_x0213_plane_1 = false;
                }
                out.push(ch as u8);
            }
            '俱' => {
                if !in_x0213_plane_1 {
                    out.extend_from_slice(b"\x1b$(Q");
                    in_x0213_plane_1 = true;
                }
                out.extend_from_slice(&[0x2e, 0x21]);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cannot encode {ch:?} as ISO-2022-JP-3 in pure Rust subset"),
                ))
            }
        }
    }
    if in_x0213_plane_1 {
        out.extend_from_slice(b"\x1b(B");
    }
    Ok(out)
}

fn is_unicode_encoding(name: &str) -> bool {
    matches!(
        normalize_encoding(name).as_str(),
        "UTF-8"
            | "UTF-8-BOM"
            | "UTF-16"
            | "UTF-16LE"
            | "UTF-16LE-BOM"
            | "UTF-16BE"
            | "UTF-16BE-BOM"
            | "UTF-32"
            | "UTF-32LE"
            | "UTF-32LE-BOM"
            | "UTF-32BE"
            | "UTF-32BE-BOM"
    )
}

#[derive(Clone, Copy)]
enum Endian {
    Big,
    Little,
}

fn decode_unicode_bytes(encoding: &str, input: &[u8]) -> io::Result<String> {
    match normalize_encoding(encoding).as_str() {
        "UTF-8" => String::from_utf8(input.to_vec()).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid byte sequence for UTF-8: {err}"),
            )
        }),
        "UTF-16" => decode_utf16_auto(input),
        "UTF-16BE" => decode_utf16(input, Endian::Big),
        "UTF-16BE-BOM" => decode_utf16(input, Endian::Big),
        "UTF-16LE" => decode_utf16(input, Endian::Little),
        "UTF-16LE-BOM" => decode_utf16(input, Endian::Little),
        "UTF-32" => decode_utf32_auto(input),
        "UTF-32BE" => decode_utf32(input, Endian::Big),
        "UTF-32BE-BOM" => decode_utf32(input, Endian::Big),
        "UTF-32LE" => decode_utf32(input, Endian::Little),
        "UTF-32LE-BOM" => decode_utf32(input, Endian::Little),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Unicode encoding: {other}"),
        )),
    }
}

fn encode_unicode_bytes(encoding: &str, text: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    match normalize_encoding(encoding).as_str() {
        "UTF-8" => out.extend_from_slice(text.as_bytes()),
        "UTF-8-BOM" => {
            out.extend_from_slice(&[0xef, 0xbb, 0xbf]);
            out.extend_from_slice(text.as_bytes());
        }
        "UTF-16" => {
            out.extend_from_slice(&[0xfe, 0xff]);
            encode_utf16(text, Endian::Big, &mut out);
        }
        "UTF-16BE-BOM" => {
            out.extend_from_slice(&[0xfe, 0xff]);
            encode_utf16(text, Endian::Big, &mut out);
        }
        "UTF-16BE" => encode_utf16(text, Endian::Big, &mut out),
        "UTF-16LE-BOM" => {
            out.extend_from_slice(&[0xff, 0xfe]);
            encode_utf16(text, Endian::Little, &mut out);
        }
        "UTF-16LE" => encode_utf16(text, Endian::Little, &mut out),
        "UTF-32" => {
            out.extend_from_slice(&[0x00, 0x00, 0xfe, 0xff]);
            encode_utf32(text, Endian::Big, &mut out);
        }
        "UTF-32BE-BOM" => {
            out.extend_from_slice(&[0x00, 0x00, 0xfe, 0xff]);
            encode_utf32(text, Endian::Big, &mut out);
        }
        "UTF-32BE" => encode_utf32(text, Endian::Big, &mut out),
        "UTF-32LE-BOM" => {
            out.extend_from_slice(&[0xff, 0xfe, 0x00, 0x00]);
            encode_utf32(text, Endian::Little, &mut out);
        }
        "UTF-32LE" => encode_utf32(text, Endian::Little, &mut out),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported Unicode encoding: {other}"),
            ))
        }
    }
    Ok(out)
}

fn decode_utf16_auto(input: &[u8]) -> io::Result<String> {
    match input {
        [0xfe, 0xff, rest @ ..] => decode_utf16(rest, Endian::Big),
        [0xff, 0xfe, rest @ ..] => decode_utf16(rest, Endian::Little),
        _ => decode_utf16(input, Endian::Big),
    }
}

fn decode_utf16(input: &[u8], endian: Endian) -> io::Result<String> {
    if input.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "odd byte length for UTF-16 input",
        ));
    }

    let units = input.chunks_exact(2).map(|chunk| match endian {
        Endian::Big => u16::from_be_bytes([chunk[0], chunk[1]]),
        Endian::Little => u16::from_le_bytes([chunk[0], chunk[1]]),
    });
    let mut out = String::new();
    for unit in char::decode_utf16(units).filter(|unit| !matches!(unit, Ok('\u{feff}'))) {
        out.push(unit.map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid surrogate pair in UTF-16 input",
            )
        })?);
    }
    Ok(out)
}

fn encode_utf16(text: &str, endian: Endian, out: &mut Vec<u8>) {
    for unit in text.encode_utf16() {
        match endian {
            Endian::Big => out.extend_from_slice(&unit.to_be_bytes()),
            Endian::Little => out.extend_from_slice(&unit.to_le_bytes()),
        }
    }
}

fn decode_utf32_auto(input: &[u8]) -> io::Result<String> {
    match input {
        [0x00, 0x00, 0xfe, 0xff, rest @ ..] => decode_utf32(rest, Endian::Big),
        [0xff, 0xfe, 0x00, 0x00, rest @ ..] => decode_utf32(rest, Endian::Little),
        _ => decode_utf32(input, Endian::Big),
    }
}

fn decode_utf32(input: &[u8], endian: Endian) -> io::Result<String> {
    if input.len() % 4 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid byte length for UTF-32 input",
        ));
    }

    let mut out = String::new();
    for chunk in input.chunks_exact(4) {
        let value = match endian {
            Endian::Big => u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            Endian::Little => u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        };
        if value == 0xfeff {
            continue;
        }
        let Some(ch) = char::from_u32(value) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid code point in UTF-32 input",
            ));
        };
        out.push(ch);
    }
    Ok(out)
}

fn encode_utf32(text: &str, endian: Endian, out: &mut Vec<u8>) {
    for ch in text.chars() {
        let value = ch as u32;
        match endian {
            Endian::Big => out.extend_from_slice(&value.to_be_bytes()),
            Endian::Little => out.extend_from_slice(&value.to_le_bytes()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EolKind {
    Cr,
    Lf,
    Crlf,
    Mixed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuessResult {
    encoding: String,
    endian: Option<&'static str>,
    bom: bool,
    eol: Option<EolKind>,
}

fn guess_report(input: &[u8], level: u8) -> String {
    let guessed = guess_input(input);
    if level == 1 {
        return guessed.encoding;
    }

    let mut out = guessed.encoding;
    if let Some(endian) = guessed.endian {
        out.push(' ');
        out.push_str(endian);
    }
    if guessed.bom {
        out.push_str(" (BOM)");
    }
    if let Some(eol) = guessed.eol {
        out.push_str(match eol {
            EolKind::Cr => " (CR)",
            EolKind::Lf => " (LF)",
            EolKind::Crlf => " (CRLF)",
            EolKind::Mixed => " (MIXED NL)",
        });
    }
    out
}

fn guess_encoding(input: &[u8]) -> &'static str {
    match guess_input(input).encoding.as_str() {
        "CP932" => "CP932",
        "EUC-JIS-2004" => "EUC-JIS-2004",
        "EUCJP-MS" | "CP51932" => "EUC-JP",
        "CP50221" | "CP50220" => "ISO-2022-JP",
        "ISO-2022-JP-3" | "ISO-2022-JP-2004" => "ISO-2022-JP-3",
        "UTF-16" => "UTF-16",
        "UTF-32" => "UTF-32",
        "Shift_JIS" => "SHIFT_JIS",
        "EUC-JP" => "EUC-JP",
        "ISO-2022-JP" => "ISO-2022-JP",
        "UTF-8" => "UTF-8",
        _ => "UTF-8",
    }
}

fn guess_input(input: &[u8]) -> GuessResult {
    let eol = guess_eol(input);
    if input.starts_with(&[0xef, 0xbb, 0xbf]) {
        return GuessResult {
            encoding: "UTF-8".to_string(),
            endian: None,
            bom: true,
            eol,
        };
    }
    if input.starts_with(&[0x00, 0x00, 0xfe, 0xff]) {
        return GuessResult {
            encoding: "UTF-32".to_string(),
            endian: Some("BE"),
            bom: true,
            eol,
        };
    }
    if input.starts_with(&[0xff, 0xfe, 0x00, 0x00]) {
        return GuessResult {
            encoding: "UTF-32".to_string(),
            endian: Some("LE"),
            bom: true,
            eol,
        };
    }
    if input.starts_with(&[0xfe, 0xff]) {
        return GuessResult {
            encoding: "UTF-16".to_string(),
            endian: Some("BE"),
            bom: true,
            eol,
        };
    }
    if input.starts_with(&[0xff, 0xfe]) {
        return GuessResult {
            encoding: "UTF-16".to_string(),
            endian: Some("LE"),
            bom: true,
            eol,
        };
    }
    if let Some(score) = guess_iso_2022_jp_score(input) {
        return GuessResult {
            encoding: refine_iso2022_name(score),
            endian: None,
            bom: false,
            eol,
        };
    }
    if let Some((encoding, endian)) = guess_utf_by_nuls(input) {
        return GuessResult {
            encoding: encoding.to_string(),
            endian,
            bom: false,
            eol,
        };
    }

    let mut candidates = GuessCandidates::new();
    for &b in input {
        candidates.code_status(b);
    }
    if let Some(kind) = candidates.established_kind {
        let encoding = match kind {
            CandidateKind::Euc => refine_euc_name(candidates.euc.score),
            CandidateKind::ShiftJis => refine_sjis_name(candidates.sjis.score),
            CandidateKind::Utf8 => "UTF-8".to_string(),
        };
        return GuessResult {
            encoding,
            endian: None,
            bom: false,
            eol,
        };
    }
    let mut best = [
        ("EUC-JP", candidates.euc.score),
        ("Shift_JIS", candidates.sjis.score),
        ("UTF-8", candidates.utf8.score),
    ];
    best.sort_by_key(|(_, score)| *score);

    let encoding = match best[0].0 {
        "EUC-JP" => refine_euc_name(candidates.euc.score),
        "Shift_JIS" => refine_sjis_name(candidates.sjis.score),
        "UTF-8" if candidates.utf8.score < SCORE_ERROR => "UTF-8".to_string(),
        _ if input.iter().all(|&b| b <= 0x7f) => "ASCII".to_string(),
        _ => "BINARY".to_string(),
    };

    GuessResult {
        encoding,
        endian: None,
        bom: false,
        eol,
    }
}

fn guess_eol(input: &[u8]) -> Option<EolKind> {
    let mut kind = None;
    let mut i = 0;
    while i < input.len() {
        let found = match input[i] {
            b'\r' if i + 1 < input.len() && input[i + 1] == b'\n' => {
                i += 2;
                Some(EolKind::Crlf)
            }
            b'\r' => {
                i += 1;
                Some(EolKind::Cr)
            }
            b'\n' => {
                i += 1;
                Some(EolKind::Lf)
            }
            _ => {
                i += 1;
                None
            }
        };
        if let Some(found) = found {
            match kind {
                None => kind = Some(found),
                Some(current) if current == found => {}
                Some(_) => return Some(EolKind::Mixed),
            }
        }
    }
    kind
}

fn guess_iso_2022_jp_score(input: &[u8]) -> Option<u32> {
    let mut i = 0;
    let mut score = SCORE_INIT;
    while i < input.len() {
        if input[i] != 0x1b {
            i += 1;
            continue;
        }
        match input.get(i + 1).copied() {
            Some(b'$') => match input.get(i + 2).copied() {
                Some(b'@' | b'B') => return Some(score),
                Some(b'(') => match input.get(i + 3).copied() {
                    Some(b'D') => {
                        score |= SCORE_X0212;
                        return Some(score);
                    }
                    Some(b'O' | b'P' | b'Q') => {
                        score |= SCORE_X0213;
                        return Some(score);
                    }
                    _ => {}
                },
                _ => {}
            },
            Some(b'(') => match input.get(i + 2).copied() {
                Some(b'I') => {
                    score |= SCORE_KANA;
                    return Some(score);
                }
                Some(b'B' | b'J' | b'H') => return Some(score),
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    None
}

fn guess_utf_by_nuls(input: &[u8]) -> Option<(&'static str, Option<&'static str>)> {
    let sample = &input[..input.len().min(256)];
    if sample.len() < 4 {
        return None;
    }
    let even_nuls = sample.iter().step_by(2).filter(|&&b| b == 0).count();
    let odd_nuls = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|&&b| b == 0)
        .count();
    if odd_nuls > sample.len() / 4 && even_nuls == 0 {
        return Some(("UTF-16", Some("LE")));
    }
    if even_nuls > sample.len() / 4 && odd_nuls == 0 {
        return Some(("UTF-16", Some("BE")));
    }
    let quads: Vec<&[u8]> = sample.chunks_exact(4).take(16).collect();
    if !quads.is_empty() {
        let be = quads
            .iter()
            .filter(|quad| quad[0] == 0 && quad[1] == 0 && quad[2] == 0)
            .count();
        let le = quads
            .iter()
            .filter(|quad| quad[1] == 0 && quad[2] == 0 && quad[3] == 0)
            .count();
        if be > quads.len() / 2 {
            return Some(("UTF-32", Some("BE")));
        }
        if le > quads.len() / 2 {
            return Some(("UTF-32", Some("LE")));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateKind {
    Euc,
    ShiftJis,
    Utf8,
}

#[derive(Clone, Debug)]
struct GuessCandidate {
    kind: CandidateKind,
    stat: i32,
    score: u32,
    index: usize,
    buf: [u8; 4],
}

impl GuessCandidate {
    fn new(kind: CandidateKind) -> Self {
        Self {
            kind,
            stat: 0,
            score: SCORE_INIT,
            index: 0,
            buf: [0; 4],
        }
    }

    fn reset(&mut self) {
        self.stat = 0;
        self.score = SCORE_INIT;
        self.index = 0;
    }

    fn clear(&mut self) {
        self.stat = 0;
        self.index = 0;
    }

    fn push(&mut self, b: u8) {
        if self.index < self.buf.len() {
            self.buf[self.index] = b;
            self.index += 1;
        }
    }

    fn disable(&mut self) {
        self.stat = -1;
        self.buf[0] = 0xff;
        self.code_score();
    }

    fn status(&mut self, b: u8, established: bool) {
        if self.stat == -1 {
            if b <= 0x7f && established {
                self.reset();
            }
            return;
        }

        match self.kind {
            CandidateKind::Euc => self.euc_status(b),
            CandidateKind::ShiftJis => self.sjis_status(b),
            CandidateKind::Utf8 => self.utf8_status(b),
        }
    }

    fn euc_status(&mut self, b: u8) {
        match self.stat {
            0 if b <= 0x7f => {}
            0 if b == 0x8f => {
                self.stat = 2;
                self.push(b);
            }
            0 if b == 0x8e || (0xa1..=0xfe).contains(&b) => {
                self.stat = 1;
                self.push(b);
            }
            0 => self.disable(),
            1 if (0xa1..=0xfe).contains(&b) => {
                self.push(b);
                self.code_score();
                self.clear();
            }
            1 => self.disable(),
            2 if (0xa1..=0xfe).contains(&b) => {
                self.stat = 1;
                self.push(b);
            }
            2 => self.disable(),
            _ => {}
        }
    }

    fn sjis_status(&mut self, b: u8) {
        match self.stat {
            0 if b <= 0x7f => {}
            0 if (0xa1..=0xdf).contains(&b) => {
                self.buf[0] = 0x8e;
                self.buf[1] = b;
                self.index = 2;
                self.code_score();
                self.clear();
            }
            0 if (0x81..0xa0).contains(&b)
                || (0xe0..=0xea).contains(&b)
                || (0xf0..=0xfc).contains(&b) =>
            {
                self.stat = 1;
                self.push(b);
            }
            0 if (0xed..=0xee).contains(&b) => {
                self.stat = 3;
                self.push(b);
            }
            0 if (0xfa..=0xfc).contains(&b) => {
                self.stat = 2;
                self.push(b);
            }
            0 => self.disable(),
            1 if is_sjis_trail(b) => {
                self.push(b);
                let (c2, c1) = sjis_to_euc_pair(self.buf[0], self.buf[1]);
                self.buf[0] = c2;
                self.buf[1] = c1;
                self.code_score();
                self.clear();
            }
            1 => self.disable(),
            2 if is_sjis_trail(b) => {
                self.push(b);
                self.score |= SCORE_CP932;
                self.clear();
            }
            2 => self.disable(),
            3 if is_sjis_trail(b) => {
                self.push(b);
                let (c2, c1) = sjis_to_euc_pair(self.buf[0], self.buf[1]);
                self.buf[0] = c2;
                self.buf[1] = c1;
                self.score |= SCORE_CP932;
                self.clear();
            }
            3 => self.disable(),
            _ => {}
        }
    }

    fn utf8_status(&mut self, b: u8) {
        match self.stat {
            0 if b <= 0x7f => {}
            0 if (0xc2..=0xdf).contains(&b) => {
                self.stat = 1;
                self.push(b);
            }
            0 if (0xe0..=0xef).contains(&b) => {
                self.stat = 2;
                self.push(b);
            }
            0 if (0xf0..=0xf4).contains(&b) => {
                self.stat = 3;
                self.push(b);
            }
            0 => self.disable(),
            1 | 2 if (0x80..=0xbf).contains(&b) => {
                self.push(b);
                if self.index > self.stat as usize {
                    if !(self.buf[0] == 0xef && self.buf[1] == 0xbb && self.buf[2] == 0xbf) {
                        self.score |= utf8_score(&self.buf[..self.index]);
                    }
                    self.clear();
                }
            }
            1 | 2 => self.disable(),
            3 if (0x80..=0xbf).contains(&b) => {
                self.push(b);
                if self.index > self.stat as usize {
                    self.clear();
                }
            }
            3 => self.disable(),
            _ => {}
        }
    }

    fn code_score(&mut self) {
        let c2 = self.buf[0];
        let c1 = self.buf[1];
        if c2 == 0xff {
            self.score |= SCORE_ERROR;
        } else if c2 == 0x8e {
            self.score |= SCORE_KANA;
        } else if c2 == 0x8f {
            self.score |= match c1 & 0x70 {
                0x20 => SCORE_TABLE_8FA0[(c1 & 0x0f) as usize],
                0x60 => SCORE_TABLE_8FE0[(c1 & 0x0f) as usize],
                0x70 => SCORE_TABLE_8FF0[(c1 & 0x0f) as usize],
                _ => SCORE_X0212,
            };
        } else if (c2 & 0x70) == 0x20 {
            self.score |= SCORE_TABLE_A0[(c2 & 0x0f) as usize];
        } else if (c2 & 0x70) == 0x70 {
            self.score |= SCORE_TABLE_F0[(c2 & 0x0f) as usize];
        } else if (c2 & 0x70) >= 0x50 {
            self.score |= SCORE_L2;
        }
    }
}

struct GuessCandidates {
    euc: GuessCandidate,
    sjis: GuessCandidate,
    utf8: GuessCandidate,
    established: bool,
    established_kind: Option<CandidateKind>,
}

impl GuessCandidates {
    fn new() -> Self {
        Self {
            euc: GuessCandidate::new(CandidateKind::Euc),
            sjis: GuessCandidate::new(CandidateKind::ShiftJis),
            utf8: GuessCandidate::new(CandidateKind::Utf8),
            established: false,
            established_kind: None,
        }
    }

    fn code_status(&mut self, b: u8) {
        self.euc.status(b, self.established);
        self.sjis.status(b, self.established);
        self.utf8.status(b, self.established);
        let active = [self.euc.stat, self.sjis.stat, self.utf8.stat]
            .into_iter()
            .filter(|&stat| stat >= 0)
            .count();
        self.established = active == 1;
        if self.established && self.established_kind.is_none() {
            self.established_kind = if self.euc.stat >= 0 {
                Some(CandidateKind::Euc)
            } else if self.sjis.stat >= 0 {
                Some(CandidateKind::ShiftJis)
            } else if self.utf8.stat >= 0 {
                Some(CandidateKind::Utf8)
            } else {
                None
            };
        }
        if b <= 0x7f && !self.established {
            self.euc.reset();
            self.sjis.reset();
            self.utf8.reset();
        }
    }
}

const SCORE_TABLE_A0: [u32; 16] = [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_X0213,
];
const SCORE_TABLE_F0: [u32; 16] = [
    SCORE_L2,
    SCORE_L2,
    SCORE_L2,
    SCORE_L2,
    SCORE_L2,
    SCORE_DEPEND,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_DEPEND,
    SCORE_DEPEND,
    SCORE_CP932,
    SCORE_CP932,
    SCORE_CP932,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_ERROR,
];
const SCORE_TABLE_8FA0: [u32; 16] = [
    0,
    SCORE_X0213,
    SCORE_X0212,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0213,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
];
const SCORE_TABLE_8FE0: [u32; 16] = [
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0213,
    SCORE_X0213,
];
const SCORE_TABLE_8FF0: [u32; 16] = [
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0212,
    SCORE_X0212,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
    SCORE_X0213,
];

fn sjis_to_euc_pair(c2: u8, c1: u8) -> (u8, u8) {
    let mut c2 = c2;
    let mut c1 = c1;
    if c2 >= 0x80 {
        c2 = c2
            .wrapping_add(c2)
            .wrapping_sub(if c2 <= 0x9f { 0xe1 } else { 0x61 });
        if c1 < 0x9f {
            c1 = c1.wrapping_sub(if c1 > 0x7f { 0x20 } else { 0x1f });
        } else {
            c1 = c1.wrapping_sub(0x7e);
            c2 = c2.wrapping_add(1);
        }
    }
    (c2 | 0x80, c1 | 0x80)
}

fn utf8_score(bytes: &[u8]) -> u32 {
    match std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.chars().next())
        .map(|ch| ch as u32)
    {
        Some(0xff61..=0xff9f) => SCORE_KANA,
        Some(0xe000..=0xf8ff) => SCORE_CP932,
        Some(0x3400..=0x9fff) => SCORE_L2,
        Some(_) => 0,
        None => SCORE_ERROR,
    }
}

fn refine_sjis_name(score: u32) -> String {
    if score & (SCORE_DEPEND | SCORE_CP932) != 0 {
        "CP932".to_string()
    } else {
        "Shift_JIS".to_string()
    }
}

fn refine_euc_name(score: u32) -> String {
    if score & SCORE_X0213 != 0 {
        "EUC-JIS-2004".to_string()
    } else if score & SCORE_X0212 != 0 {
        "EUCJP-MS".to_string()
    } else if score & (SCORE_DEPEND | SCORE_CP932) != 0 {
        "CP51932".to_string()
    } else {
        "EUC-JP".to_string()
    }
}

fn refine_iso2022_name(score: u32) -> String {
    if score & SCORE_X0213 != 0 {
        "ISO-2022-JP-3".to_string()
    } else if score & SCORE_X0212 != 0 {
        "ISO-2022-JP-1".to_string()
    } else if score & SCORE_KANA != 0 {
        "CP50221".to_string()
    } else if score & (SCORE_DEPEND | SCORE_CP932) != 0 {
        "CP50220".to_string()
    } else {
        "ISO-2022-JP".to_string()
    }
}

fn is_sjis_trail(b: u8) -> bool {
    (0x40..=0x7e).contains(&b) || (0x80..=0xfc).contains(&b)
}

fn apply_line_endings(mut bytes: Vec<u8>, mode: LineEnding) -> Vec<u8> {
    if mode == LineEnding::Preserve {
        return bytes;
    }

    let mut normalized = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if i + 1 < bytes.len() && bytes[i + 1] == b'\n' => {
                normalized.push(b'\n');
                i += 2;
            }
            b'\r' => {
                normalized.push(b'\n');
                i += 1;
            }
            b => {
                normalized.push(b);
                i += 1;
            }
        }
    }

    bytes = match mode {
        LineEnding::Preserve | LineEnding::Unix => normalized,
        LineEnding::Windows => {
            let mut out = Vec::with_capacity(normalized.len());
            for b in normalized {
                if b == b'\n' {
                    out.extend_from_slice(b"\r\n");
                } else {
                    out.push(b);
                }
            }
            out
        }
        LineEnding::Mac => normalized
            .into_iter()
            .map(|b| if b == b'\n' { b'\r' } else { b })
            .collect(),
    };
    bytes
}

fn normalize_encoding(name: &str) -> String {
    let upper = name.trim().replace('_', "-").to_ascii_uppercase();
    match upper.as_str() {
        "SJIS" | "SHIFT-JIS" => "SHIFT_JIS".to_string(),
        "SHIFT-JISX0213" | "SHIFT-JIS-X0213" => "SHIFT_JISX0213".to_string(),
        "SHIFT-JIS-2004" => "SHIFT_JIS-2004".to_string(),
        "WINDOWS-31J" | "CSWINDOWS31J" | "MS932" | "CP932" => "CP932".to_string(),
        "EUCJP" | "EUC-JP" | "EUCJP-NKF" => "EUC-JP".to_string(),
        "CP51932" => "CP51932".to_string(),
        "EUCJP-MS" | "EUC-JP-MS" | "EUCJPMS" => "EUCJP-MS".to_string(),
        "EUCJP-ASCII" | "EUC-JP-ASCII" => "EUCJP-ASCII".to_string(),
        "EUC-JISX0213" | "EUC-JIS-2004" => upper,
        "JIS" | "ISO2022JP" | "ISO-2022-JP" => "ISO-2022-JP".to_string(),
        "ISO2022JP-1" | "ISO-2022-JP-1" => "ISO-2022-JP-1".to_string(),
        "ISO2022JP-3" | "ISO-2022-JP-3" => "ISO-2022-JP-3".to_string(),
        "ISO2022JP-2004" | "ISO-2022-JP-2004" => "ISO-2022-JP-2004".to_string(),
        "ISO2022JP-CP932" | "ISO-2022-JP-CP932" | "CP50220" => "CP50220".to_string(),
        "CSISO2022JP" | "CP50221" => "CP50221".to_string(),
        "CP50222" => "CP50222".to_string(),
        "CP10001" => "CP10001".to_string(),
        "LATIN1" | "LATIN-1" | "ISO8859-1" | "ISO-8859-1" => "ISO-8859-1".to_string(),
        "UTF8" | "UTF-8N" => "UTF-8".to_string(),
        "UTF8-BOM" | "UTF-8-BOM" => "UTF-8-BOM".to_string(),
        "UTF16" => "UTF-16".to_string(),
        "UTF16BE" => "UTF-16BE".to_string(),
        "UTF16BE-BOM" | "UTF-16BE-BOM" => "UTF-16BE-BOM".to_string(),
        "UTF16LE" => "UTF-16LE".to_string(),
        "UTF16LE-BOM" | "UTF-16LE-BOM" => "UTF-16LE-BOM".to_string(),
        "UTF32" => "UTF-32".to_string(),
        "UTF32BE" => "UTF-32BE".to_string(),
        "UTF32BE-BOM" | "UTF-32BE-BOM" => "UTF-32BE-BOM".to_string(),
        "UTF32LE" => "UTF-32LE".to_string(),
        "UTF32LE-BOM" | "UTF-32LE-BOM" => "UTF-32LE-BOM".to_string(),
        other => other.to_string(),
    }
}

fn same_encoding(left: &str, right: &str) -> bool {
    normalize_encoding(left) == normalize_encoding(right)
}

fn print_configuration() {
    print!("{}", configuration_report());
}

fn configuration_report() -> String {
    let mut out = String::new();
    out.push_str(&format!("{NKF_VERSION}\n"));
    out.push_str("Configuration:\n");
    out.push_str("  implementation: pure Rust\n");
    out.push_str("  external iconv: disabled\n");
    out.push_str("  native C linkage: disabled\n");
    out.push_str("  Unicode normalization: enabled\n");
    out.push_str("  MIME decode/encode: enabled\n");
    out.push_str("  JIS X 0213 tables: enabled\n");
    out.push_str("  X0212 tables: enabled\n");
    out.push_str("  CP932 tables: enabled\n");
    out.push_str("  exec-in/exec-out: enabled via direct process execution\n");
    out.push_str("  buffering options: -b buffered, -u flush-on-complete\n");
    out.push_str("  encodings: UTF-8, UTF-16, UTF-32, ISO-2022-JP, EUC-JP, Shift_JIS, CP932, CP51932, EUCJP-MS, EUCJP-ASCII, CP50220/21/22, CP10001\n");
    out
}

fn debug_report(config: &Config, guessed_input: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("nkf-rust debug:\n");
    out.push_str(&format!(
        "  input: {}\n",
        config.input.as_deref().unwrap_or("(guess)")
    ));
    if let Some(guessed) = guessed_input {
        out.push_str(&format!("  guessed-input: {guessed}\n"));
    }
    out.push_str(&format!("  output: {}\n", config.output));
    out.push_str(&format!("  line-ending: {:?}\n", config.line_ending));
    out.push_str(&format!("  buffering: {:?}\n", config.buffering));
    out.push_str(&format!("  mime-decode: {:?}\n", config.mime_decode_mode));
    out.push_str(&format!("  mime-encode: {:?}\n", config.mime_encode_mode));
    out.push_str(&format!("  cp932-compat: {}\n", config.cp932_compat));
    out.push_str(&format!("  cp932inv: {}\n", config.cp932inv));
    out.push_str(&format!("  no-cp932ext: {}\n", config.no_cp932ext));
    out.push_str(&format!(
        "  no-best-fit-chars: {}\n",
        config.no_best_fit_chars
    ));
    out.push_str(&format!("  ms-ucs-map: {:?}\n", config.ms_ucs_map));
    if let Some(exec_mode) = config.exec_mode {
        out.push_str(&format!("  exec-mode: {:?}\n", exec_mode));
        out.push_str(&format!("  exec-command: {:?}\n", config.exec_command));
    }
    out
}

fn print_help() {
    let program = env::args().next().unwrap_or_else(|| "nkf-rust".to_string());
    println!(
        "{program} {NKF_VERSION}
Usage: {program} [options] [file ...]

Options:
  -j, -e, -s, -w       output ISO-2022-JP, EUC-JP, Shift_JIS, UTF-8
  -J, -E, -S, -W       input ISO-2022-JP, EUC-JP, Shift_JIS, UTF-8
  -V                  print build/configuration details
  -b, -u              buffered output, or flush output on completion
  --ic=ENC --oc=ENC    set input/output encoding
  -g                  print guessed input encoding
  -t                  transparent copy
  -mB                 decode Base64 input
  -MB                 encode output as Base64
  -m                  decode MIME encoded-words
  -M, -MQ             encode output as MIME encoded-word
  -Z[0-4]             convert fullwidth ASCII/symbols/space/kana
  -X, -x              convert halfwidth kana to fullwidth, or preserve it
  -h[1-3]             convert hiragana/katakana
  -r                  apply ROT13 to ASCII letters
  -B[0-2]             repair broken JIS escape sequences
  -i[@B], -o[JBH]      set ISO-2022-JP kanji/ascii escape introducers
  --cap-input          decode :XX hex input
  --url-input          decode %XX hex input
  --numchar-input      decode numeric character references
  --prefix=XY          prefix X before each Y byte
  --no-output          suppress normal output
  --debug              print conversion settings to stderr
  --exec-in CMD ...    read input from CMD stdout and convert it
  --exec-out CMD ...   convert input and write it to CMD stdin
  --fb-*              encode fallback: skip/html/xml/java/perl/subchar
  -f[LEN[-MARGIN]]     fold lines, default 60-10
  -F[LEN[-MARGIN]]     fold lines preserving input newlines
  --utf8mac-input      normalize UTF-8-MAC-style input to NFC
  -Lu -Lw -Lm -d -c    convert line endings
  -O FILE             write output to FILE
  --overwrite[=SUF]    overwrite files, preserving timestamps
  --in-place[=SUF]     overwrite files without preserving timestamps
  --help --version"
    );
}
