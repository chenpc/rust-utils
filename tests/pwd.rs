//! pwd(1) guest e2e 測試。
//!
//! 斷言依據：docs/man/pwd.1.gz 的 `-L/--logical`（用環境 PWD）與 `-P/--physical`（避連字號），
//! 以及 man 未載明、改由本機參考實測決定的項目（預設 physical、後出現的選項贏、
//! 非選項 operand 印 `pwd: ignoring non-option arguments`），見 docs/assumptions.md。
//!
//! 注意：pwd 讀取行程層級狀態（cwd、PWD），每個測試都要保存並還原，否則會污染後續測試。

use coreutils::testutil as t;
use std::path::PathBuf;

fn fresh(tag: &str) -> PathBuf {
    let dir = t::workdir().join(format!("pwd_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create_dir_all");
    dir
}

/// 保存 cwd 與 PWD，Drop 時還原。
struct Saved {
    cwd: PathBuf,
    pwd: Option<String>,
}

impl Saved {
    fn capture() -> Saved {
        Saved {
            cwd: std::env::current_dir().expect("current_dir"),
            pwd: std::env::var("PWD").ok(),
        }
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.cwd);
        match &self.pwd {
            Some(v) => std::env::set_var("PWD", v),
            None => std::env::remove_var("PWD"),
        }
    }
}

/// 在 dir 裡建立 real/ 目錄與指向它的 symlink `link`，并把 cwd 切到 link。
/// 回 (dir, real, link)。
fn setup_link(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = fresh(tag);
    let real = dir.join("real");
    std::fs::create_dir_all(&real).expect("create real");
    let link = dir.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    std::env::set_current_dir(&link).expect("chdir link");
    (dir, real, link)
}

#[test]
fn default_is_physical() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("default");
    let r = t::run("pwd", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim(), real.to_str().unwrap());
    assert_ne!(r.out().trim(), link.to_str().unwrap());
    let r2 = t::run("pwd", &["-P"], "");
    assert_eq!(r2.out().trim(), real.to_str().unwrap());
}

#[test]
fn logical_uses_pwd_even_with_symlink() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("logical");
    std::env::set_var("PWD", &link);
    let r = t::run("pwd", &["-L"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim(), link.to_str().unwrap());
    // 預設仍是 physical（即使 PWD 設成 symlink 路徑）
    let r2 = t::run("pwd", &[], "");
    assert_eq!(r2.out().trim(), real.to_str().unwrap());
}

#[test]
fn later_option_wins() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("order");
    std::env::set_var("PWD", &link);
    let r = t::run("pwd", &["-P", "-L"], "");
    assert_eq!(r.out().trim(), link.to_str().unwrap(), "{}", r.out());
    let r2 = t::run("pwd", &["-L", "-P"], "");
    assert_eq!(r2.out().trim(), real.to_str().unwrap(), "{}", r2.out());
    // 同一選項重複 → clap 的「cannot be used multiple times」（只有 error 行 + 空行 + For more）
    for args in [&["-L", "-L"][..], &["--logical", "--logical"][..], &["-LL"][..]] {
        let r = t::run("pwd", args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        assert_eq!(
            r.err(),
            "error: the argument '--logical' cannot be used multiple times\n\nFor more information, try '--help'.\n",
            "{:?}",
            args
        );
    }
    for args in [&["-P", "-P"][..], &["--phys", "--physical"][..]] {
        let r = t::run("pwd", args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        assert_eq!(
            r.err(),
            "error: the argument '--physical' cannot be used multiple times\n\nFor more information, try '--help'.\n",
            "{:?}",
            args
        );
    }
}

#[test]
fn logical_falls_back_when_pwd_does_not_match() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("fallback");
    let relative = link
        .to_str()
        .unwrap()
        .split('/')
        .skip(2)
        .collect::<Vec<_>>()
        .join("/");
    for pwd in ["/bogus", "", relative.as_str()] {
        std::env::set_var("PWD", pwd);
        let r = t::run("pwd", &["-L"], "");
        assert_eq!(r.code, 0);
        assert_eq!(
            r.out().trim(),
            real.to_str().unwrap(),
            "PWD={} 也必須退回 physical",
            pwd
        );
    }
    // PWD 完全不存在時也一樣
    std::env::remove_var("PWD");
    let r = t::run("pwd", &["-L"], "");
    assert_eq!(r.out().trim(), real.to_str().unwrap());
}

#[test]
fn non_option_operands_are_ignored_with_warning() {
    let _s = Saved::capture();
    let (_d, real, _l) = setup_link("operands");
    for args in [&["x"][..], &["--", "-x"][..], &["a", "b"][..], &["-"][..]] {
        let r = t::run("pwd", args, "");
        assert_eq!(r.code, 0, "{}", r.err());
        assert_eq!(r.out().trim(), real.to_str().unwrap());
        assert!(
            r.err().contains("pwd: ignoring non-option arguments"),
            "{:?} → {}",
            args,
            r.err()
        );
    }
    // 沒有 operand 時不印警告
    let r = t::run("pwd", &["-P"], "");
    assert_eq!(r.err(), "");
}

