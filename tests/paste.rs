use coreutils::testutil as t;

#[test]
fn parallel_joins_with_tab() {
    let w = t::workdir();
    std::fs::write(w.join("a"), "1\n2\n3\n").unwrap();
    std::fs::write(w.join("b"), "x\ny\n").unwrap();
    let r = t::run("paste", &[w.join("a").to_str().unwrap(), w.join("b").to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1\tx\n2\ty\n3\t\n");
}

#[test]
fn stdin_default_is_a_file() {
    let r = t::run("paste", &[], "a\nb\n");
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn custom_delimiters_cycle() {
    let w = t::workdir();
    std::fs::write(w.join("a"), "1\n2\n").unwrap();
    std::fs::write(w.join("b"), "x\ny\n").unwrap();
    std::fs::write(w.join("c"), "p\nq\n").unwrap();
    let r = t::run("paste", &["-d", ",;", w.join("a").to_str().unwrap(), w.join("b").to_str().unwrap(), w.join("c").to_str().unwrap()], "");
    assert_eq!(r.out(), "1,x;p\n2,y;q\n");
}

#[test]
fn serial_mode() {
    let w = t::workdir();
    std::fs::write(w.join("a"), "1\n2\n").unwrap();
    std::fs::write(w.join("b"), "x\ny\n").unwrap();
    let r = t::run("paste", &["-s", w.join("a").to_str().unwrap(), w.join("b").to_str().unwrap()], "");
    assert_eq!(r.out(), "1\t2\nx\ty\n");
}

#[test]
fn serial_mode_with_delimiters() {
    let w = t::workdir();
    std::fs::write(w.join("a"), "1\n2\n").unwrap();
    std::fs::write(w.join("b"), "3\n4\n").unwrap();
    let r = t::run("paste", &["-s", "-d", ",", w.join("a").to_str().unwrap()], "");
    assert_eq!(r.out(), "1,2\n");
    let r2 = t::run("paste", &["-s", "-d", ",;", w.join("a").to_str().unwrap(), w.join("b").to_str().unwrap()], "");
    assert_eq!(r2.out(), "1,2\n3,4\n");
}

#[test]
fn zero_terminated() {
    let w = t::workdir();
    std::fs::write(w.join("a"), "1\02\0").unwrap();
    std::fs::write(w.join("b"), "x\0").unwrap();
    let r = t::run("paste", &["-z", w.join("a").to_str().unwrap(), w.join("b").to_str().unwrap()], "");
    assert_eq!(r.out(), "1\tx\02\t\0");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("paste", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("paste: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("paste", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("paste"));
    let v = t::run("paste", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("paste"));
}

#[test]
fn unknown_option_is_exit_2() {
    let r = t::run("paste", &["-x"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("unknown option"));
}

#[test]
fn missing_delimiter_list_is_exit_2() {
    let r = t::run("paste", &["-d"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing delimiter"));
}

#[test]
fn serial_missing_file_is_error_and_exit_1() {
    let r = t::run("paste", &["-s", "/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("paste: /nonexistent/x"));
}

#[test]
fn fuzz_serial_joins_lines() {
    let mut rng = t::Rng::new(0xBEEF);
    for _ in 0..60 {
        let d = char::from_u32(0x21 + rng.below(60) as u32).unwrap();
        if d == '\\' { continue; }
        let n = 1 + rng.below(4);
        let mut lines = Vec::new();
        let mut file = String::new();
        for _ in 0..n {
            let s: String = (0..rng.below(5)).map(|_| {
                let c = loop {
                    let c = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
                    if c != d && c != '\n' { break c; }
                };
                c
            }).collect();
            lines.push(s.clone());
            file.push_str(&s);
            file.push('\n');
        }
        let w = t::workdir();
        std::fs::write(w.join("sf"), file.as_bytes()).unwrap();
        let dstr = d.to_string();
        let path = w.join("sf").to_str().unwrap().to_string();
        let r = t::run("paste", &["-s", "-d", &dstr, &path], "");
        assert_eq!(r.code, 0, "d={:?} err={}", d, r.err());
        let expected: String = lines.join(&d.to_string()) + "\n";
        assert_eq!(r.out(), expected, "d={:?}", d);
    }
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("paste", &[], "a\nb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn fuzz_parallel_fields_match_lines() {
    let mut rng = t::Rng::new(0x9E37);
    for _ in 0..100 {
        let d = char::from_u32(0x21 + rng.below(60) as u32).unwrap();
        if d == '\\' { continue; }
        let na = 1 + rng.below(4);
        let nb = 1 + rng.below(4);
        let mut fa = String::new();
        let mut fb = String::new();
        let mut la = Vec::new();
        let mut lb = Vec::new();
        for _ in 0..na {
            let s: String = (0..rng.below(5)).map(|_| {
                let c = loop {
                    let c = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
                    if c != d && c != '\n' { break c; }
                };
                c
            }).collect();
            la.push(s.clone());
            fa.push_str(&s);
            fa.push('\n');
        }
        for _ in 0..nb {
            let s: String = (0..rng.below(5)).map(|_| {
                let c = loop {
                    let c = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
                    if c != d && c != '\n' { break c; }
                };
                c
            }).collect();
            lb.push(s.clone());
            fb.push_str(&s);
            fb.push('\n');
        }
        let w = t::workdir();
        std::fs::write(w.join("fa"), fa.as_bytes()).unwrap();
        std::fs::write(w.join("fb"), fb.as_bytes()).unwrap();
        let r = t::run("paste", &["-d", &d.to_string(), w.join("fa").to_str().unwrap(), w.join("fb").to_str().unwrap()], "");
        assert_eq!(r.code, 0);
        let o = r.out();
        let rows: Vec<&str> = o.lines().collect();
        let n = la.len().max(nb as usize);
        assert_eq!(rows.len(), n);
        for i in 0..n {
            let a = la.get(i).cloned().unwrap_or_default();
            let b = lb.get(i).cloned().unwrap_or_default();
            assert_eq!(rows[i], format!("{}{}{}", a, d, b), "row {} d={:?} la={:?} lb={:?} rows={:?}", i, d, la, lb, rows);
        }
    }
}
