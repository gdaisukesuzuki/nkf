use nkf::{
    convert_bytes, guess_report_text, normalize_encoding_name, process_with_config, Config,
    EncodeFallback, LineEnding, X0201Mode,
};

#[test]
fn normalizes_names_and_converts_supported_encodings() {
    assert_eq!(normalize_encoding_name("sjis"), "SHIFT_JIS");
    assert_eq!(normalize_encoding_name("Windows-31J"), "CP932");
    assert_eq!(normalize_encoding_name("MS932"), "CP932");
    assert_eq!(normalize_encoding_name("CP51932"), "CP51932");
    assert_eq!(normalize_encoding_name("eucJP-MS"), "EUCJP-MS");
    assert_eq!(normalize_encoding_name("eucJP-ASCII"), "EUCJP-ASCII");
    assert_eq!(normalize_encoding_name("CP50220"), "CP50220");
    assert_eq!(normalize_encoding_name("CP50221"), "CP50221");
    assert_eq!(normalize_encoding_name("CP50222"), "CP50222");
    assert_eq!(normalize_encoding_name("CP10001"), "CP10001");
    assert_eq!(normalize_encoding_name("UTF-8-BOM"), "UTF-8-BOM");
    assert_eq!(normalize_encoding_name("ISO2022JP"), "ISO-2022-JP");

    let sjis = convert_bytes("UTF-8", "Shift_JIS", "日本語".as_bytes()).unwrap();
    let utf8 = convert_bytes("Shift_JIS", "UTF-8", &sjis).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "日本語");

    let euc = convert_bytes("UTF-8", "EUC-JIS-2004", "俱".as_bytes()).unwrap();
    assert_eq!(euc, vec![0xae, 0xa1]);
    let utf8 = convert_bytes("EUC-JIS-2004", "UTF-8", &euc).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "俱");
}

#[test]
fn supports_cp932_compatibility_aliases_and_guards() {
    let cp932 = convert_bytes("UTF-8", "Windows-31J", "日本語".as_bytes()).unwrap();
    let utf8 = convert_bytes("CP932", "UTF-8", &cp932).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "日本語");

    let cp51932 = convert_bytes("UTF-8", "CP51932", "日本語".as_bytes()).unwrap();
    let utf8 = convert_bytes("CP51932", "UTF-8", &cp51932).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "日本語");

    let cp50221 = convert_bytes("UTF-8", "CP50221", "日本語".as_bytes()).unwrap();
    let utf8 = convert_bytes("CP50221", "UTF-8", &cp50221).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "日本語");

    let cp10001 = convert_bytes("UTF-8", "CP10001", "日本語".as_bytes()).unwrap();
    let utf8 = convert_bytes("CP10001", "UTF-8", &cp10001).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "日本語");

    let mut config = Config::default();
    config.input = Some("UTF-8".to_string());
    config.output = "CP932".to_string();
    assert!(process_with_config(&config, "①".as_bytes()).is_ok());

    config.no_cp932ext = true;
    assert!(process_with_config(&config, "①".as_bytes()).is_err());

    config = Config::default();
    config.input = Some("UTF-8".to_string());
    config.output = "CP932".to_string();
    config.no_best_fit_chars = true;
    assert!(process_with_config(&config, "¥".as_bytes()).is_err());
}

#[test]
fn uses_imported_nkf_utf8_euc_tables() {
    assert_eq!(
        String::from_utf8(convert_bytes("EUC-JP", "UTF-8", &[0x8e, 0xa6]).unwrap()).unwrap(),
        "ｦ"
    );
    assert_eq!(
        String::from_utf8(convert_bytes("EUC-JP", "UTF-8", &[0xa4, 0xa2]).unwrap()).unwrap(),
        "あ"
    );
    assert_eq!(
        convert_bytes("UTF-8", "EUC-JP", "¥".as_bytes()).unwrap(),
        vec![0xa1, 0xef]
    );
    assert_eq!(
        convert_bytes("UTF-8", "EUC-JP", "あ".as_bytes()).unwrap(),
        vec![0xa4, 0xa2]
    );
    assert_eq!(
        String::from_utf8(convert_bytes("EUC-JP", "UTF-8", &[0xa1, 0xb1]).unwrap()).unwrap(),
        "‾"
    );
    assert_eq!(
        String::from_utf8(convert_bytes("EUCJP-MS", "UTF-8", &[0xa1, 0xb1]).unwrap()).unwrap(),
        "￣"
    );
    assert_eq!(
        convert_bytes("UTF-8", "EUCJP-MS", "￣".as_bytes()).unwrap(),
        vec![0xa1, 0xb1]
    );
    assert_eq!(
        String::from_utf8(convert_bytes("EUC-JISX0213", "UTF-8", &[0xa2, 0xaf]).unwrap()).unwrap(),
        "＇"
    );
}

