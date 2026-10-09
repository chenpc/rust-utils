use coreutils::testutil as t;

#[test]
fn leading_blanks_become_tabs() {
    let r = t::run("unexpand", &[], "        a\n");
    assert_eq!(r.out(), "\ta\n");
}

#[test]
fn run_longer_than_a_stop_leaves_remainder() {
    let r = t::run("unexpand", &[], "         a\n");
    assert_eq!(r.out(), "\t a\n");
}

#[test]
fn short_run_is_not_converted() {
    let r = t::run("unexpand", &[], "   a\n");
    assert_eq!(r.out(), "   a\n");
}

#[test]
fn all_flag_converts_interior_blanks_at_stops() {
    let r = t::run("unexpand", &["-a", "-t", "4"], "a   b\n");
    assert_eq!(r.out(), "a\tb\n");
}

#[test]
fn first_only_overrides_all() {
    let r = t::run("unexpand", &["-a", "-f", "-t", "4"], "a   b\n");
    assert_eq!(r.out(), "a   b\n");
}

#[test]
fn tabs_option_enables_all() {
    let r = t::run("unexpand", &["-t", "4"], "a   b\n");
    assert_eq!(r.out(), "a\tb\n");
}

#[test]
fn multiple_stops_from_list() {
    let r = t::run("unexpand", &["-a", "-t", "3,6"], "aa  x\n");
    assert_eq!(r.out(), "aa\t x\n");
}

#[test]
fn existing_tabs_are_kept() {
    let r = t::run("unexpand", &["-a", "-t", "4"], "\ta\n");
    assert_eq!(r.out(), "\ta\n");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("unexpand", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("unexpand: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("unexpand", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("unexpand"));
    let v = t::run("unexpand", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("unexpand"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("unexpand", &["-a", "-t", "4"], "a   b\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a\tb\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("unexpand", &["-a", "-t", "4"], b"a   b\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_expand_roundtrip() {
    let mut rng = t::Rng::new(0xF0);
    for _ in 0..100 {
        let width = 1 + rng.below(12) as usize;
        let parts: Vec<String> = (0..1 + rng.below(4))
            .map(|_| {
                let len = rng.below(6) as usize;
                (0..len)
                    .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                    .collect()
            })
            .collect();
        let line = parts.join(&" ".repeat(1 + rng.below(12) as usize)) + "\n";
        let u = t::run("unexpand", &["-a", "-t", &width.to_string()], &line);
        assert_eq!(u.code, 0);
        let o = u.out();
        let back = t::run("expand", &["-t", &width.to_string()], &o);
        assert_eq!(back.code, 0);
        assert_eq!(back.out(), line, "line={line} width={width}");
    }
}
