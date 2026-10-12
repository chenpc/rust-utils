use coreutils::testutil as t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn fresh(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/nh_{}_{}", tag, std::process::id()));
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

/// Copy the multi-call binary into `dir` under `name` so argv[0] dispatch works.
/// The guest binary is ~29 MB, so helper commands are symlinks to it (argv[0]
/// basename selects the applet) instead of per-test copies.
fn copy_as(dir: &PathBuf, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::os::unix::fs::symlink(binary(), &p).unwrap();
    p
}

#[test]
fn runs_command_and_reports_no_output() {
    let d = fresh("run");
    let tr = copy_as(&d, "truncate");
    let r = t::run(
        "nohup",
        &[&tr.to_string_lossy(), "-s", "7", &d.join("f").to_string_lossy()],
        "",
    );
    assert_eq!(r.code, 0);
    // In the guest the test binary's stdout is the serial console (a TTY), so
    // nohup legitimately reports the redirect; with pipes it stays silent.
    assert!(
        r.err().is_empty() || r.err().starts_with("nohup: ignoring input"),
        "err {}",
        r.err()
    );
    assert_eq!(fs::metadata(&d.join("f")).unwrap().len(), 7);
}

#[test]
fn child_stdout_goes_to_the_inherited_stream() {
    let d = fresh("stdout");
    let sh = copy_as(&d, "sh");
    let out = d.join("o");
    let r = t::run(
        "nohup",
        &[
            &sh.to_string_lossy(),
            "-c",
            &format!("echo hello > {}", out.display()),
        ],
        "",
    );
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_to_string(&out).unwrap(), "hello\n");
}

#[test]
fn exit_code_of_the_command_is_returned() {
    let d = fresh("rc");
    let sh = copy_as(&d, "sh");
    let r = t::run("nohup", &[&sh.to_string_lossy(), "-c", "exit 41"], "");
    assert_eq!(r.code, 41);
}

#[test]
fn missing_command_is_127() {
    let r = t::run("nohup", &["/nonexistent/nothere"], "");
    assert_eq!(r.code, 127);
    assert_eq!(
        r.err(),
        "nohup: failed to run command '/nonexistent/nothere': No such file or directory\n"
    );
}

#[test]
fn unexecutable_command_is_126() {
    let d = fresh("noexec");
    let p = d.join("plain");
    fs::write(&p, "not a program\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
    let r = t::run("nohup", &[&p.to_string_lossy()], "");
    assert_eq!(r.code, 126);
    assert_eq!(
        r.err(),
        format!(
            "nohup: failed to run command '{}': Permission denied\n",
            p.display()
        )
    );
}

#[test]
fn relative_names_are_resolved_through_path() {
    let r = t::run("nohup", &["coreutils-missing-program-xyz"], "");
    assert_eq!(r.code, 127);
    assert!(r.err().starts_with("nohup: failed to run command '"));
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("nohup", &["--help"], "");
    assert_eq!(r.code, 0);
    let first = r.out().lines().next().unwrap().to_string();
    assert_eq!(first, "Run COMMAND ignoring hangup signals.");
    assert!(r.out().contains("\nUsage: nohup COMMAND [ARG]...\n       nohup OPTION\n"));
    assert!(r.out().contains("  -h, --help     Print help\n"));
    assert!(r.out().contains("  -V, --version  Print version\n"));

    let v = t::run("nohup", &["--version"], "");
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "nohup (uutils coreutils) 0.10.0\n");
    let h = t::run("nohup", &["-h"], "");
    assert_eq!(h.out(), r.out());
    let hv = t::run("nohup", &["-hV"], "");
    assert_eq!(hv.out(), r.out());
    let vh = t::run("nohup", &["-Vh"], "");
    assert_eq!(vh.out(), "nohup (uutils coreutils) 0.10.0\n");
}

