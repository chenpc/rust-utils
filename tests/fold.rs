use coreutils::testutil as t;

#[test]
fn default_width_80() {
    let line = "x".repeat(100);
    let r = t::run("fold", &[], format!("{line}\n").as_str());
    let o = r.out();
    let got = o.lines().collect::<Vec<_>>();
    assert_eq!(got[0].len(), 80);
    assert_eq!(got[1].len(), 20);
}

#[test]
fn width_splits() {
    let r = t::run("fold", &["-w", "4"], "abcdefghij\n");
    assert_eq!(r.out(), "abcd\nefgh\nij\n");
}

#[test]
fn spaces_break_at_last_space() {
    let r = t::run("fold", &["-w", "4", "-s"], "a b c d e\n");
    assert_eq!(r.out(), "a b \nc d \ne\n");
}

#[test]
fn short_line_unchanged() {
    let r = t::run("fold", &["-w", "10"], "abc\n");
    assert_eq!(r.out(), "abc\n");
}

#[test]
fn characters_count_multibyte() {
    let r = t::run("fold", &["-c", "-w", "2"], "aébc\n");
    assert_eq!(r.out(), "aé\nbc\n");
}

#[test]
fn bytes_mode_splits_raw_bytes() {
    let r = t::run_bytes("fold", &["-b", "-w", "2"], "aé\n".as_bytes());
    assert_eq!(r.out_bytes(), b"a\xC3\n\xA9\n");
}

#[test]
fn invalid_width_is_exit_2() {
    let r = t::run("fold", &["-w", "0"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid width"));
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("fold", &["-w", "4", "/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("fold: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("fold", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("fold"));
    let v = t::run("fold", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("fold"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("fold", &["-w", "3"], "abcdef\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "abc\ndef\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(2);
    let mut e = Vec::new();
    let code = t::run_with("fold", &["-w", "3"], b"abcdef\nabcdef\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_chunks_rebuild_input() {
    let mut rng = t::Rng::new(0xF01D);
    for _ in 0..120 {
        let len = 1 + rng.below(60) as usize;
        let mut data = rng.bytes(len);
        for b in data.iter_mut() {
            if *b == 0x0A || *b >= 0x80 {
                *b = 0x20;
            }
        }
        let w = 1 + rng.below(20) as usize;
        let mut input = data.clone();
        input.push(b'\n');
        let r = t::run_bytes("fold", &["-w", &w.to_string()], &input);
        assert_eq!(r.code, 0);
        let rebuilt: Vec<u8> = r.out_bytes().iter().filter(|b| **b != b'\n').copied().collect();
        assert_eq!(rebuilt, data, "w={w}");
    }
}
