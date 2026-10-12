//! chroot e2e tests. The guest runs as root, so chroot(2) succeeds there;
//! on the host the same paths are reported as EPERM.

use coreutils::testutil as t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn fresh(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/ch_{}_{}", tag, std::process::id()));
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

/// A single reusable new root on the 9p share (/out is host-backed, so the
/// 29 MB instrumented binary fits there; the guest tmpfs is too small).
fn test_root() -> PathBuf {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    // tmpfs (not 9p): chroot(2)+chdir("/") behaves normally there, and a hard
    // link to the initramfs copy costs no space (the guest only has 128 MB).
    let dir = PathBuf::from("/tmp/e4_root");
    ONCE.call_once(|| {
        let _ = fs::remove_dir_all(&dir);
        let bin = dir.join("bin/coreutils");
        fs::create_dir_all(bin.parent().unwrap()).unwrap();
        // The initramfs carries the same multi-call binary as /init; a hard link
        // to it stays on the same tmpfs and costs no space.
        if fs::hard_link("/init", &bin).is_err() && fs::hard_link(&binary(), &bin).is_err() {
            fs::copy(&binary(), &bin).unwrap();
        }
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    });
    dir
}

#[test]
fn chroots_into_the_new_root_and_runs_the_command() {
    let root = test_root();
    let out = root.join("o");
    let _ = fs::remove_file(&out);
    // The command writes to /o, which after chroot(2) is inside the new root.
    let r = t::run(
        "chroot",
        &[&root.to_string_lossy(), "/bin/coreutils", "sh", "-c", "echo inside > /o"],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(&out).unwrap(), "inside\n");
}

#[test]
fn skip_chdir_accepts_old_root_and_leaves_the_working_directory() {
    let d = fresh("skip");
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&d).unwrap();
    let r = t::run(
        "chroot",
        &["--skip-chdir", "/", &binary().to_string_lossy(), "sh", "-c", "pwd > o"],
        "",
    );
    std::env::set_current_dir(&old).unwrap();
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(d.join("o")).unwrap().trim(), d.to_string_lossy());
}

