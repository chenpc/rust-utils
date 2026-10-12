use coreutils::testutil as t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn fresh(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/to_{}_{}", tag, std::process::id()));
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
fn command_that_finishes_in_time_returns_its_status() {
    let d = fresh("ok");
    let tr = copy_as(&d, "truncate");
    let r = t::run(
        "timeout",
        &[
            "5",
            &tr.to_string_lossy(),
            "-s",
            "3",
            &d.join("f").to_string_lossy(),
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::metadata(&d.join("f")).unwrap().len(), 3);
}

#[test]
fn exit_code_of_the_command_is_propagated() {
    let d = fresh("rc");
    let sh = copy_as(&d, "sh");
    let r = t::run("timeout", &["5", &sh.to_string_lossy(), "-c", "exit 33"], "");
    assert_eq!(r.code, 33);
}

#[test]
fn timeout_default_exit_is_124() {
    let d = fresh("t124");
    let sl = copy_as(&d, "sleep");
    let r = t::run("timeout", &["0.2", &sl.to_string_lossy(), "5"], "");
    assert_eq!(r.code, 124, "err {}", r.err());
    assert_eq!(r.err(), "");
}

#[test]
fn verbose_reports_the_signal_sent() {
    let d = fresh("verb");
    let sl = copy_as(&d, "sleep");
    let slp = sl.to_string_lossy().to_string();
    let r = t::run("timeout", &["-v", "0.2", &slp, "5"], "");
    assert_eq!(r.code, 124);
    assert_eq!(
        r.err(),
        format!("timeout: sending signal TERM to command '{}'\n", slp)
    );
}

#[test]
fn preserve_status_returns_128_plus_signal() {
    let d = fresh("pres");
    let sl = copy_as(&d, "sleep");
    let r = t::run("timeout", &["-p", "0.2", &sl.to_string_lossy(), "5"], "");
    assert_eq!(r.code, 143, "err {}", r.err());
}

#[test]
fn kill_after_escalates_to_kill_with_137() {
    let d = fresh("kill");
    let sl = copy_as(&d, "sleep");
    let slp = sl.to_string_lossy().to_string();
    // SIGSTOP parks the child, so TERM alone is not enough and KILL is used.
    let r = t::run("timeout", &["-k", "0.1", "-s", "STOP", "-v", "0.2", &slp, "30"], "");
    assert_eq!(r.code, 137, "err {}", r.err());
    let err = r.err();
    assert!(err.contains("sending signal KILL to command"), "err {}", err);
}

#[test]
fn custom_signal_is_used() {
    let d = fresh("sig");
    let sl = copy_as(&d, "sleep");
    let r = t::run(
        "timeout",
        &["-s", "INT", "-p", "-v", "0.2", &sl.to_string_lossy(), "30"],
        "",
    );
    assert!(r.err().contains("sending signal INT to command"), "err {}", r.err());
    assert!(r.code == 130 || r.code == 124 || r.code == 137, "rc {}", r.code);
}

#[test]
fn foreground_signals_only_the_command() {
    let d = fresh("fg");
    let sl = copy_as(&d, "sleep");
    let r = t::run("timeout", &["--foreground", "0.2", &sl.to_string_lossy(), "30"], "");
    assert_eq!(r.code, 124, "err {}", r.err());
}

#[test]
fn invalid_duration_is_125() {
    let d = fresh("baddur");
    let tr = copy_as(&d, "truncate");
    let r = t::run("timeout", &["abc", &tr.to_string_lossy()], "");
    assert_eq!(r.code, 125);
    assert!(r.err().starts_with("timeout: invalid time interval 'abc'\n"));
    assert!(r.err().contains("--help"));
}

#[test]
fn invalid_kill_after_is_125() {
    let r = t::run("timeout", &["-k", "zz", "1", "true"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().starts_with("timeout: invalid time interval 'zz'\n"));
}

#[test]
fn invalid_signal_is_125() {
    let r = t::run("timeout", &["-s", "bogus", "1", "true"], "");
    assert_eq!(r.code, 125);
    assert_eq!(r.err().lines().next().unwrap(), "timeout: 'bogus': invalid signal");
}

#[test]
fn missing_duration_and_command_match_required_arguments() {
    let r = t::run("timeout", &[], "");
    assert_eq!(r.code, 125);
    let err = r.err();
    assert!(err.starts_with("error: the following required arguments were not provided:\n"));
    assert!(err.contains("  <duration>\n"));
    assert!(err.contains("  <command>...\n"));

    let s = t::run("timeout", &["1"], "");
    assert_eq!(s.code, 125);
    assert!(s.err().contains("  <command>...\n"));
}

#[test]
fn missing_option_values_match_clap_wording() {
    let r = t::run("timeout", &["-k"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().contains("a value is required for '--kill-after <kill-after>'"));
    let s = t::run("timeout", &["--signal"], "");
    assert!(s.err().contains("a value is required for '--signal <SIGNAL>'"));
}

#[test]
fn unknown_option_matches_clap_structure() {
    let r = t::run("timeout", &["--bogus"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: timeout [OPTION] DURATION COMMAND...\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn missing_program_is_127_or_126() {
    let r = t::run("timeout", &["1", "nosuchprogram-xyz"], "");
    assert_eq!(r.code, 127);
    assert_eq!(
        r.err(),
        "timeout: failed to execute process: No such file or directory (os error 2)\n"
    );

    let d = fresh("noexec");
    let p = d.join("plain");
    fs::write(&p, "not a program\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
    let s = t::run("timeout", &["1", &p.to_string_lossy()], "");
    assert_eq!(s.code, 126);
    assert!(s.err().contains("Permission denied"));
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("timeout", &["--help"], "");
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out().lines().next().unwrap(),
        "Start COMMAND, and kill it if still running after DURATION."
    );
    assert!(r
        .out()
        .contains("\nUsage: timeout [OPTION] DURATION COMMAND...\n"));
    assert!(r
        .out()
        .contains("  -k, --kill-after <kill-after>  also send a KILL signal if COMMAND is still running this long after"));
    assert!(r
        .out()
        .contains("  -s, --signal <SIGNAL>          specify the signal to be sent on timeout"));
    assert!(r
        .out()
        .contains("  -p, --preserve-status          exit with the same status as COMMAND, even when the command times"));
    assert!(r.out().contains("  -h, --help                     Print help"));
    let v = t::run("timeout", &["--version"], "");
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "timeout (uutils coreutils) 0.10.0\n");
    let hv = t::run("timeout", &["-Vh"], "");
    assert_eq!(hv.out(), "timeout (uutils coreutils) 0.10.0\n");
    let vh = t::run("timeout", &["-hV"], "");
    assert_eq!(vh.out(), r.out());
}

#[test]
fn fuzz_duration_and_flag_combinations() {
    let d = fresh("fuzz");
    let tr = copy_as(&d, "truncate");
    let mut rng = t::Rng::new(20261202);
    let durations = ["1", "0.1", "0", "abc", "-1", "1s"];
    let flags = ["-v", "-p", "-f", "--foreground", "-x", "--bogus", "--"];
    for _case in 0..12 {
        let mut args: Vec<String> = Vec::new();
        args.push(flags[rng.below(flags.len() as u64) as usize].to_string());
        args.push(durations[rng.below(durations.len() as u64) as usize].to_string());
        if rng.below(2) == 0 {
            args.push(tr.to_string_lossy().to_string());
            args.push("-s".to_string());
            args.push("1".to_string());
        }
        let refs: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
        let r = t::run("timeout", &refs, "");
        assert!(
            matches!(r.code, 0 | 124 | 125 | 126 | 127),
            "args {:?} rc {}",
            refs,
            r.code
        );
    }
}

#[test]
fn e2e_through_the_real_binary() {
    let d = fresh("e2e");
    let tr = copy_as(&d, "truncate");
    let r = t::run_bin(
        "timeout",
        &[
            "5",
            &tr.to_string_lossy(),
            "-s",
            "2",
            &d.join("f").to_string_lossy(),
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::metadata(&d.join("f")).unwrap().len(), 2);
}
