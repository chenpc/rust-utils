use coreutils::testutil as t;

fn wfile(name: &str, content: &str) -> String {
    let w = t::workdir();
    let p = w.join(name);
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn joins_on_first_field() {
    let a = wfile("j1.txt", "a 1\nb 2\n");
    let b = wfile("j2.txt", "a 3\nb 4\n");
    let r = t::run("join", &[&a, &b], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a 1 3\nb 2 4\n");
}

#[test]
fn delimiter_option() {
    let a = wfile("j3.txt", "a,1\nb,2\n");
    let b = wfile("j4.txt", "a,3\nb,4\n");
    let r = t::run("join", &["-t", ",", &a, &b], "");
    assert_eq!(r.out(), "a,1,3\nb,2,4\n");
}

#[test]
fn field_selection() {
    let a = wfile("j5.txt", "x a\ny b\n");
    let b = wfile("j6.txt", "a 1\nb 2\n");
    let r = t::run("join", &["-1", "2", &a, &b], "");
    assert_eq!(r.out(), "a x 1\nb y 2\n");
}

#[test]
fn shared_join_field() {
    let a = wfile("j7.txt", "k 1
m 2
");
    let b = wfile("j8.txt", "k 3
m 4
");
    let r = t::run("join", &["-j", "1", &a, &b], "");
    assert_eq!(r.out(), "k 1 3\nm 2 4\n");
}

#[test]
fn unpairable_from_file1() {
    let a = wfile("j9.txt", "a 1\nz 9\n");
    let b = wfile("j10.txt", "a 2\n");
    let r = t::run("join", &["-a", "1", &a, &b], "");
    assert_eq!(r.out(), "a 1 2\nz 9\n");
}

#[test]
fn unpairable_only() {
    let a = wfile("j11.txt", "a 1\nz 9\n");
    let b = wfile("j12.txt", "a 2\nw 8\n");
    let r = t::run("join", &["-v", "2", &a, &b], "");
    assert_eq!(r.out(), "w 8\n");
}

#[test]
fn empty_field_replacement() {
    let a = wfile("j13.txt", "a\nb 2\n");
    let b = wfile("j14.txt", "a 3\nb 4\n");
    let r = t::run("join", &["-o", "0 1.2 2.2", "-e", "-", &a, &b], "");
    assert_eq!(r.out(), "a - 3\nb 2 4\n");
}

#[test]
fn output_format() {
    let a = wfile("j15.txt", "a 1\n");
    let b = wfile("j16.txt", "a 3\n");
    let r = t::run("join", &["-o", "1.2,2.2", &a, &b], "");
    assert_eq!(r.out(), "1 3\n");
}

#[test]
fn ignore_case() {
    let a = wfile("j17.txt", "A 1\n");
    let b = wfile("j18.txt", "a 2\n");
    let r = t::run("join", &["-i", &a, &b], "");
    assert_eq!(r.out(), "A 1 2\n");
}

#[test]
fn header_is_preserved() {
    let a = wfile("j19.txt", "key x\na 1\n");
    let b = wfile("j20.txt", "key y\na 2\n");
    let r = t::run("join", &["--header", &a, &b], "");
    assert_eq!(r.out(), "key x\nkey y\na 1 2\n");
}

#[test]
fn zero_terminated() {
    let a = wfile("j21.txt", "a 1\0");
    let b = wfile("j22.txt", "a 2\0");
    let r = t::run("join", &["-z", &a, &b], "");
    assert_eq!(r.out(), "a 1 2\0");
}

#[test]
fn unsorted_input_is_error() {
    let a = wfile("j23.txt", "b 1\na 2\n");
    let b = wfile("j24.txt", "a 3\n");
    let r = t::run("join", &["--check-order", &a, &b], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("not in sorted order"));
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let a = wfile("j25.txt", "a 1\n");
    let r = t::run("join", &[&a, "/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("join: /nonexistent/x"));
}

#[test]
fn missing_operand_is_exit_2() {
    let r = t::run("join", &["a"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("join: missing operand"));
}

#[test]
fn help_and_version() {
    let r = t::run("join", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("join"));
    let v = t::run("join", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("join"));
}

#[test]
fn e2e_binary_dispatch() {
    let a = wfile("j26.txt", "a 1\n");
    let b = wfile("j27.txt", "a 2\n");
    let r = t::run_bin("join", &[&a, &b], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a 1 2\n");
}

#[test]
fn fuzz_joined_lines_share_the_key() {
    let mut rng = t::Rng::new(0xCA);
    for _ in 0..100 {
        let n = 1 + rng.below(10);
        let keys: Vec<String> = (0..n)
            .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap().to_string())
            .collect();
        let mut k1 = keys.clone();
        k1.sort();
        k1.dedup();
        let mut k2 = keys[..(1 + rng.below(n as u64) as usize)].to_vec();
        k2.sort();
        k2.dedup();
        let a_lines: Vec<String> = k1.iter().map(|k| format!("{k} A")).collect();
        let b_lines: Vec<String> = k2.iter().map(|k| format!("{k} B")).collect();
        let s1 = a_lines.join("\n") + "\n";
        let a = wfile("fj1.txt", &s1);
        let s2 = b_lines.join("\n") + "\n";
        let b = wfile("fj2.txt", &s2);
        let r = t::run("join", &[&a, &b], "");
        assert_eq!(r.code, 0);
        let o = r.out();
        let out: Vec<&str> = o.lines().collect();
        for line in &out {
            let parts: Vec<&str> = line.split(' ').collect();
            let k = parts[0].to_string();
            assert!(k1.contains(&k) && k2.contains(&k), "line={line}");
            assert!(parts.contains(&"A") && parts.contains(&"B"), "line={line}");
        }
        let expected = k2.iter().filter(|k| k1.contains(k)).count();
        assert_eq!(out.len(), expected, "out={}", r.out());
    }
}
