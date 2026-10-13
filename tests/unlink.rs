//! link/unlink guest 測試：措辭與 rc。

use coreutils::testutil as t;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/ukt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

#[test]
fn unlink_basic_and_errors() {
    fresh("unlink");
    std::fs::write("f1", b"x").unwrap();
    let r = t::run("unlink", &["f1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    let r = t::run("unlink", &["f1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "unlink: cannot unlink 'f1': No such file or directory"));
    // symlink 可删
    std::fs::write("t1", b"x").unwrap();
    std::os::unix::fs::symlink("t1", "sl").unwrap();
    let r = t::run("unlink", &["sl"], "");
    assert_eq!(r.code, 0);
    assert!(std::path::Path::new("t1").exists());
    // 目錄：unlink 不能删目錄
    std::fs::create_dir("d1").unwrap();
    let r = t::run("unlink", &["d1"], "");
    assert_eq!((r.code, r.err().contains("cannot unlink 'd1'")), (1, true));
    for args in [vec![], vec!["a", "b"]] {
        let r = t::run("unlink", &args, "");
        assert_eq!((r.code, r.err().contains("missing operand")), (2, true));
    }
    let r = t::run("unlink", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: unlink")), (0, true));
    let r = t::run("unlink", &["-V"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "unlink (uutils coreutils) 0.10.0"));
    let r = t::run("unlink", &["--bogus"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
}


#[test]
fn fuzz_invariants() {
    // 不變量：存在→刪除成功且檔案消失；不存在→ rc1 + ENOENT 訊息、不動其他檔；
    // clap 錯誤 rc2 且檔案不受影響。
    let dir = fresh("fuzz");
    let names: Vec<String> = (0..40).map(|i| format!("f{i}")).collect();
    for n in &names {
        std::fs::write(dir.join(n), "x").expect("write");
    }
    let mut st: u64 = 0x0C11_7E5E;
    for i in 0..40u32 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let target = &names[(st >> 40) as usize % names.len()];
        let missing = format!("gone{i}");
        let existed = dir.join(target).exists();
        let mode = (st >> 30) % 3;
        match mode {
            0 => {
                let r = t::run("unlink", &[target], "");
                assert_eq!(r.code, if existed { 0 } else { 1 }, "{target} {}", r.err());
                assert!(!dir.join(target).exists(), "{target} 仍在");
            }
            1 => {
                let r = t::run("unlink", &[&missing], "");
                assert_eq!(r.code, 1, "{missing}");
                assert!(r.err().contains("No such file"), "{missing}: {}", r.err());
            }
            _ => {
                // 兩個 operand：unlink 只收一個 FILE → clap 錯誤，不動任何檔案
                let r = t::run("unlink", &[target, target], "");
                assert_eq!(r.code, 2, "{target} -> {} {}", r.code, r.err());
                assert_eq!(dir.join(target).exists(), existed, "{target}");
            }
        }
        let _ = i;
    }
    assert_eq!(t::run("unlink", &["--nope"], "").code, 2);
    assert_eq!(t::run("unlink", &[], "").code, 2);
}
