use coreutils::testutil as t;

fn wfile(name: &str, content: &str) -> String {
    let p = t::workdir().join(name);
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn man_page_example() {
    let r = t::run("tsort", &[], "UI Backend\nBackend Database\nDocs UI");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "Docs\nUI\nBackend\nDatabase\n");
}

#[test]
fn simple_chain() {
    let r = t::run("tsort", &[], "a b\nb c\n");
    assert_eq!(r.out(), "a\nb\nc\n");
}

#[test]
fn reads_from_file() {
    let f = wfile("ts1.txt", "x y\ny z\n");
    let r = t::run("tsort", &[&f], "");
    assert_eq!(r.out(), "x\ny\nz\n");
}

#[test]
fn missing_file_is_error() {
    let r = t::run("tsort", &["/nonexistent/ts-missing"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("tsort"));
}

#[test]
fn cycle_is_reported() {
    let r = t::run("tsort", &[], "a b\nb a\n");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "tsort: cycle in data\n");
}

#[test]
fn single_token_is_its_own_node() {
    let r = t::run("tsort", &[], "only\n");
    assert_eq!(r.out(), "only\n");
}

#[test]
fn help_and_version() {
    let h = t::run("tsort", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("tsort"));
    let v = t::run("tsort", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("tsort"));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("tsort", &[], "a b\nb c\n");
    assert_eq!(o.out(), "a\nb\nc\n");
}

#[test]
fn fuzz_respects_every_dependency() {
    let mut rng = t::Rng::new(0x2A);
    for _ in 0..120 {
        let n = 2 + rng.below(8) as usize;
        let names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
        let mut lines = String::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut present: Vec<usize> = Vec::new();
        for _ in 0..n {
            let a = rng.below(n as u64) as usize;
            let b = rng.below(n as u64) as usize;
            if a == b {
                continue;
            }
            edges.push((a, b));
            lines.push_str(&format!("{} {}\n", names[a], names[b]));
            if !present.contains(&a) {
                present.push(a);
            }
            if !present.contains(&b) {
                present.push(b);
            }
        }
        if edges.is_empty() {
            continue;
        }
        let r = t::run("tsort", &[], &lines);
        if r.code != 0 {
            // a cycle is legitimate; the message must still be the documented one
            assert_eq!(r.err(), "tsort: cycle in data\n");
            continue;
        }
        let got: Vec<String> = r.out().lines().map(|s| s.to_string()).collect();
        assert_eq!(got.len(), present.len(), "all nodes printed");
        let pos: std::collections::HashMap<String, usize> =
            got.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        for &(a, b) in &edges {
            let pa = *pos.get(&names[a]).unwrap();
            let pb = *pos.get(&names[b]).unwrap();
            assert!(pa < pb, "edge {} -> {}", names[a], names[b]);
        }
    }
}

#[test]
fn fuzz_output_is_a_permutation_of_input_tokens() {
    let mut rng = t::Rng::new(0x77);
    for _ in 0..100 {
        let n = 2 + rng.below(10) as usize;
        let mut lines = String::new();
        let mut seen: Vec<String> = Vec::new();
        for _ in 0..n {
            let a = format!("t{}", rng.below(50));
            let b = format!("t{}", rng.below(50));
            if !seen.contains(&a) {
                seen.push(a.clone());
            }
            if !seen.contains(&b) {
                seen.push(b.clone());
            }
            lines.push_str(&format!("{a} {b}\n"));
        }
        let r = t::run("tsort", &[], &lines);
        if r.code != 0 {
            continue;
        }
        let mut got: Vec<String> = r.out().lines().map(|s| s.to_string()).collect();
        got.sort();
        let mut want = seen.clone();
        want.sort();
        assert_eq!(got, want);
    }
}
