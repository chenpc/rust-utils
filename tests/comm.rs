use coreutils::testutil as t;

fn wfile(name: &str, content: &str) -> String {
    let w = t::workdir();
    let p = w.join(name);
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn three_columns() {
    let a = wfile("c1.txt", "a\nb\nc\n");
    let b = wfile("c2.txt", "b\nc\nd\n");
    let r = t::run("comm", &[&a, &b], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a\n\t\tb\n\t\tc\n\td\n");
}

#[test]
fn suppress_columns() {
    let a = wfile("c3.txt", "a\nb\n");
    let b = wfile("c4.txt", "b\n");
    let r = t::run("comm", &["-1", &a, &b], "");
    assert_eq!(r.out(), "\t\tb\n");
    let r2 = t::run("comm", &["-3", &a, &b], "");
    assert_eq!(r2.out(), "a\n");
}

#[test]
fn output_delimiter() {
    let a = wfile("c5.txt", "a\n");
    let b = wfile("c6.txt", "a\n");
    let r = t::run("comm", &["--output-delimiter", ",", &a, &b], "");
    assert_eq!(r.out(), ",,a\n");
}

#[test]
fn stdin_is_one_file() {
    let a = wfile("c7.txt", "a\nb\n");
    let r = t::run("comm", &[&a, "-"], "b\n");
    assert_eq!(r.out(), "a\n\t\tb\n");
}

#[test]
fn total_summary() {
    let a = wfile("c8.txt", "a\nb\n");
    let b = wfile("c9.txt", "b\n");
    let r = t::run("comm", &["--total", &a, &b], "");
    assert_eq!(r.out(), "a\n\t\tb\n1\t0\t1\ttotal\n");
}

#[test]
fn zero_terminated() {
    let a = wfile("c10.txt", "a\0b\0");
    let b = wfile("c11.txt", "b\0");
    let r = t::run("comm", &["-z", &a, &b], "");
    assert_eq!(r.out(), "a\0\t\tb\0");
}

#[test]
fn unsorted_input_is_error() {
    let a = wfile("c12.txt", "b\na\n");
    let b = wfile("c13.txt", "a\n");
    let r = t::run("comm", &["--check-order", &a, &b], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("not in sorted order"));
    let ok = t::run("comm", &["--nocheck-order", &a, &b], "");
    assert_eq!(ok.code, 0);
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let a = wfile("c14.txt", "a\n");
    let r = t::run("comm", &[&a, "/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("comm: /nonexistent/x"));
}

#[test]
fn missing_operand_is_exit_2() {
    let r = t::run("comm", &["a"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("comm: missing operand"));
}

#[test]
fn help_and_version() {
    let r = t::run("comm", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("comm"));
    let v = t::run("comm", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("comm"));
}

#[test]
fn e2e_binary_dispatch() {
    let a = wfile("c15.txt", "a\n");
    let b = wfile("c16.txt", "a\n");
    let r = t::run_bin("comm", &[&a, &b], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "\t\ta\n");
}

#[test]
fn fuzz_columns_partition_input() {
    let mut rng = t::Rng::new(0xC0);
    for _ in 0..100 {
        let n = 1 + rng.below(12);
        let mut l1: Vec<String> = (0..n)
            .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap().to_string())
            .collect();
        let mut l2: Vec<String> = (0..n)
            .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap().to_string())
            .collect();
        l1.sort();
        l1.dedup();
        l2.sort();
        l2.dedup();
        let s1 = l1.join("\n") + "\n";
        let a = wfile("f1.txt", &s1);
        let s2 = l2.join("\n") + "\n";
        let b = wfile("f2.txt", &s2);
        let r = t::run("comm", &[&a, &b], "");
        assert_eq!(r.code, 0);
        let o = r.out();
        let lines: Vec<&str> = o.lines().collect();
        // every output line is from one of the inputs, with the right column prefix
        for line in &lines {
            let col = line.chars().take_while(|&c| c == '\t').count();
            let body = line[col..].to_string();
            let expected = match col {
                0 => l1.contains(&body),
                1 => l2.contains(&body),
                _ => l1.contains(&body) && l2.contains(&body),
            };
            assert!(expected, "line={line}");
        }
        // counts must match the partition
        let common = l1.iter().filter(|x| l2.contains(x)).count();
        let only1 = l1.iter().filter(|x| !l2.contains(x)).count();
        let only2 = l2.iter().filter(|x| !l1.contains(x)).count();
        let got = [
            lines.iter().filter(|l| !l.starts_with('\t')).count(),
            lines.iter().filter(|l| l.starts_with("\t") && !l.starts_with("\t\t")).count(),
            lines.iter().filter(|l| l.starts_with("\t\t")).count(),
        ];
        assert_eq!(got, [only1, only2, common], "out={}", r.out());
    }
}
