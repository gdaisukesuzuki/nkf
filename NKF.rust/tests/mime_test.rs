use nkf::{process_with_config, Config, MimeDecodeMode, MimeEncodeMode};

#[test]
fn decodes_base64_and_quoted_printable_input_modes() {
    let mut config = Config::default();
    config.mime_decode_mode = MimeDecodeMode::Base64;
    let output = process_with_config(&config, b"5pel5pys6KqeCg==").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "日本語\n");

    config.mime_decode_mode = MimeDecodeMode::QuotedPrintable;
    let output = process_with_config(&config, b"Hello=20World=\n!").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "Hello World!");
}

#[test]
fn decodes_mime_encoded_words_with_strict_whitespace_rules() {
    let mut config = Config::default();
    config.input = Some("UTF-8".to_string());
    config.mime_decode_mode = MimeDecodeMode::Strict;

    let output = process_with_config(&config, b"Subject: =?UTF-8?Q?hello=5Fworld?=").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "Subject: hello_world");

    let output =
        process_with_config(&config, b"=?UTF-8?B?5pel?= \r\n\t=?UTF-8?B?5pys?= word").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "日本 word");

    let output = process_with_config(&config, b"=?UTF-8?B?5pel\r\n 5pys?=").unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "日本");
}

#[test]
fn encodes_mime_words_and_folds_long_output() {
    let mut config = Config::default();
    config.mime_encode_mode = MimeEncodeMode::Base64;
    let output = process_with_config(&config, "日本語".as_bytes()).unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "=?UTF-8?B?5pel5pys6Kqe?="
    );

    config.mime_encode_mode = MimeEncodeMode::QuotedPrintable;
    let output = process_with_config(&config, b"hello world").unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "=?UTF-8?Q?hello_world?="
    );

    config.input = Some("UTF-8".to_string());
    config.output = "ISO-2022-JP".to_string();
    config.mime_encode_mode = MimeEncodeMode::Base64;
    let output = process_with_config(&config, "日本語".as_bytes()).unwrap();
    assert!(String::from_utf8(output)
        .unwrap()
        .starts_with("=?ISO-2022-JP?B?"));

    config.output = "UTF-8".to_string();
    let output = process_with_config(&config, "日本語".repeat(20).as_bytes()).unwrap();
    let encoded = String::from_utf8(output).unwrap();
    assert!(encoded.contains("\n "));
    assert!(encoded.lines().all(|line| line.len() <= 76));
}
