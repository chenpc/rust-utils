use coreutils::testutil as t;

fn wfile(name: &str, content: &str) -> String {
    let w = t::workdir();
    let p = w.join(name);
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

fn clean() {
    let w = t::workdir();
    for e in std::fs::read_dir(&w).unwrap() {
        let n = e.unwrap().file_name().to_string_lossy().to_string();
        if n.starts_with("cs") && !n.ends_with(".txt") {
            std::fs::remove_file(w.join(n)).unwrap();
        }
    }
}

fn prefix() -> String {
    t::workdir().join("cs").to_str().unwrap().to_string()
}

#[test]
fn splits_on_pattern() {
    clean();
    let f = wfile("cs_in1.txt", "a\nb\nc\nd\n");
    let r = t::run("csplit", &["-f", &prefix(), &f, "/c/"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "4\n4\n");
    assert_eq!(std::fs::read_to_string(prefix() + "00").unwrap(), "a\nb\n");
    assert_eq!(std::fs::read_to_string(prefix() + "01").unwrap(), "c\nd\n");
}

#[test]
fn line_number_pattern() {
    clean();
    let f = wfile("cs_in2.txt", "a\nb\nc\nd\n");
    let r = t::run("csplit", &["-f", &prefix(), &f, "3"], "");
    assert_eq!(r.out(), "4\n4\n");
    assert_eq!(std::fs::read_to_string(prefix() + "01").unwrap(), "c\nd\n");
}

#[test]
fn suppress_matched() {
    clean();
    let f = wfile("cs_in3.txt", "a\nb\nc\nd\n");
    let r = t::run("csplit", &["-f", &prefix(), "--suppress-matched", &f, "/b/"], "");
    assert_eq!(r.out(), "2\n4\n");
    assert_eq!(std::fs::read_to_string(prefix() + "00").unwrap(), "a\n");
    assert_eq!(std::fs::read_to_string(prefix() + "01").unwrap(), "c\nd\n");
}

#[test]
fn percent_pattern_skips_matched_line() {
    clean();
    let f = wfile("cs_in4.txt", "a\nb\nc\n");
    let r = t::run("csplit", &["-f", &prefix(), &f, "%b%"], "");
    assert_eq!(r.out(), "2\n2\n");
    assert_eq!(std::fs::read_to_string(prefix() + "01").unwrap(), "c\n");
}

#[test]
fn repeat_pattern() {
    clean();
    let f = wfile("cs_in5.txt", "a\nx\nb\nx\nc\n");
    let r = t::run("csplit", &["-f", &prefix(), &f, "/x/", "{1}"], "");
    assert_eq!(r.out(), "2\n4\n4\n");
    assert_eq!(std::fs::read_to_string(prefix() + "02").unwrap(), "x\nc\n");
}

#[test]
fn digits_and_suffix_format() {
    clean();
    let f = wfile("cs_in6.txt", "a\nb\n");
    let r = t::run("csplit", &["-f", &prefix(), "-n", "3", &f, "/b/"], "");
    assert_eq!(r.out(), "2\n2\n");
    assert_eq!(std::fs::read_to_string(prefix() + "001").unwrap(), "b\n");
    let r2 = t::run("csplit", &["-f", &prefix(), "-b", "part_%03d", &f, "/b/"], "");
    assert_eq!(r2.code, 0);
    assert_eq!(std::fs::read_to_string(prefix() + "part_001").unwrap(), "b\n");
}

#[test]
fn quiet_suppresses_counts() {
    clean();
    let f = wfile("cs_in7.txt", "a\nb\n");
    let r = t::run("csplit", &["-f", &prefix(), "-q", &f, "/b/"], "");
    assert_eq!(r.out(), "");
    assert_eq!(std::fs::read_to_string(prefix() + "00").unwrap(), "a\n");
}

#[test]
fn elide_empty_files() {
    clean();
    let f = wfile("cs_in8.txt", "b\nc\n");
    let r = t::run("csplit", &["-f", &prefix(), "-z", &f, "/b/"], "");
    assert_eq!(r.out(), "4\n");
    assert!(!std::fs::exists(prefix() + "00").unwrap());
}

#[test]
fn pattern_not_found_is_exit_1() {
    let f = wfile("cs_in9.txt", "a\nb\n");
    let r = t::run("csplit", &["-f", &prefix(), "-q", &f, "/zzz/"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("line not found"));
}

#[test]
fn keep_files_on_error() {
    clean();
    let f = wfile("cs_in10.txt", "a\nb\nc\n");
    let r = t::run("csplit", &["-f", &prefix(), "-k", "-q", &f, "/b/", "/zzz/"], "");
    assert_eq!(r.code, 1);
    assert_eq!(std::fs::read_to_string(prefix() + "00").unwrap(), "a\n");
}

#[test]
fn missing_operand_is_exit_2() {
    let r = t::run("csplit", &["-f", &prefix(), "-q", "onlyfile"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("csplit: missing operand"));
}

#[test]
fn help_and_version() {
    let r = t::run("csplit", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("csplit"));
    let v = t::run("csplit", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("csplit"));
}

#[test]
fn e2e_binary_dispatch() {
    let f = wfile("cs_in11.txt", "a\nb\n");
    let r = t::run_bin("csplit", &["-f", &prefix(), "-q", &f, "/b/"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(prefix() + "01").unwrap(), "b\n");
}

#[test]
fn fuzz_sections_reconstruct_input() {
    let mut rng = t::Rng::new(0xCB);
    for _ in 0..60 {
        let n = 1 + rng.below(12);
        let lines: Vec<String> = (0..n)
            .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap().to_string())
            .collect();
        let input = lines.join("\n") + "\n";
        let f = wfile("cs_fuzz.txt", &input);
        let cut = 1 + rng.below(n as u64) as usize;
        let r = t::run("csplit", &["-f", &prefix(), "-q", &f, &cut.to_string()], "");
        assert_eq!(r.code, 0);
        let mut rebuilt = String::new();
        for i in 0..2 {
            let p = prefix() + &format!("{i:0>2}");
            if std::fs::exists(&p).unwrap() {
                rebuilt.push_str(&std::fs::read_to_string(&p).unwrap());
            }
        }
        assert_eq!(rebuilt, input, "cut={cut} out={}", r.out());
    }
}
