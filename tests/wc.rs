use coreutils::testutil::*;
use std::path::PathBuf;

fn write_file(name: &str, content: &[u8]) -> PathBuf {
    let p = workdir().join(name);
    std::fs::write(&p, content).expect("write test file");
    p
}

#[test]
fn default_counts_lines_words_bytes_from_stdin() {
    let r = run("wc", &[], "a b\nc d\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "2 4 8\n");
}

#[test]
fn file_output_has_filename() {
    let p = write_file("wc_one.txt", b"x y\n");
    let r = run("wc", &[p.to_str().unwrap()], "");
    assert_eq!(r.out(), format!("1 2 4 {}\n", p.to_str().unwrap()));
}

#[test]
fn total_line_appears_for_more_than_one_file() {
    let a = write_file("wc_a.txt", b"1\n");
    let b = write_file("wc_b.txt", b"2 3\n");
    let r = run("wc", &[a.to_str().unwrap(), b.to_str().unwrap()], "");
    let s = r.out();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2], "2 3 6 total");
}

#[test]
fn single_flag_selects_single_count() {
    let r = run("wc", &["-l"], "a\nb\n");
    assert_eq!(r.out(), "2\n");
    assert_eq!(run("wc", &["-w"], "a b c").out(), "3\n");
    assert_eq!(run("wc", &["-c"], "abc").out(), "3\n");
    assert_eq!(run("wc", &["-L"], "abc\nde").out(), "3\n");
}

#[test]
fn combined_flags_print_in_documented_order() {
    // 假設：輸出順序 = lines, words, bytes, chars, max-line-length
    let r = run("wc", &["-l", "-w", "-c"], "a b\n");
    assert_eq!(r.out(), "1 2 4\n");
    let r2 = run("wc", &["-lw"], "a b\n");
    assert_eq!(r2.out(), "1 2\n");
}

#[test]
fn chars_counts_multibyte_codepoints() {
    let r = run("wc", &["-m"], "éx\n");
    assert_eq!(r.out(), "3\n"); // 3 chars（含 newline）, 4 bytes
    let b = run("wc", &["-c"], "éx\n");
    assert_eq!(b.out(), "4\n");
}

#[test]
fn total_mode_always_and_never() {
    let a = write_file("wc_t1.txt", b"a\n");
    let r = run("wc", &["--total", "always", a.to_str().unwrap()], "");
    assert_eq!(r.out().lines().count(), 2);
    assert!(r.out().contains("total"));
    let r2 = run("wc", &["--total", "never", a.to_str().unwrap()], "");
    assert_eq!(r2.out().lines().count(), 1);
    let b = write_file("wc_t2.txt", b"b\n");
    let r3 = run("wc", &["--total", "only", a.to_str().unwrap(), b.to_str().unwrap()], "");
    let s3 = r3.out();
    let lines: Vec<&str> = s3.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("total"));
}

#[test]
fn files0_from_reads_nul_separated_names_from_stdin() {
    let p = write_file("wc_f0.txt", b"a b\n");
    let names = format!("{}\0", p.to_str().unwrap());
    let r = run("wc", &["--files0-from", "-"], &names);
    assert_eq!(r.out(), format!("1 2 4 {}\n", p.to_str().unwrap()));
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = run("wc", &["/tmp/no-such-wc-file"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("no-such-wc-file"), "stderr: {}", r.err());
}

#[test]
fn help_and_version() {
    let h = run("wc", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("wc"));
    assert!(run("wc", &["-V"], "").out().contains("wc"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin("wc", &["-l"], "a\nb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "2\n");
}

#[test]
fn fuzz_byte_and_line_counts_are_exact() {
    // man page 不變量：byte 數 = 輸入長度；line 數 = '\n' 的個數
    let mut rng = Rng::new(0x9CC);
    for _ in 0..200 {
        let len = rng.below(200) as usize;
        let data = rng.bytes(len);
        let r = run_bytes("wc", &["-c"], &data);
        assert_eq!(r.out().trim(), len.to_string());
        let nl = data.iter().filter(|&&b| b == b'\n').count();
        let r2 = run_bytes("wc", &["-l"], &data);
        assert_eq!(r2.out().trim(), nl.to_string());
    }
}

#[test]
fn fuzz_word_count_matches_whitespace_runs() {
    // 假定義：word = 非空白字元的連續段
    let mut rng = Rng::new(0x500D);
    for _ in 0..200 {
        let len = rng.below(120) as usize;
        let data = rng.bytes(len);
        let mut words = 0usize;
        let mut in_word = false;
        for &b in &data {
            if b.is_ascii_whitespace() {
                in_word = false;
            } else if !in_word {
                words += 1;
                in_word = true;
            }
        }
        let r = run_bytes("wc", &["-w"], &data);
        assert_eq!(r.out().trim(), words.to_string());
    }
}