#[test]
fn skip_chdir_is_refused_when_new_root_is_not_the_old_root() {
    let d = fresh("skipbad");
    let r = t::run("chroot", &["--skip-chdir", &d.to_string_lossy(), "true"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "chroot: option --skip-chdir only permitted if NEWROOT is old '/'"
    );
}

#[test]
fn missing_newroot_matches_required_arguments() {
    let r = t::run("chroot", &[], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "error: the following required arguments were not provided:\n  <newroot>\n\nUsage: chroot [OPTION]... NEWROOT [COMMAND [ARG]...]\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn nonexistent_or_non_directory_newroot_is_reported() {
    let r = t::run("chroot", &["/nonexistent_root_dir_xyz", "true"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "chroot: cannot change root directory to '/nonexistent_root_dir_xyz': no such directory\n"
    );

    let d = fresh("file");
    let p = d.join("f");
    fs::write(&p, "not a directory\n").unwrap();
    let s = t::run("chroot", &[&p.to_string_lossy(), "true"], "");
    assert_eq!(s.code, 125);
    assert!(s.err().contains("no such directory"));
}

#[test]
fn invalid_userspec_and_groups_are_rejected_before_any_syscall() {
    let root = test_root().to_string_lossy().to_string();

    let r = t::run("chroot", &["--userspec=nosuchuser_xyz", &root, "true"], "");
    assert_eq!(r.code, 125);
    assert_eq!(r.err(), "chroot: invalid user\n");

    let g = t::run("chroot", &["--userspec=0:nosuchgroup_xyz", &root, "true"], "");
    assert_eq!(g.code, 125);
    assert_eq!(g.err(), "chroot: invalid group\n");

    let l = t::run("chroot", &["--groups=nosuchgroup_xyz", &root, "true"], "");
    assert_eq!(l.code, 125);
    assert_eq!(l.err(), "chroot: invalid group\n");
}

#[test]
fn numeric_userspec_is_accepted() {
    let root = test_root();
    let out = root.join("o2");
    let _ = fs::remove_file(&out);
    let r = t::run(
        "chroot",
        &[
            "--userspec=0:0",
            &root.to_string_lossy(),
            "/bin/coreutils",
            "sh",
            "-c",
            "pwd > /o2",
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(&out).unwrap().trim(), "/");
}

#[test]
fn missing_option_values_match_clap_wording() {
    let r = t::run("chroot", &["/tmp", "--userspec"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().contains("a value is required for '--userspec <USER:GROUP>'"));
    let g = t::run("chroot", &["/tmp", "--groups"], "");
    assert!(g.err().contains("a value is required for '--groups <GROUP1,GROUP2...>'"));
}

#[test]
fn unknown_option_matches_clap_structure() {
    let r = t::run("chroot", &["--bogus"], "");
    assert_eq!(r.code, 125);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: chroot [OPTION]... NEWROOT [COMMAND [ARG]...]\n\nFor more information, try '--help'.\n"
    );
    let s = t::run("chroot", &["-x"], "");
    assert_eq!(s.code, 125);
    assert!(s.err().contains("  tip: to pass '-x' as a value, use '-- -x'"));
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("chroot", &["--help"], "");
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out().lines().next().unwrap(),
        "Run COMMAND with root directory set to NEWROOT."
    );
    assert!(r
        .out()
        .contains("\nUsage: chroot [OPTION]... NEWROOT [COMMAND [ARG]...]\n"));
    assert!(r
        .out()
        .contains("      --groups <GROUP1,GROUP2...>  Comma-separated list of groups to switch to"));
    assert!(r
        .out()
        .contains("      --userspec <USER:GROUP>      Colon-separated user and group to switch to."));
    assert!(r.out().contains("      --skip-chdir                 Use this option to not change the working directory to / after"));
    assert!(r.out().contains("  -h, --help                       Print help"));
    assert!(r.out().contains("  -V, --version                    Print version"));

    let v = t::run("chroot", &["--version"], "");
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "chroot (uutils coreutils) 0.10.0\n");
    let vh = t::run("chroot", &["-Vh"], "");
    assert_eq!(vh.out(), "chroot (uutils coreutils) 0.10.0\n");
    let hv = t::run("chroot", &["-hV"], "");
    assert_eq!(hv.out(), r.out());
}

#[test]
fn fuzz_option_sequences() {
    let root = test_root().to_string_lossy().to_string();
    let mut rng = t::Rng::new(20261203);
    let flags = [
        "--skip-chdir",
        "--userspec=0:0",
        "--groups=0",
        "-x",
        "--bogus",
        "--help",
        "--version",
        "--",
    ];
    for _case in 0..12 {
        let mut args: Vec<String> = Vec::new();
        let count = 1 + (rng.below(3) as usize);
        for _ in 0..count {
            args.push(flags[rng.below(flags.len() as u64) as usize].to_string());
        }
        args.push(root.clone());
        args.push("/bin/coreutils".to_string());
        args.push("true".to_string());
        let refs: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
        let r = t::run("chroot", &refs, "");
        let first_bad = refs.iter().position(|a| *a == "-x" || *a == "--bogus");
        let help_at = refs.iter().position(|a| *a == "--help");
        let vers_at = refs.iter().position(|a| *a == "--version");
        let wins_help = help_at.map(|h| first_bad.map_or(true, |b| h < b)).unwrap_or(false);
        let wins_ver = vers_at.map(|h| first_bad.map_or(true, |b| h < b)).unwrap_or(false);
        if wins_help {
            assert!(r.out().starts_with("Run COMMAND with root directory"), "args {:?}", refs);
        } else if wins_ver {
            assert_eq!(r.out(), "chroot (uutils coreutils) 0.10.0\n", "args {:?}", refs);
        } else {
            assert!(
                matches!(r.code, 0 | 125 | 126 | 127),
                "args {:?} rc {} err {}",
                refs,
                r.code,
                r.err()
            );
        }
    }
}

#[test]
fn e2e_through_the_real_binary() {
    let root = test_root();
    let out = root.join("o3");
    let _ = fs::remove_file(&out);
    let r = t::run_bin(
        "chroot",
        &[
            &root.to_string_lossy(),
            "/bin/coreutils",
            "sh",
            "-c",
            "echo exec-inside > /o3",
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(&out).unwrap(), "exec-inside\n");
}

#[test]
fn inline_values_on_flag_options_are_rejected() {
    let r = t::run("chroot", &["--help=1"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().contains("unexpected value '1' for '--help'"), "err {}", r.err());
    let v = t::run("chroot", &["--version=x"], "");
    assert_eq!(v.code, 125);
    assert!(v.err().contains("unexpected value 'x' for '--version'"), "err {}", v.err());
}

#[test]
fn double_dash_makes_newroot_missing() {
    let r = t::run("chroot", &["--", "/tmp"], "");
    assert_eq!(r.code, 125);
    assert!(r.err().contains("  <newroot>"), "err {}", r.err());
    assert!(r.err().contains("For more information, try '--help'."));
}

#[test]
fn userspec_empty_parts_and_group_list_with_empty_entries() {
    let root = test_root();
    let out = root.join("up_out");
    let _ = fs::remove_file(&out);
    // ":0" sets only the group, "0:" only the user; "--groups=" yields an empty list.
    let r = t::run(
        "chroot",
        &[
            "--userspec=:0",
            "--groups=0,,0",
            &root.to_string_lossy(),
            "/bin/coreutils",
            "sh",
            "-c",
            "pwd > /up_out",
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(&out).unwrap().trim_end(), "/");
    let s = t::run("chroot", &["--userspec=nosuchuser_xyz:", &root.to_string_lossy()], "");
    assert_eq!(s.code, 125);
    assert_eq!(s.err(), "chroot: invalid user\n");
    let u = t::run("chroot", &["--groups=nosuchgroup_xyz,0", &root.to_string_lossy()], "");
    assert_eq!(u.code, 125);
    assert_eq!(u.err(), "chroot: invalid group\n");
}

#[test]
fn dropping_privileges_before_chroot_is_reported() {
    let root = test_root();
    // --groups= avoids initgroups, so setgid/setuid succeed as root and chroot(2) is the step that fails.
    let r = t::run(
        "chroot",
        &[
            "--groups=",
            "--userspec=12345:12345",
            &root.to_string_lossy(),
            "/bin/coreutils",
            "true",
        ],
        "",
    );
    assert_eq!(r.code, 125, "err {}", r.err());
    assert!(r.err().contains("cannot chroot to '"), "err {}", r.err());
    assert!(r.err().contains("Operation not permitted"), "err {}", r.err());
}

#[test]
fn command_missing_or_not_executable_inside_the_new_root() {
    let root = test_root();
    let r = t::run("chroot", &[&root.to_string_lossy(), "/no_such_prog_xyz"], "");
    assert_eq!(r.code, 127, "err {}", r.err());
    assert!(r.err().contains("No such file or directory"), "err {}", r.err());

    let f = root.join("notexec");
    fs::write(&f, "not a program\n").unwrap();
    fs::set_permissions(&f, fs::Permissions::from_mode(0o644)).unwrap();
    let s = t::run("chroot", &[&root.to_string_lossy(), "/notexec"], "");
    assert_eq!(s.code, 126, "err {}", s.err());
    assert!(s.err().contains("Permission denied"), "err {}", s.err());
}

#[test]
fn without_a_command_the_shell_is_used() {
    let root = test_root();
    std::env::set_var("SHELL", "/no_such_shell_xyz");
    let r = t::run("chroot", &[&root.to_string_lossy()], "");
    std::env::remove_var("SHELL");
    assert_eq!(r.code, 127, "err {}", r.err());
    assert!(r.err().contains("no_such_shell_xyz"), "err {}", r.err());
}

#[test]
fn option_values_may_follow_as_a_separate_argument() {
    let root = test_root();
    let out = root.join("sep_out");
    let _ = fs::remove_file(&out);
    let r = t::run(
        "chroot",
        &[
            "--userspec",
            "0:0",
            "--groups",
            "0",
            &root.to_string_lossy(),
            "/bin/coreutils",
            "sh",
            "-c",
            "pwd > /sep_out",
        ],
        "",
    );
    assert_eq!(r.code, 0, "err {}", r.err());
    assert_eq!(fs::read_to_string(&out).unwrap().trim_end(), "/");
}

#[test]
fn numeric_user_without_groups_reports_the_privilege_step_that_fails() {
    let root = test_root();
    let r = t::run(
        "chroot",
        &["--userspec=12345", &root.to_string_lossy(), "/bin/coreutils", "true"],
        "",
    );
    assert_eq!(r.code, 125, "err {}", r.err());
    assert!(
        r.err().contains("cannot set groups") || r.err().contains("cannot chroot to '"),
        "err {}",
        r.err()
    );
}
