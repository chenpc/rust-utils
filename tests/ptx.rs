use coreutils::testutil as t;

fn lines(s: &str) -> Vec<String> {
    s.lines().map(|l| l.to_string()).collect()
}

#[test]
fn permuted_index_is_sorted() {
    let r = t::run("ptx", &[], "alpha beta gamma\n");
    let out = lines(&r.out());
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], "alpha   beta gamma");
    assert_eq!(out[1], "beta   alpha /gamma");
    assert_eq!(out[2], "gamma   alpha beta");
    let keys: Vec<String> = out.iter().map(|l| l.split(" ").next().unwrap().to_string()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn ignore_case_sorts_folded() {
    let r = t::run("ptx", &["-f"], "Zebra apple\n");
    let out = lines(&r.out());
    assert_eq!(out[0].split(" ").next().unwrap(), "apple");
    assert_eq!(out[1].split(" ").next().unwrap(), "Zebra");
    let plain = t::run("ptx", &[], "Zebra apple\n");
    assert_eq!(plain.out().lines().next().unwrap().split(' ').next().unwrap(), "Zebra");
}

#[test]
fn gap_size_changes_columns() {
    let r = t::run("ptx", &["-g", "1"], "a b\n");
    assert_eq!(r.out(), "a b\nb a\n");
    let r = t::run("ptx", &["-g", "5"], "a b\n");
    assert_eq!(r.out(), "a     b\nb     a\n");
}

#[test]
fn width_truncation_marks_with_flag() {
    let r = t::run("ptx", &["-w", "12", "-g", "1"], "alpha beta gamma delta\n");
    for l in lines(&r.out()) {
        let key = l.split(' ').next().unwrap();
        assert!(l.len() <= 12 + key.len(), "line too long: {}", l);
    }
    let one = t::run("ptx", &["-w", "8", "-g", "1"], "aaaa bbbb\n");
    // kw(4) + gap(1) leaves 3 columns for the context -> right side truncated
    assert_eq!(one.out().lines().next().unwrap(), "aaaa bb/");
}

#[test]
fn flag_string_is_used_for_truncation() {
    let r = t::run("ptx", &["-F", "!", "-g", "1", "-w", "8"], "aaaa bbbb\n");
    assert_eq!(r.out().lines().next().unwrap(), "aaaa bb!");
}

#[test]
fn references_are_the_first_field() {
    let r = t::run("ptx", &["-r", "-g", "1"], "ref one two\n");
    let out = lines(&r.out());
    assert_eq!(out[0], "ref one two");
    assert_eq!(out[1], "ref two one");
}

#[test]
fn auto_reference_is_line_number() {
    let r = t::run("ptx", &["-A", "-g", "1"], "a b\nc d\n");
    let out = lines(&r.out());
    assert_eq!(out[0], "1 a b");
    assert_eq!(out[1], "1 b a");
    assert_eq!(out[2], "2 c d");
    assert_eq!(out[3], "2 d c");
}

#[test]
fn right_side_references_are_not_counted_in_width() {
    let r = t::run("ptx", &["-R", "-r", "-g", "1", "-w", "12"], "ref kw ctx\n");
    for l in lines(&r.out()) {
        let last = l.split(' ').filter(|p| !p.is_empty()).last().unwrap();
        assert_eq!(last, "ref", "reference must be at the right: {}", l);
    }
}

#[test]
fn ignore_and_only_files() {
    let wd = t::workdir();
    std::fs::write(wd.join("ignore.txt"), "stop\n").unwrap();
    std::fs::write(wd.join("only.txt"), "keep\n").unwrap();
    let r = t::run("ptx", &["-i", wd.join("ignore.txt").to_str().unwrap(), "-g", "1"], "stop go\n");
    assert_eq!(r.out(), "go stop\n");
    let r = t::run("ptx", &["-o", wd.join("only.txt").to_str().unwrap(), "-g", "1"], "keep drop\n");
    assert_eq!(r.out(), "keep drop\n");
}

#[test]
fn break_file_changes_word_boundaries() {
    let wd = t::workdir();
    std::fs::write(wd.join("break.txt"), ",").unwrap();
    let r = t::run("ptx", &["-b", wd.join("break.txt").to_str().unwrap(), "-g", "1"], "a,b\n");
    assert_eq!(r.out(), "a b\nb a\n");
}

#[test]
fn roff_and_tex_directives_use_macro_name() {
    let r = t::run("ptx", &["-O", "-g", "1"], "a b\n");
    assert_eq!(r.out(), ".xx  a b\n.xx  b a\n");
    let r = t::run("ptx", &["-T", "-M", "kw", "-g", "1"], "a b\n");
    assert_eq!(r.out(), "\\kw{}{a}{b}\n\\kw{}{b}{a}\n");
}

#[test]
fn typeset_mode_widens_to_100() {
    let r = t::run("ptx", &["-t", "-g", "1"], "a b\n");
    assert_eq!(r.out(), "a b\nb a\n");
}

#[test]
fn missing_operand_and_unknown_option() {
    assert_eq!(t::run("ptx", &["-w"], "").code, 2);
    assert_eq!(t::run("ptx", &["-Z"], "").code, 2);
    assert_eq!(t::run("ptx", &["-w", "abc"], "").code, 2);
    assert_eq!(t::run("ptx", &["-g", "x"], "").code, 2);
}

#[test]
fn missing_file_is_error() {
    let r = t::run("ptx", &["nope.txt"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("nope.txt"));
}

#[test]
fn help_and_version() {
    assert!(t::run("ptx", &["-h"], "").out().contains("ptx"));
    assert!(t::run("ptx", &["-V"], "").out().contains("ptx"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("ptx", &["-g", "1"], "a b\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a b\nb a\n");
}

#[test]
fn fuzz_keyword_and_order_invariants() {
    for seed in 0..120u64 {
        let mut rng = t::Rng::new(seed);
        let nwords = (rng.below(6) + 1) as usize;
        let mut words: Vec<String> = Vec::new();
        for _ in 0..nwords {
            let len = (rng.below(4) + 1) as usize;
            let bs = t::Rng::new(seed + len as u64).bytes(len);
            let w: String = bs
                .iter()
                .filter(|&b| *b >= 0x21 && *b <= 0x7e && *b != b'/' && *b != b' ')
                .map(|&b| b as char)
                .collect();
            if !w.is_empty() {
                words.push(w);
            }
        }
        if words.is_empty() {
            continue;
        }
        let input = words.join(" ") + "\n";
        let r = t::run("ptx", &["-g", "1"], &input);
        assert_eq!(r.code, 0, "seed {}", seed);
        let out = lines(&r.out());
        assert_eq!(out.len(), words.len(), "seed {}: one line per keyword", seed);
        // every keyword appears as the first field
        let keys: Vec<String> = out.iter().map(|l| l.split(' ').next().unwrap().to_string()).collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys, "seed {}: output must be sorted", seed);
        let mut expected = words.clone();
        expected.sort();
        assert_eq!(sorted_keys, expected, "seed {}: keyword set", seed);
        // context must contain the other words (possibly truncated with the flag)
        for (i, l) in out.iter().enumerate() {
            let kw = keys[i].clone();
            let rest = &l[kw.len() + 1..];
            for w in &words {
                if w != &kw && !rest.contains(w) && !rest.contains("/") {
                    panic!("seed {}: context missing word {} in {}", seed, w, l);
                }
            }
        }
    }
}