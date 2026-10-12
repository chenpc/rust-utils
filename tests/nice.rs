use coreutils::testutil as t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn fresh(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/ni_{}_{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn binary() -> PathBuf {
    if let Ok(p) = std::env::var("COREUTILS_BIN") {
        return PathBuf::from(p);
    }
    PathBuf::from("/mnt/bin/coreutils")
}

/// The guest binary is ~29 MB, so helper commands are symlinks to it (argv[0]
/// basename selects the applet) instead of per-test copies.
fn copy_as(dir: &PathBuf, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::os::unix::fs::symlink(binary(), &p).unwrap();
    p
}

#[test]
fn prints_current_niceness_without_operands() {
    let r = t::run("nice", &[], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    let value: i64 = out.trim().parse().expect(&format!("not a number: {:?}", out));
    assert!(value >= -20 && value <= 20, "out {:?}", out);
    assert_eq!(r.err(), "");
}

#[test]
fn adjustment_without_command_is_rejected() {
    for args in [vec!["-n", "5"], vec!["-5"], vec!["--adjustment", "3"]] {
        let r = t::run("nice", &args, "");
        assert_eq!(r.code, 125, "args {:?}", args);
        assert!(r.err().starts_with("nice: A command must be given with an adjustment.\n"));
        assert!(r.err().contains("--help"));
    }
}

#[test]
fn runs_the_command_with_the_adjustment() {
    let d = fresh("run");
    let sh = copy_as(&d, "sh");
    let niced = copy_as(&d, "nice");
    let out = d.join("o");
    let base = current_niceness();
    let r = t::run(
        "nice",
        &[
            "-n",
            "5",
            &sh.to_string_lossy(),
            "-c",
            &format!("{} > {}", niced.display(), out.display()),
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    let got: i64 = fs::read_to_string(&out).unwrap().trim().parse().unwrap();
    assert_eq!(got, base + 5);
}

fn current_niceness() -> i64 {
    let r = t::run("nice", &[], "");
    r.out().trim().parse().unwrap_or(0)
}

#[test]
fn old_style_adjustment_is_accepted() {
    let d = fresh("old");
    let tr = copy_as(&d, "truncate");
    let r = t::run(
        "nice",
        &[
            "-10",
            &tr.to_string_lossy(),
            "-s",
            "4",
            &d.join("f").to_string_lossy(),
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::metadata(&d.join("f")).unwrap().len(), 4);
}

#[test]
fn invalid_adjustment_is_reported() {
    // The missing-command check comes first, before the adjustment is parsed.
    let r = t::run("nice", &["-n", "abc"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().starts_with("nice: A command must be given with an adjustment."));
    assert!(r.err().contains("--help"));

    let r = t::run("nice", &["-n", "abc", "/bin/true"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "nice: \"abc\" is not a valid number: invalid digit found in string\n"
    );
}

#[test]
fn missing_adjustment_value_matches_clap_wording() {
    let r = t::run("nice", &["-n"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().contains("a value is required for '--adjustment <adjustment>'"));
}

#[test]
fn missing_command_is_127_and_unexecutable_is_126() {
    let r = t::run("nice", &["nosuchprogram-xyz"], "");
    assert_eq!(r.code, 127);
    assert!(r.err().starts_with("nice: nosuchprogram-xyz: "));

    let d = fresh("noexec");
    let p = d.join("plain");
    fs::write(&p, "not a program\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
    let s = t::run("nice", &[&p.to_string_lossy()], "");
    assert_eq!(s.code, 126);
    assert!(s.err().starts_with("nice: "));
    assert!(s.err().contains("Permission denied"));
}

#[test]
fn command_exit_code_is_propagated() {
    let d = fresh("rc");
    let sh = copy_as(&d, "sh");
    let r = t::run("nice", &["-n", "1", &sh.to_string_lossy(), "-c", "exit 20"], "");
    assert_eq!(r.code, 20);
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("nice", &["--help"], "");
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out().lines().next().unwrap(),
        "Run COMMAND with an adjusted niceness, which affects process scheduling."
    );
    assert!(r.out().contains("\nUsage: nice [OPTION] [COMMAND [ARG]...]\n"));
    assert!(r
        .out()
        .contains("  -n, --adjustment <adjustment>  add N to the niceness (default is 10)\n"));
    assert!(r.out().contains("  -h, --help                     Print help\n"));
    assert!(r.out().contains("  -V, --version                  Print version\n"));

    let v = t::run("nice", &["--version"], "");
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "nice (uutils coreutils) 0.10.0\n");
    let hv = t::run("nice", &["-Vh"], "");
    assert_eq!(hv.out(), "nice (uutils coreutils) 0.10.0\n");
    let hv2 = t::run("nice", &["-hV"], "");
    assert_eq!(hv2.out(), r.out());
}

#[test]
fn unknown_option_matches_clap_structure() {
    let r = t::run("nice", &["--bogus"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: nice [OPTION] [COMMAND [ARG]...]\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn fuzz_flag_sequences() {
    let d = fresh("fuzz");
    let tr = copy_as(&d, "truncate");
    let mut rng = t::Rng::new(20261201);
    let flags = ["-n", "-x", "--help", "--version", "--bogus", "-5", "+3", "--"];
    for _case in 0..12 {
        let mut args: Vec<String> = Vec::new();
        let count = 1 + (rng.below(3) as usize);
        for _ in 0..count {
            args.push(flags[rng.below(flags.len() as u64) as usize].to_string());
        }
        if rng.below(2) == 0 {
            args.push(tr.to_string_lossy().to_string());
        }
        let refs: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
        // -n consumes the following token, so --help after -n is a value, not help.
        let mut first_bad = usize::MAX;
        let mut wins_help = false;
        let mut wins_ver = false;
        let mut i = 0;
        let mut consumed_next = false;
        while i < refs.len() {
            let a = refs[i];
            if consumed_next {
                consumed_next = false;
                i += 1;
                continue;
            }
            if a == "-x" || a == "--bogus" {
                first_bad = i;
                break;
            }
            if a == "-n" {
                consumed_next = true;
            } else if a == "-h" || a == "--help" {
                wins_help = true;
                break;
            } else if a == "-V" || a == "--version" {
                wins_ver = true;
                break;
            }
            i += 1;
        }
        let _ = first_bad;
        let r = t::run("nice", &refs, "");
        if wins_help {
            assert_eq!(r.code, 0, "args {:?}", refs);
            assert!(r.out().starts_with("Run COMMAND with an adjusted niceness"));
        } else if wins_ver {
            assert_eq!(r.out(), "nice (uutils coreutils) 0.10.0\n", "args {:?}", refs);
        } else {
            assert!(r.code == 0 || r.code == 125 || r.code == 126 || r.code == 127, "args {:?}", refs);
        }
    }
}

#[test]
fn e2e_through_the_real_binary() {
    let d = fresh("e2e");
    let tr = copy_as(&d, "truncate");
    let r = t::run_bin(
        "nice",
        &[
            "-n",
            "0",
            &tr.to_string_lossy(),
            "-s",
            "6",
            &d.join("f").to_string_lossy(),
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::metadata(&d.join("f")).unwrap().len(), 6);
}
