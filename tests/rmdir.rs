//! rmdir guest 測試：ENOTEMPTY 措辭、--ignore-fail-on-non-empty、-p 逐級、-v 順序。

use coreutils::testutil as t;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/rdt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

#[test]
fn basic_and_notempty() {
    fresh("basic");
    std::fs::create_dir("b").unwrap();
    let r = t::run("rmdir", &["b"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    let r = t::run("rmdir", &["b"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "rmdir: failed to remove 'b': No such file or directory"));
    std::fs::create_dir_all("c/sub").unwrap();
    let r = t::run("rmdir", &["c"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "rmdir: failed to remove 'c': Directory not empty"));
}

#[test]
fn ignore_fail_on_non_empty() {
    fresh("ign");
    std::fs::create_dir_all("b/f").unwrap();
    let r = t::run("rmdir", &["--ignore-fail-on-non-empty", "b"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert!(std::path::Path::new("b/f").is_dir());
    // ENOENT 不受 ignore 影響
    let r = t::run("rmdir", &["--ignore-fail-on-non-empty", "nope_zz"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "rmdir: failed to remove 'nope_zz': No such file or directory"));
}

#[test]
fn parents_chain() {
    fresh("parents");
    std::fs::create_dir_all("b/c/d").unwrap();
    let r = t::run("rmdir", &["-p", "b/c/d"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert!(!std::path::Path::new("b").exists());
    // 失敗即停
    let r = t::run("rmdir", &["-p", "b/c/d"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "rmdir: failed to remove 'b/c/d': No such file or directory"));
    // 中途非空：删掉 d、c，停在 b
    std::fs::create_dir_all("e/f/g").unwrap();
    std::fs::write("e/marker", b"x").unwrap();
    let r = t::run("rmdir", &["--parents", "e/f/g"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "rmdir: failed to remove 'e': Directory not empty"));
    assert!(!std::path::Path::new("e/f").exists());
}

#[test]
fn verbose_order_and_long_opts() {
    fresh("verb");
    std::fs::create_dir("v1").unwrap();
    let r = t::run("rmdir", &["-v", "v1"], "");
    assert_eq!((r.code, r.err()), (0, "rmdir: removing directory, 'v1'\n".to_string()));
    assert!(r.stdout.is_empty());
    let r = t::run("rmdir", &["--verbose", "nope2"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "rmdir: removing directory, 'nope2'\nrmdir: failed to remove 'nope2': No such file or directory\n");
    let r = t::run("rmdir", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: rmdir")), (0, true));
    let r = t::run("rmdir", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "rmdir (uutils coreutils) 0.10.0"));
    let r = t::run("rmdir", &["-h"], "");
    assert_eq!((r.code, r.out().contains("Usage")), (0, true));
    let r = t::run("rmdir", &["-V"], "");
    assert_eq!((r.code, r.out().contains("uutils")), (0, true));
    let r = t::run("rmdir", &["--bogus"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
    let r = t::run("rmdir", &["-q"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "rmdir: invalid option -- 'q'"));
    let r = t::run("rmdir", &[], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "rmdir: missing operand"));
    // "--" 之後視為目錄名
    let r = t::run("rmdir", &["--", "nope3"], "");
    assert_eq!((r.code, r.err().contains("'nope3'")), (1, true));
}

#[test]
fn fuzz_rmdir_matches_fs_state() {
    fresh("fz");
    let mut rng = t::Rng::new(4242);
    for _ in 0..40 {
        let n = format!("d{}", rng.below(9999));
        let mk = std::fs::create_dir(&n).is_ok();
        let r = t::run("rmdir", &[&n], "");
        if mk {
            assert_eq!(r.code, 0);
            assert!(!std::path::Path::new(&n).exists());
        } else {
            assert_eq!(r.code, 1);
        }
        // 重複删必失敗且措辭穩定
        let r = t::run("rmdir", &[&n], "");
        assert_eq!((r.code, r.err().trim_end()), (1, format!("rmdir: failed to remove '{}': No such file or directory", n).as_str()));
    }
}
