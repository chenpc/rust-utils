use coreutils::testutil as t;

fn r(args: &[&str], input: &str) -> t::Out {
    t::run("shuf", args, input)
}

#[test]
fn same_seed_is_reproducible() {
    let input = "a\nb\nc\nd\ne\n";
    let a = r(&["--random-seed", "42"], input);
    let b = r(&["--random-seed", "42"], input);
    assert_eq!(a.out(), b.out());
    assert_eq!(a.out().lines().count(), 5);
}

#[test]
fn different_seed_can_reorder() {
    let input = "a\nb\nc\nd\ne\nf\ng\nh\n";
    let a = r(&["--random-seed", "1"], input);
    let b = r(&["--random-seed", "2"], input);
    assert_ne!(a.out(), b.out());
}

#[test]
fn permutation_preserves_the_multiset() {
    let mut rng = t::Rng::new(0x1234);
    for _ in 0..120 {
        let n = 1 + rng.below(12) as usize;
        let mut lines = Vec::new();
        for _ in 0..n {
            lines.push(format!("l{}", rng.below(20)));
        }
        let input = lines.join("\n") + "\n";
        let got = r(&["--random-seed", "7"], &input);
        let mut a: Vec<String> = got.out().lines().map(|s| s.to_string()).collect();
        let mut b = lines.clone();
        a.sort();
        b.sort();
        assert_eq!(a, b, "multiset preserved");
    }
}

#[test]
fn head_count_limits_output() {
    let input = "a\nb\nc\nd\ne\n";
    let got = r(&["-n", "3", "--random-seed", "5"], input);
    assert_eq!(got.out().lines().count(), 3);
    for line in got.out().lines() {
        assert!(input.contains(line), "{line} not in input");
    }
}

#[test]
fn head_count_larger_than_input_keeps_all() {
    let got = r(&["-n", "10", "--random-seed", "5"], "a\nb\n");
    assert_eq!(got.out().lines().count(), 2);
}

#[test]
fn echo_treats_args_as_lines() {
    let got = r(&["-e", "--random-seed", "3", "x", "y", "z"], "");
    let mut a: Vec<String> = got.out().lines().map(|s| s.to_string()).collect();
    a.sort();
    assert_eq!(a, vec!["x".to_string(), "y".to_string(), "z".to_string()]);
}

#[test]
fn input_range() {
    let got = r(&["-i", "1-5", "--random-seed", "9"], "");
    let mut a: Vec<String> = got.out().lines().map(|s| s.to_string()).collect();
    a.sort();
    assert_eq!(a, vec!["1", "2", "3", "4", "5"]);
}

#[test]
fn repeat_can_duplicate() {
    let got = r(&["-r", "-n", "20", "--random-seed", "11", "-e", "a", "b"], "");
    let lines: Vec<String> = got.out().lines().map(|s| s.to_string()).collect();
    assert_eq!(lines.len(), 20);
    for l in &lines {
        assert!(l == "a" || l == "b", "{l}");
    }
    assert!(lines.iter().filter(|l| *l == "a").count() > 0);
    assert!(lines.iter().filter(|l| *l == "b").count() > 0);
}

#[test]
fn zero_terminated() {
    let got = r(&["-z", "--random-seed", "1", "-e", "a", "b"], "");
    assert_eq!(got.out_bytes().split(|c| *c == b'\n').count(), 1);
    assert!(got.out_bytes().contains(&0));
}

#[test]
fn writes_to_output_file() {
    let out = t::workdir().join("sh_out.txt");
    let got = r(&["-o", out.to_str().unwrap(), "--random-seed", "4", "-e", "a", "b"], "");
    assert_eq!(got.code, 0);
    let content = std::fs::read_to_string(&out).unwrap();
    let mut a: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    a.sort();
    assert_eq!(a, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn invalid_range_is_an_error() {
    let got = r(&["-i", "x-5"], "");
    assert_eq!(got.code, 1);
    assert!(got.err().contains("shuf"));
}

#[test]
fn missing_operand_is_an_error() {
    assert_eq!(r(&["-n"], "").code, 1);
    assert_eq!(r(&["-i"], "").code, 1);
}

#[test]
fn empty_input_produces_no_output() {
    let got = r(&["--random-seed", "1"], "");
    assert_eq!(got.code, 0);
    assert_eq!(got.out(), "");
}

#[test]
fn help_and_version() {
    let h = r(&["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("shuf"));
    let v = r(&["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("shuf"));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("shuf", &["-e", "--random-seed", "8", "a", "b", "c"], "");
    let mut a: Vec<String> = o.out().lines().map(|s| s.to_string()).collect();
    a.sort();
    assert_eq!(a, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
}

#[test]
fn fuzz_head_count_never_exceeds_input() {
    let mut rng = t::Rng::new(0x99);
    for _ in 0..120 {
        let n = 1 + rng.below(15) as usize;
        let mut lines = Vec::new();
        for _ in 0..n {
            lines.push(format!("v{}", rng.below(30)));
        }
        let input = lines.join("\n") + "\n";
        let k = 1 + rng.below(20) as usize;
        let got = r(&["-n", &k.to_string(), "--random-seed", "seed"], &input);
        let out_lines: Vec<String> = got.out().lines().map(|s| s.to_string()).collect();
        assert_eq!(out_lines.len(), k.min(n));
        for l in &out_lines {
            assert!(lines.contains(l), "{l} not from input");
        }
    }
}
