use coreutils::testutil as t;

fn wfile(name: &str, content: &str) -> String {
    let w = t::workdir();
    let p = w.join(name);
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string();
    p.to_str().unwrap().to_string()
}

fn clean() {
    let w = t::workdir();
    for e in std::fs::read_dir(&w).unwrap() {
        let n = e.unwrap().file_name().to_string_lossy().to_string();
        if n.starts_with("sp") && !n.ends_with(".txt") {
            std::fs::remove_file(w.join(n)).unwrap();
        }
    }
}

fn alphabetic(idx: usize) -> String {
    let mut s = String::new();
    let mut n = idx;
    for _ in 0..2 {
        s.insert(0, (b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s
}

fn pfx() -> String {
    t::workdir().join("sp").to_str().unwrap().to_string()
}

#[test]
fn default_is_1000_lines() {
    let input = (0..5).map(|i| format!("{i}\n")).collect::<String>();
    let f = wfile("sp_in1.txt", &input);
    let r = t::run("split", &[&f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), input);
}

#[test]
fn lines_per_file() {
    clean();
    let f = wfile("sp_in2.txt", "1\n2\n3\n4\n5\n");
    let r = t::run("split", &["-l", "2", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "1\n2\n");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "3\n4\n");
    assert_eq!(std::fs::read_to_string(pfx() + "ac").unwrap(), "5\n");
}

#[test]
fn bytes_per_file() {
    clean();
    let f = wfile("sp_in3.txt", "abcdef");
    let r = t::run("split", &["-b", "3", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "abc");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "def");
}

#[test]
fn line_bytes_does_not_split_a_line() {
    clean();
    let f = wfile("sp_in4.txt", "aaaa\nbbbb\n");
    let r = t::run("split", &["-C", "5", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "aaaa\n");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "bbbb\n");
}

#[test]
fn number_of_chunks() {
    clean();
    let f = wfile("sp_in5.txt", "abcdef");
    let r = t::run("split", &["-n", "2", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "abc");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "def");
}

#[test]
fn round_robin_chunks() {
    clean();
    let f = wfile("sp_in6.txt", "1\n2\n3\n4\n");
    let r = t::run("split", &["-n", "k/2", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "1\n3\n");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "2\n4\n");
}

#[test]
fn remainder_chunks() {
    clean();
    let f = wfile("sp_in7.txt", "abcde");
    let r = t::run("split", &["-n", "r/2", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "abc");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "de");
}

#[test]
fn numeric_suffixes() {
    clean();
    let f = wfile("sp_in8.txt", "ab");
    let r = t::run("split", &["-b", "1", "-d", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "00").unwrap(), "a");
    assert_eq!(std::fs::read_to_string(pfx() + "01").unwrap(), "b");
}

#[test]
fn hex_suffixes() {
    clean();
    let f = wfile("sp_in9.txt", "abc");
    let r = t::run("split", &["-b", "1", "-x", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "00").unwrap(), "a");
    assert_eq!(std::fs::read_to_string(pfx() + "02").unwrap(), "c");
}

#[test]
fn suffix_length() {
    clean();
    let f = wfile("sp_in10.txt", "ab");
    let r = t::run("split", &["-b", "1", "-a", "3", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aaa").unwrap(), "a");
}

#[test]
fn additional_suffix() {
    clean();
    let f = wfile("sp_in11.txt", "ab\n");
    let r = t::run("split", &["--additional-suffix", ".txt", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa.txt").unwrap(), "ab\n");
}

#[test]
fn verbose_reports_names() {
    clean();
    let f = wfile("sp_in12.txt", "ab\n");
    let r = t::run("split", &["--verbose", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("writing to"));
}

#[test]
fn separator_option() {
    clean();
    let f = wfile("sp_in13.txt", "a,b,c\n");
    let r = t::run("split", &["-l", "1", "-t", ",", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "a,");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "b,");
}

#[test]
fn elide_empty_with_chunks() {
    clean();
    let f = wfile("sp_in14.txt", "ab");
    let r = t::run("split", &["-n", "4", "-e", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "a");
    assert_eq!(std::fs::read_to_string(pfx() + "ab").unwrap(), "b");
    assert!(!std::fs::exists(pfx() + "ac").unwrap());
}

#[test]
fn filter_writes_through_shell() {
    clean();
    let f = wfile("sp_in15.txt", "ab\n");
    let out = t::workdir().join("filter_ok.txt");
    let cmd = format!("echo $FILE > {}", out.to_str().unwrap());
    let r = t::run("split", &["--filter", &cmd, &f, &pfx()], "");
    assert_eq!(r.code, 0, "err={}", r.err());
    assert_eq!(std::fs::read_to_string(&out).unwrap(), pfx() + "aa
");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("split", &["/nonexistent/x", &pfx()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("split: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("split", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("split"));
    let v = t::run("split", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("split"));
}

#[test]
fn e2e_binary_dispatch() {
    let f = wfile("sp_in16.txt", "ab\n");
    let r = t::run_bin("split", &["-l", "1", &f, &pfx()], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(pfx() + "aa").unwrap(), "ab\n");
}

#[test]
fn fuzz_parts_reconstruct_input() {
    let mut rng = t::Rng::new(0xCC);
    for _ in 0..60 {
        let len = 1 + rng.below(60) as usize;
        // ASCII-safe bytes: non-UTF8 bytes would be corrupted by the lossy String conversion
        let data: Vec<u8> = (0..len)
            .map(|_| 0x21 + (rng.below(90) as u8))
            .collect();
        let f = wfile("sp_fuzz.txt", &String::from_utf8_lossy(&data));
        for old in std::fs::read_dir(t::workdir()).unwrap() {
            let n = old.unwrap().file_name().to_string_lossy().to_string();
            if n.starts_with("sp") && n != "sp_fuzz.txt" {
                std::fs::remove_file(t::workdir().join(n)).unwrap();
            }
        }
        let k = 1 + rng.below(6) as usize;
        let r = t::run("split", &["-b", &k.to_string(), &f, &pfx()], "");
        assert_eq!(r.code, 0);
        let mut rebuilt = Vec::new();
        for i in 0..300 {
            let p = pfx() + &alphabetic(i);
            if std::fs::exists(&p).unwrap() {
                rebuilt.extend_from_slice(&std::fs::read(&p).unwrap());
            }
        }
        assert_eq!(rebuilt, data, "k={k}");
    }
}
