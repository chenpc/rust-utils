use coreutils::testutil::*;

fn collect(name: &str, args: &[&str], limit: usize) -> (i32, Vec<u8>) {
    let mut w = FailWriter::new(limit);
    let mut e = Vec::new();
    let code = run_with(name, args, &[], &mut w, &mut e);
    (code, w.out)
}

#[test]
fn default_string_is_y() {
    let (code, out) = collect("yes", &[], 10);
    assert_eq!(code, 1, "write error must be reported as failure");
    assert_eq!(String::from_utf8_lossy(&out), "y\ny\ny\ny\ny\n");
}

#[test]
fn specified_string_is_repeated() {
    let (_, out) = collect("yes", &["abc"], 12);
    assert_eq!(String::from_utf8_lossy(&out), "abc\nabc\nabc\n");
}

#[test]
fn multiple_strings_are_joined_by_space() {
    let (_, out) = collect("yes", &["a", "b"], 8);
    assert_eq!(String::from_utf8_lossy(&out), "a b\na b\n");
}

#[test]
fn empty_string_repeats_newline() {
    let (_, out) = collect("yes", &[""], 3);
    assert_eq!(String::from_utf8_lossy(&out), "\n\n\n");
}

#[test]
fn help_and_version_stop_the_loop() {
    let h = run("yes", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("yes"), "help must name the applet: {}", h.out());
    let v = run("yes", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("yes"), "version must name the applet: {}", v.out());
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin_limit("yes", &["xy"], 12);
    assert_eq!(String::from_utf8_lossy(&r.stdout), "xy\nxy\nxy\nxy\n");
}

#[test]
fn fuzz_output_is_exact_repetition_of_one_line() {
    // man page 不變量：輸出 = (STRING + newline) 的無限重複；被截斷後仍是同一模式的 prefix
    let mut rng = Rng::new(0x9E55);
    for _ in 0..200 {
        let n = rng.below(4) as usize;
        let mut parts: Vec<String> = Vec::new();
        for _ in 0..n {
            let len = 1 + rng.below(10) as usize;
            let bytes = rng.bytes(len);
            let s = String::from_utf8_lossy(&bytes).into_owned();
            if s.starts_with('-') {
                continue;
            }
            parts.push(s);
        }
        let pattern = if parts.is_empty() {
            "y\n".to_string()
        } else {
            parts.join(" ") + "\n"
        };
        let limit = 1 + rng.below(64) as usize;
        let args: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        let (_, out) = collect("yes", &args, limit);
        assert_eq!(out.len(), limit, "writer limit must be exactly consumed");
        for (i, &b) in out.iter().enumerate() {
            assert_eq!(b, pattern.as_bytes()[i % pattern.len()], "pattern={pattern:?} out={out:?}");
        }
    }
}
