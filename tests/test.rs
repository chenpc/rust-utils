use coreutils::testutil as t;

fn r(args: &[&str]) -> t::Out {
    t::run("test", args, "")
}

fn b(args: &[&str]) -> t::Out {
    t::run("[", args, "")
}

fn wd(name: &str) -> String {
    let p = t::workdir().join(name);
    p.to_str().unwrap().to_string()
}

#[test]
fn string_length_operators() {
    assert_eq!(r(&["-n", "abc"]).code, 0);
    assert_eq!(r(&["-n", ""]).code, 1);
    assert_eq!(r(&["-z", ""]).code, 0);
    assert_eq!(r(&["-z", "abc"]).code, 1);
}

#[test]
fn bare_string_is_truthy_when_nonempty() {
    assert_eq!(r(&["x"]).code, 0);
    assert_eq!(r(&[""]).code, 1);
}

#[test]
fn omitted_expression_is_false() {
    assert_eq!(r(&[]).code, 1);
    assert_eq!(b(&["]"]).code, 1);
}

#[test]
fn string_comparison() {
    assert_eq!(r(&["a", "=", "a"]).code, 0);
    assert_eq!(r(&["a", "=", "b"]).code, 1);
    assert_eq!(r(&["a", "!=", "b"]).code, 0);
    assert_eq!(r(&["a", "<", "b"]).code, 0);
    assert_eq!(r(&["b", ">", "a"]).code, 0);
    assert_eq!(r(&["b", "<", "a"]).code, 1);
}

#[test]
fn integer_comparison() {
    assert_eq!(r(&["1", "-eq", "1"]).code, 0);
    assert_eq!(r(&["1", "-ne", "2"]).code, 0);
    assert_eq!(r(&["2", "-lt", "3"]).code, 0);
    assert_eq!(r(&["3", "-le", "3"]).code, 0);
    assert_eq!(r(&["4", "-gt", "3"]).code, 0);
    assert_eq!(r(&["4", "-ge", "4"]).code, 0);
    assert_eq!(r(&["5", "-lt", "10"]).code, 0);
    assert_eq!(r(&["x", "-eq", "1"]).code, 2);
}

#[test]
fn logical_operators() {
    assert_eq!(r(&["a", "=", "a", "-a", "b", "=", "b"]).code, 0);
    assert_eq!(r(&["a", "=", "a", "-a", "a", "=", "b"]).code, 1);
    assert_eq!(r(&["a", "=", "b", "-o", "c", "=", "c"]).code, 0);
    assert_eq!(r(&["a", "=", "b", "-o", "c", "=", "d"]).code, 1);
    assert_eq!(r(&["!", "a", "=", "b"]).code, 0);
    assert_eq!(r(&["!", "a", "=", "a"]).code, 1);
    assert_eq!(r(&["(", "a", "=", "b", ")", "-o", "c", "=", "c"]).code, 0);
    assert_eq!(r(&["(", "a", "=", "b"]).code, 2);
}

#[test]
fn precedence_and_or() {
    // -a binds tighter than -o
    assert_eq!(r(&["a", "=", "b", "-o", "c", "=", "c", "-a", "d", "=", "d"]).code, 0);
    assert_eq!(r(&["a", "=", "a", "-a", "b", "=", "c", "-o", "d", "=", "d"]).code, 0);
    assert_eq!(r(&["a", "=", "b", "-a", "c", "=", "c", "-o", "d", "=", "e"]).code, 1);
}

#[test]
fn file_tests_on_created_files() {
    let f = wd("tf_file");
    std::fs::write(&f, "hello").unwrap();
    let d = wd("tf_dir");
    let _ = std::fs::create_dir(&d);
    assert_eq!(r(&["-e", &f]).code, 0);
    assert_eq!(r(&["-f", &f]).code, 0);
    assert_eq!(r(&["-d", &f]).code, 1);
    assert_eq!(r(&["-d", &d]).code, 0);
    assert_eq!(r(&["-f", &d]).code, 1);
    assert_eq!(r(&["-s", &f]).code, 0);
    assert_eq!(r(&["-e", &wd("tf_missing")]).code, 1);
    assert_eq!(r(&["-b", &f]).code, 1);
    assert_eq!(r(&["-c", &f]).code, 1);
    assert_eq!(r(&["-p", &f]).code, 1);
    assert_eq!(r(&["-S", &f]).code, 1);
    assert_eq!(r(&["-r", &f]).code, 0);
    assert_eq!(r(&["-w", &f]).code, 0);
    assert_eq!(r(&["-x", &f]).code, 1);
    assert_eq!(r(&["-L", &f]).code, 1);
    assert_eq!(r(&["-h", &f]).code, 1);
}

#[test]
fn empty_file_is_not_sized() {
    let f = wd("tf_empty");
    std::fs::write(&f, "").unwrap();
    assert_eq!(r(&["-s", &f]).code, 1);
    assert_eq!(r(&["-e", &f]).code, 0);
}

#[test]
fn executable_bit() {
    let f = wd("tf_exec");
    std::fs::write(&f, "x").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(r(&["-x", &f]).code, 0);
    assert_eq!(r(&["-r", &f]).code, 0);
    assert_eq!(r(&["-w", &f]).code, 0);
}