#[test]
fn unknown_option_is_clap_style_rc1() {
    for args in [&["-x"][..], &["--bogus"][..], &["-L", "--nope"][..], &["-Lx"][..]] {
        let r = t::run("pwd", args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        let err = r.err();
        let lines: Vec<&str> = err.lines().collect();
        assert!(lines[0].starts_with("error: unexpected argument '"), "{}", err);
        assert!(lines[0].ends_with("' found"), "{}", err);
        assert_eq!(lines[1], "");
        assert!(err.contains("  tip: to pass '"), "{}", err);
        assert!(err.contains("Usage: pwd [OPTION]..."), "{}", err);
        assert!(err.contains("For more information, try '--help'."), "{}", err);
        assert!(!err.contains("Try '"), "clap 層錯誤不附 Try 行: {}", err);
    }
}

#[test]
fn long_option_prefix_inference() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("prefix");
    std::env::set_var("PWD", &link);
    let r = t::run("pwd", &["--log"], "");
    assert_eq!(r_out(&r), link.to_str().unwrap());
    let r = t::run("pwd", &["--logical"], "");
    assert_eq!(r_out(&r), link.to_str().unwrap());
    let r = t::run("pwd", &["--phys"], "");
    assert_eq!(r_out(&r), real.to_str().unwrap());
    // 'p' 開頭只有 physical（help/logical/version 不衝突）→ 前綴推斷成立
    let r = t::run("pwd", &["--p"], "");
    assert_eq!(r_out(&r), real.to_str().unwrap());
}

fn r_out(r: &t::Out) -> String {
    r.out().trim().to_string()
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("pwd", &["--help"], "");
    assert_eq!(r.code, 0);
    let expect = "Display the full filename of the current working directory.\n\
                 \n\
                 Usage: pwd [OPTION]...\n\
                 \n\
                 Options:\n\
                 \x20 -L, --logical   use PWD from environment, even if it contains symlinks\n\
                 \x20 -P, --physical  avoid all symlinks\n\
                 \x20 -h, --help      Print help\n\
                 \x20 -V, --version   Print version\n";
    assert_eq!(r.out(), expect);
    let v = t::run("pwd", &["--version"], "");
    assert_eq!(v.code, 0);
    assert_eq!(v.out().trim(), "pwd (uutils coreutils) 0.10.0");
    // -h 先赢、-V 先赢
    assert_eq!(t::run("pwd", &["-hV"], "").out(), expect);
    assert_eq!(
        t::run("pwd", &["-Vh"], "").out().trim(),
        "pwd (uutils coreutils) 0.10.0"
    );
    // help/version 之後的東西部不影響（一律直接回 0）
    assert_eq!(t::run("pwd", &["--hel"], "").out(), expect);
    assert_eq!(
        t::run("pwd", &["--vers"], "").out().trim(),
        "pwd (uutils coreutils) 0.10.0"
    );
}

#[test]
fn e2e_binary_dispatch_and_run_bin() {
    let _s = Saved::capture();
    let (_d, real, _l) = setup_link("bin");
    let r = t::run_bin("pwd", &["-P"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim(), real.to_str().unwrap());
}

#[test]
fn fuzz_flag_sequences() {
    let _s = Saved::capture();
    let (_d, real, link) = setup_link("fuzz");
    std::env::set_var("PWD", &link);
    let mut rng = t::Rng::new(20261124);
    for round in 0..24 {
        let n = 1 + rng.below(4) as usize;
        let mut args: Vec<String> = Vec::new();
        let mut expect_link = false;
        let mut used_logical = false;
        let mut used_physical = false;
        for _ in 0..n {
            match rng.below(3) {
                0 => {
                    if used_logical {
                        continue;
                    }
                    used_logical = true;
                    args.push("-L".to_string());
                    expect_link = true;
                }
                1 => {
                    if used_physical {
                        continue;
                    }
                    used_physical = true;
                    args.push("-P".to_string());
                    expect_link = false;
                }
                _ => {
                    if used_physical {
                        continue;
                    }
                    used_physical = true;
                    args.push("--physical".to_string());
                    expect_link = false;
                }
            }
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = t::run("pwd", &refs, "");
        let want = if expect_link {
            link.to_str().unwrap()
        } else {
            real.to_str().unwrap()
        };
        assert_eq!(r.code, 0);
        assert_eq!(r_out(&r), want, "round {}: {:?}", round, args);
    }
}