#[test]
fn emits_utf_bom_variants() {
    assert_eq!(
        convert_bytes("UTF-8", "UTF-8-BOM", b"A").unwrap(),
        vec![0xef, 0xbb, 0xbf, b'A']
    );
    assert_eq!(
        convert_bytes("UTF-8", "UTF-16LE-BOM", b"A").unwrap(),
        vec![0xff, 0xfe, 0x41, 0x00]
    );
    assert_eq!(
        convert_bytes("UTF-8", "UTF-32BE-BOM", b"A").unwrap(),
        vec![0x00, 0x00, 0xfe, 0xff, 0x00, 0x00, 0x00, 0x41]
    );
}

#[test]
fn guesses_common_and_jis_x0213_inputs() {
    assert_eq!(guess_report_text("日本語\n".as_bytes(), 2), "UTF-8 (LF)");
    let euc = convert_bytes("UTF-8", "EUC-JP", "日本語".as_bytes()).unwrap();
    assert_eq!(guess_report_text(&euc, 1), "EUC-JP");
    assert_eq!(guess_report_text(b"\x1b$(Q!!\x1b(B", 1), "ISO-2022-JP-3");
}

#[test]
fn applies_text_filters_and_input_preprocessors() {
    let mut config = Config::default();
    config.cap_input = true;
    config.url_input = true;
    config.numchar_input = true;
    config.input = Some("UTF-8".to_string());
    config.kana_flags = 3;
    config.rot = true;
    config.prefix_rules.push((b'N', b'!'));

    let output = process_with_config(&config, b":E3:81:8B%E3%83%8A&#x41;").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "カな!N");
}

#[test]
fn converts_width_kana_line_endings_and_fallbacks() {
    let mut config = Config::default();
    config.z_flags = 1 | 16;
    let output = process_with_config(&config, "カガパヴー。、「」".as_bytes()).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "ｶｶﾞﾊﾟｳﾞｰ｡､｢｣");

    config = Config::default();
    config.x0201_mode = X0201Mode::Fullwidth;
    let output = process_with_config(&config, "｡｢｣､･ｶﾞﾊﾟｳﾞ".as_bytes()).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "。「」、・ガパヴ");

    config = Config::default();
    config.line_ending = LineEnding::Unix;
    let output = process_with_config(&config, b"a\r\nb\rc").unwrap();
    assert_eq!(output, b"a\nb\nc");

    config = Config::default();
    config.input = Some("UTF-8".to_string());
    config.output = "Shift_JIS".to_string();
    config.encode_fallback = EncodeFallback::Html;
    let output = process_with_config(&config, "A😀B".as_bytes()).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "A&#128512;B");
}

#[test]
fn handles_iso2022jp_repairs_intro_overrides_and_strict_geta() {
    let mut config = Config::default();
    config.input = Some("ISO-2022-JP".to_string());
    config.broken_jis_flags = 1;
    let output = process_with_config(&config, b"$B$3$s$K$A$O(B").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "こんにちは");

    config = Config::default();
    config.input = Some("UTF-8".to_string());
    config.output = "ISO-2022-JP".to_string();
    config.iso2022_kanji_intro = Some(b'@');
    config.iso2022_ascii_intro = Some(b'J');
    let output = process_with_config(&config, "日本語ABC".as_bytes()).unwrap();
    assert!(output.windows(3).any(|window| window == b"\x1b$@"));
    assert!(output.windows(3).any(|window| window == b"\x1b(J"));

    config.iso2022_kanji_intro = None;
    config.iso2022_ascii_intro = None;
    config.iso2022jp_strict = true;
    let output = process_with_config(&config, "A😀B".as_bytes()).unwrap();
    let utf8 = convert_bytes("ISO-2022-JP", "UTF-8", &output).unwrap();
    assert_eq!(String::from_utf8(utf8).unwrap(), "A〓B");
}
