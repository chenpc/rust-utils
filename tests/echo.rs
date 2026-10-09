use coreutils::testutil::*;

#[test]
fn default_prints_string_and_newline() {
    let r = run("echo", &["hello"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "hello\n");
}

#[test]
fn multiple_strings_joined_by_space() {
    let r = run("echo", &["a", "b", "c"], "");
    assert_eq!(r.out(), "a b c\n");
}

#[test]
fn no_args_prints_blank_line() {
    let r = run("echo", &[], "");
    assert_eq!(r.out(), "\n");
}

#[test]
fn n_suppresses_trailing_newline() {
    let r = run("echo", &["-n", "hi"], "");
    assert_eq!(r.out(), "hi");
}

#[test]
fn combined_flags_are_parsed() {
    let r = run("echo", &["-ne", "a\\nb"], "");
    assert_eq!(r.out(), "a\nb");
}

#[test]
fn e_is_default_so_backslash_is_literal() {
    let r = run("echo", &["a\\nb"], "");
    assert_eq!(r.out(), "a\\nb\n");
}

#[test]
fn e_flag_after_e_disables_escapes() {
    let r = run("echo", &["-e", "-E", "a\\nb"], "");
    assert_eq!(r.out(), "a\\nb\n");
}

#[test]
fn named_escapes_are_expanded() {
    for (esc, want) in [
        ("\\\\", "\\"),
        ("\\a", "\x07"),
        ("\\b", "\x08"),
        ("\\e", "\x1B"),
        ("\\f", "\x0C"),
        ("\\n", "\n"),
        ("\\r", "\r"),
        ("\\t", "\t"),
        ("\\v", "\x0B"),
    ] {
        let r = run("echo", &["-e", esc], "");
        assert_eq!(r.out(), format!("{want}\n"), "escape {esc}");
    }
}

#[test]
fn octal_escape_accepts_1_to_3_digits() {
    assert_eq!(run("echo", &["-e", "\\0"], "").out(), "\x00\n");
    assert_eq!(run("echo", &["-e", "\\7"], "").out(), "\x07\n");
    assert_eq!(run("echo", &["-e", "\\41"], "").out(), "!\n"); // 0o41 = 33 = '!'
    assert_eq!(run("echo", &["-e", "\\101"], "").out(), "A\n");
    // 3 digits 上限：\1013 -> \101 = A，剩下的 '3' 是字面值
    assert_eq!(run("echo", &["-e", "\\1013"], "").out(), "A3\n");
}

#[test]
fn hex_escape_accepts_1_to_2_digits() {
    assert_eq!(run("echo", &["-e", "\\x41"], "").out(), "A\n");
    assert_eq!(run("echo", &["-e", "\\x4"], "").out(), "\x04\n");
    assert_eq!(run("echo", &["-e", "\\x414"], "").out(), "A4\n");
    // man page 沒定義 \x 後面无 hex digit 的行為：保留 'x'（假設）
    assert_eq!(run("echo", &["-e", "\\xZ"], "").out(), "xZ\n");
}

#[test]
fn c_stops_all_further_output() {
    let r = run("echo", &["-e", "a\\cb"], "");
    assert_eq!(r.out(), "a");
    let r2 = run("echo", &["-e", "x", "y\\cz", "w"], "");
    assert_eq!(r2.out(), "x y");
}

#[test]
fn unknown_flag_is_treated_as_string() {
    // man page 只列 -n -e -E -h -V；未知選項視為 STRING（docs/assumptions.md）
    let r = run("echo", &["-x", "hi"], "");
    assert_eq!(r.out(), "-x hi\n");
}

#[test]
fn help_and_version() {
    let r = run("echo", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("echo"), "help must name the applet: {}", r.out());
    let v = run("echo", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("echo"), "version must name the applet: {}", v.out());
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin("echo", &["-n", "hi"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "hi");
}

#[test]
fn fuzz_default_output_is_join_plus_newline() {
    // man page 可推導的不變量：-E（預設）時輸出 = args 用空白 join + newline
    let mut rng = Rng::new(0xEC0D);
    for _ in 0..200 {
        let n = rng.below(6) as usize;
        let mut args: Vec<String> = Vec::new();
        let mut parts: Vec<String> = Vec::new();
        for _ in 0..n {
            let len = 1 + rng.below(12) as usize;
            let bytes = rng.bytes(len);
            let s = String::from_utf8_lossy(&bytes).into_owned();
            if s.starts_with('-') && s.len() > 1 {
                continue; // 選項會被解析，不屬於 STRING
            }
            parts.push(s.clone());
            args.push(s);
        }
        let want = parts.join(" ") + "\n";
        let r = run("echo", &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), "");
        assert_eq!(r.code, 0);
        assert_eq!(r.out(), want);
    }
}

#[test]
fn fuzz_no_newline_flag_consistency() {
    // -n 只影響結尾 newline，字串內容不變（-E 下不展開 escape）
    let mut rng = Rng::new(0xF00D);
    for _ in 0..100 {
        let len = 1 + rng.below(20) as usize;
        let bytes = rng.bytes(len);
        let s = String::from_utf8_lossy(&bytes).into_owned();
        if s.starts_with('-') {
            continue;
        }
        let full = run("echo", &["-n", &s], "");
        let with = run("echo", &[&s], "");
        assert_eq!(full.out() + "\n", with.out());
    }
}
