//! mkdir guest 測試：錯誤措辭（EEXIST 帶 operand、其他不帶）、-p/-v/-m（octal/symbolic）。
//!
//! guest 以 root（umask 0）跑：預設 mode 777；`-m` 走 mkdir(2) mode（uutils 不補
//! chmod），期望值以即時 umask 計算。

use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/mkt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

fn mode_of(p: &str) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

/// 以一次無 -m 的 mkdir 探得 umask（observed = 0o777 & !umask）
fn umask_bits() -> u32 {
    let _ = std::fs::remove_dir("/tmp/mkt_umask_probe");
    let r = t::run("mkdir", &["/tmp/mkt_umask_probe"], "");
    assert_eq!(r.code, 0);
    let m = mode_of("/tmp/mkt_umask_probe");
    0o777 & !m
}

#[test]
fn basic_create_and_eexist() {
    fresh("basic");
    let r = t::run("mkdir", &["a"], "");
    assert_eq!(r.code, 0);
    assert!(r.stderr.is_empty());
    assert!(std::path::Path::new("a").is_dir());
    assert_eq!(mode_of("a"), 0o777 & !umask_bits());
    let r = t::run("mkdir", &["a"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "mkdir: a: File exists\n");
}

#[test]
fn parents_and_verbose() {
    fresh("parents");
    let r = t::run("mkdir", &["-p", "b/c/d"], "");
    assert_eq!(r.code, 0);
    assert!(r.stderr.is_empty());
    assert!(std::path::Path::new("b/c/d").is_dir());
    // -p 已存在 → rc0 靜默
    let r = t::run("mkdir", &["-p", "b/c/d"], "");
    assert_eq!(r.code, 0);
    assert!(r.stderr.is_empty());
    // -v 每層都印
    let r = t::run("mkdir", &["-v", "-p", "x/y/z"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "mkdir: created directory 'x'\nmkdir: created directory 'x/y'\nmkdir: created directory 'x/y/z'\n");
    // -v 但已存在（無 -p）→ 錯誤
    let r = t::run("mkdir", &["-v", "x"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "mkdir: x: File exists"));
}

#[test]
fn enoent_error_has_no_operand_prefix() {
    fresh("enoent");
    // uutils quirk：非 EEXIST 錯誤不帶 operand 前綴
    let r = t::run("mkdir", &["sub/complex/../x"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "mkdir: No such file or directory\n");
}

#[test]
fn mode_octal_and_symbolic() {
    let u = umask_bits();
    fresh("mode");
    let r = t::run("mkdir", &["-m", "750", "f"], "");
    assert_eq!(r.code, 0);
    assert_eq!(mode_of("f"), 0o750 & !u);
    // symbolic：u=rwx,go=r → 744
    let r = t::run("mkdir", &["-m", "u=rwx,go=r", "mm"], "");
    assert_eq!(r.code, 0);
    assert_eq!(mode_of("mm"), 0o744 & !u);
    // -m750 內嵌、--mode= 長選項
    let r = t::run("mkdir", &["-m640", "g1"], "");
    assert_eq!(r.code, 0);
    assert_eq!(mode_of("g1"), 0o640 & !u);
    let r = t::run("mkdir", &["--mode=700", "g2"], "");
    assert_eq!(r.code, 0);
    assert_eq!(mode_of("g2"), 0o700 & !u);
    // a+rwx 與 u+x 型
    let r = t::run("mkdir", &["--mode", "go-w", "g3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(mode_of("g3"), 0o755 & !u);
    // 多運算元：部分失敗 rc1、其他仍建立
    let r = t::run("mkdir", &["ok1", "ok2"], "");
    assert_eq!(r.code, 0);
    let r = t::run("mkdir", &["ok1", "ok3"], "");
    assert_eq!((r.code, r.err()), (1, "mkdir: ok1: File exists\n".to_string()));
    assert!(std::path::Path::new("ok3").is_dir());
}

#[test]
fn mode_invalid() {
    fresh("badmode");
    let r = t::run("mkdir", &["-m", "abc", "g"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "mkdir: invalid operator (expected +, -, or =, but found b)"));
    let r = t::run("mkdir", &["-m", "888", "g2"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "mkdir: invalid mode '888'"));
    let r = t::run("mkdir", &["-m"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "mkdir: option requires an argument -- 'm'"));
}

#[test]
fn long_opts_z_context_and_help() {
    fresh("long");
    // -Z/--context 接受但 no-op
    let r = t::run("mkdir", &["-Z", "zz"], "");
    assert_eq!(r.code, 0);
    assert!(std::path::Path::new("zz").is_dir());
    let r = t::run("mkdir", &["--context=foo", "cc"], "");
    assert_eq!(r.code, 0);
    let r = t::run("mkdir", &["--parents", "dd/ee"], "");
    assert_eq!(r.code, 0);
    let r = t::run("mkdir", &["--verbose", "ff"], "");
    assert_eq!(r.out(), "mkdir: created directory 'ff'\n");
    let r = t::run("mkdir", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: mkdir")), (0, true));
    let r = t::run("mkdir", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "mkdir (uutils coreutils) 0.10.0"));
    // 無效選項 rc2（clap 措辭不追隨）
    let r = t::run("mkdir", &["--no-such", "q"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
    let r = t::run("mkdir", &["-q", "r"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "mkdir: invalid option -- 'q'"));
    let r = t::run("mkdir", &[], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "mkdir: missing operand"));
}

#[test]
fn fuzz_mkdir_rmdir_roundtrip() {
    fresh("fz");
    let mut rng = t::Rng::new(20261010);
    for i in 0..60 {
        let depth = rng.below(3) + 1;
        let mut path = String::new();
        for d in 0..depth {
            if d > 0 {
                path.push('/');
            }
            path.push_str(&format!("d{}", rng.below(1000)));
        }
        let r = t::run("mkdir", &["-p", &path], "");
        assert_eq!(r.code, 0, "case {i} {path}: {}", r.err());
        assert!(std::path::Path::new(&path).is_dir());
        // -p 冪等
        let r = t::run("mkdir", &["-p", &path], "");
        assert_eq!(r.code, 0);
        // rmdir -p 收回
        let r = t::run("rmdir", &["-p", &path], "");
        assert_eq!(r.code, 0, "case {i} rmdir {path}: {}", r.err());
        assert!(!std::path::Path::new(&path).exists());
    }
}