#[test]
fn symlink_test() {
    let target = wd("tf_target");
    std::fs::write(&target, "x").unwrap();
    let link = wd("tf_link");
    let _ = std::fs::remove_file(&link);
    if std::os::unix::fs::symlink(&target, &link).is_err() {
        return;
    }
    assert_eq!(r(&["-L", &link]).code, 0);
    assert_eq!(r(&["-h", &link]).code, 0);
    assert_eq!(r(&["-f", &link]).code, 0);
    assert_eq!(r(&["-L", &target]).code, 1);
}

#[test]
fn setgid_and_sticky_bits() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let f = wd("tf_gid");
        std::fs::write(&f, "x").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o2644)).unwrap();
        assert_eq!(r(&["-g", &f]).code, 0);
        assert_eq!(r(&["-k", &f]).code, 1);
        let g = wd("tf_sticky");
        std::fs::write(&g, "x").unwrap();
        std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o1644)).unwrap();
        assert_eq!(r(&["-k", &g]).code, 0);
        assert_eq!(r(&["-g", &g]).code, 1);
    }
}

#[test]
fn newer_and_older() {
    let a = wd("tf_new");
    let b = wd("tf_old");
    std::fs::write(&a, "x").unwrap();
    std::fs::write(&b, "x").unwrap();
    assert_eq!(r(&[&a, "-nt", &b]).code, 1);
    assert_eq!(r(&[&b, "-ot", &a]).code, 1);
    assert_eq!(r(&[&a, "-ef", &b]).code, 1);
    assert_eq!(r(&[&a, "-ef", &a]).code, 0);
}

#[test]
fn bracket_requires_closing() {
    assert_eq!(b(&["a", "=", "a"]).code, 2);
    assert!(b(&["a", "=", "a"]).err().contains("]"));
    assert_eq!(b(&["a", "=", "a", "]"]).code, 0);
    assert_eq!(b(&["a", "=", "b", "]"]).code, 1);
}

#[test]
fn test_and_bracket_agree() {
    let exprs: Vec<Vec<&str>> = vec![
        vec!["a", "=", "a"],
        vec!["-n", "x"],
        vec!["1", "-lt", "2"],
        vec!["-z", ""],
    ];
    for expr in &exprs {
        let mut with = expr.clone();
        with.push("]");
        assert_eq!(r(expr).code, b(&with).code);
    }
}

#[test]
fn terminal_test() {
    // guest 內 stdin 是 console（tty）；host 上跑時不是 tty，所以只驗證不會崩潰
    let got = r(&["-t", "0"]);
    assert!(got.code == 0 || got.code == 1);
    assert_eq!(r(&["-t", "9"]).code, 1);
}

#[test]
fn syntax_errors_exit_2() {
    assert_eq!(r(&["-n"]).code, 2);
    assert_eq!(r(&["a", "="]).code, 2);
    assert_eq!(r(&["a", "=", "a", "extra"]).code, 2);
    assert_eq!(r(&["-eq", "1"]).code, 2);
}

#[test]
fn help_and_version() {
    let h = r(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().contains("test"));
    let v = r(&["-V"]);
    assert_eq!(v.code, 0);
    assert!(v.out().contains("test"));
    let bh = b(&["--help"]);
    assert_eq!(bh.code, 0);
    assert!(bh.out().contains("["));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("test", &["a", "=", "a"], "");
    assert_eq!(o.code, 0);
    let o2 = t::run_bin("[", &["-n", "x", "]"], "");
    assert_eq!(o2.code, 0);
}

#[test]
fn fuzz_string_and_integer_operators() {
    let mut rng = t::Rng::new(0x77);
    for _ in 0..200 {
        let la = 1 + rng.below(6) as usize;
        let lb = 1 + rng.below(6) as usize;
        let a = rng.bytes(la).to_vec();
        let b2 = rng.bytes(lb).to_vec();
        let as_str = String::from_utf8_lossy(&a).to_string();
        let bs_str = String::from_utf8_lossy(&b2).to_string();
        // 字串比較：實作必須與 Rust 的字典序一致
        let got = r(&[&as_str, "=", &bs_str]);
        assert_eq!(got.code, if as_str == bs_str { 0 } else { 1 });
        let got2 = r(&[&as_str, "<", &bs_str]);
        assert_eq!(got2.code, if as_str < bs_str { 0 } else { 1 });
        // -n / -z
        assert_eq!(r(&["-n", &as_str]).code, 0);
        assert_eq!(r(&["-z", &as_str]).code, 1);
        // 整數比較
        let x = rng.below(1000) as i64;
        let y = rng.below(1000) as i64;
        let xs = x.to_string();
        let ys = y.to_string();
        assert_eq!(r(&[&xs, "-eq", &ys]).code, if x == y { 0 } else { 1 });
        assert_eq!(r(&[&xs, "-lt", &ys]).code, if x < y { 0 } else { 1 });
        assert_eq!(r(&[&xs, "-ge", &ys]).code, if x >= y { 0 } else { 1 });
    }
}

#[test]
fn fuzz_combined_expressions() {
    let mut rng = t::Rng::new(0x1234);
    for _ in 0..150 {
        let a = rng.below(2) == 1;
        let b2 = rng.below(2) == 1;
        let c = rng.below(2) == 1;
        let sa = if a { "t" } else { "" };
        let sb = if b2 { "t" } else { "" };
        let sc = if c { "t" } else { "" };
        // -a 比 -o 優先：a OR (b AND c)
        let got = r(&[sa, "-o", sb, "-a", sc]);
        assert_eq!(got.code, if a || (b2 && c) { 0 } else { 1 });
        // 否定
        let got2 = r(&["!", sa]);
        assert_eq!(got2.code, if !a { 0 } else { 1 });
    }
}