#[test]
fn unknown_option_matches_clap_structure() {
    let r = t::run("nohup", &["--bogus"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: nohup COMMAND [ARG]...\n       nohup OPTION\n\nFor more information, try '--help'.\n"
    );
    let s = t::run("nohup", &["-x"], "");
    assert_eq!(s.code, 125);
    assert!(s.err().contains("error: unexpected argument '-x' found\n"));
}

#[test]
fn missing_command_name_is_required_argument() {
    let r = t::run("nohup", &[], "");
    assert_eq!(r.code, 125);
    assert!(r.err().starts_with("error: the following required arguments were not provided:\n"));
}

#[test]
fn double_dash_keeps_the_next_token_as_the_command() {
    let r = t::run("nohup", &["--", "--bogus"], "");
    assert_eq!(r.code, 127);
    assert_eq!(
        r.err(),
        "nohup: failed to run command '--bogus': No such file or directory\n"
    );
}

#[test]
fn options_after_the_command_reach_the_command() {
    let d = fresh("after");
    let tr = copy_as(&d, "truncate");
    let f = d.join("f");
    fs::write(&f, b"0123456789").unwrap();
    // The flags belong to the command, not to nohup.
    let r = t::run(
        "nohup",
        &[&tr.to_string_lossy(), "-s", "5", &f.to_string_lossy()],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::metadata(&f).unwrap().len(), 5);
}

#[test]
fn fuzz_argument_permutations() {
    let d = fresh("fuzz");
    let tr = copy_as(&d, "truncate");
    let mut rng = t::Rng::new(20261130);
    let flags = ["-h", "-V", "-x", "--help", "--version", "--bogus", "--"];
    for case in 0..12 {
        let _ = case;
        let mut args: Vec<String> = Vec::new();
        let count = 1 + (rng.below(3) as usize);
        for _ in 0..count {
            args.push(flags[rng.below(flags.len() as u64) as usize].to_string());
        }
        if rng.below(2) == 0 {
            args.push(tr.to_string_lossy().to_string());
            args.push("-s".to_string());
            args.push("1".to_string());
        }
        let refs: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
        let r = t::run("nohup", &refs, "");
        let as_str: Vec<&str> = refs.iter().map(|s| *s).collect();
        let expect_help = as_str.iter().any(|a| *a == "-h" || *a == "--help");
        let expect_version = as_str.iter().any(|a| *a == "-V" || *a == "--version");
        if !expect_help && !expect_version {
            let unknown = as_str
                .iter()
                .any(|a| a.starts_with('-') && *a != "--" && !matches!(*a, "-h" | "-V"));
            if unknown {
                assert_eq!(r.code, 125, "args {:?}", as_str);
            } else {
                assert!(r.code == 0 || r.code == 127, "args {:?}", as_str);
            }
        }
    }
}

#[test]
fn e2e_through_the_real_binary() {
    let d = fresh("e2e");
    let sh = copy_as(&d, "sh");
    let out = d.join("o");
    let r = t::run_bin(
        "nohup",
        &[
            &sh.to_string_lossy(),
            "-c",
            &format!("echo via-exec > {}", out.display()),
        ],
        "",
    );
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_to_string(&out).unwrap(), "via-exec\n");
}

#[test]
fn output_falls_back_to_home_when_local_nohup_out_is_unusable() {
    let d = fresh("home");
    let cwd = d.join("cwd");
    let home = d.join("home");
    fs::create_dir(&cwd).unwrap();
    fs::create_dir(&home).unwrap();
    // ./nohup.out is a directory: opening it for append must fail (EISDIR).
    fs::create_dir(cwd.join("nohup.out")).unwrap();
    let sh = copy_as(&d, "sh");
    let prev_cwd = std::env::current_dir().unwrap();
    let prev_home = std::env::var("HOME").ok();
    std::env::set_current_dir(&cwd).unwrap();
    std::env::set_var("HOME", &home);
    let r = t::run("nohup", &[&sh.to_string_lossy(), "-c", "echo hi"], "");
    let _ = std::env::set_current_dir(&prev_cwd);
    match prev_home {
        Some(h) => std::env::set_var("HOME", h),
        None => std::env::remove_var("HOME"),
    }
    assert_eq!(r.code, 0, "err {}", r.err());
    let landed = home.join("nohup.out");
    assert_eq!(fs::read_to_string(&landed).unwrap(), "hi\n");
    assert!(r.err().contains("nohup.out"), "err {}", r.err());
}
