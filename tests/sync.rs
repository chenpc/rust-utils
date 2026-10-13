//! sync 測試：guest 以 root 執行，全部走 in-process dispatch。

use coreutils::testutil as t;

#[test]
fn no_args_syncs_all() {
    let r = t::run("sync", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"");
    assert_eq!(r.stderr, b"");
}

#[test]
fn syncs_regular_file() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &[&format!("{dir}/f1")], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stderr, b"");
}

#[test]
fn data_only_mode() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &["-d", &format!("{dir}/f1")], "");
    assert_eq!(r.code, 0);
    // --data 同義
    let r2 = t::run("sync", &["--data", &format!("{dir}/f1")], "");
    assert_eq!(r2.code, 0);
}

#[test]
fn file_system_mode() {
    let r = t::run("sync", &["-f"], "");
    assert_eq!(r.code, 0);
    let r2 = t::run("sync", &["--file-system"], "");
    assert_eq!(r2.code, 0);
    // 帶檔案也接受
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r3 = t::run("sync", &["-f", &format!("{dir}/f1")], "");
    assert_eq!(r3.code, 0);
}

#[test]
fn multiple_files_one_bad() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &[&format!("{dir}/f1"), "/tmp/sc/noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.stderr
            .starts_with(b"sync: error opening '/tmp/sc/noexist_zz': No such file or directory\n"),
        "{:?}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn directory_operand_ok() {
    // 目錄 fsync 在 Linux 合法
    std::fs::create_dir_all("/tmp/sc").unwrap();
    let r = t::run("sync", &["/tmp/sc"], "");
    assert_eq!(r.code, 0);
}

#[test]
fn conflict_fs_data() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &["-f", "-d", &format!("{dir}/f1")], "");
    assert_eq!(r.code, 1);
    assert!(
        r.stderr
            .starts_with(b"sync: the argument '--file-system' cannot be used with '--data'"),
        "{:?}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn data_without_file_errors() {
    let r = t::run("sync", &["-d"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stderr,
        b"sync: --data needs at least one argument\n"
    );
}

#[test]
fn unrecognized_option() {
    // 我方自訂解析採 GNU 慣例 rc=2；參考（clap）措辭與 rc 不追隨
    let r = t::run("sync", &["-x"], "");
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"sync: invalid option -- 'x'"));
    let r2 = t::run("sync", &["--bogus"], "");
    assert_eq!(r2.code, 2);
    assert!(r2.stderr.starts_with(b"sync: unrecognized option '--bogus'"));
}

#[test]
fn help_and_version() {
    let r = t::run("sync", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with(b"Usage: sync"));
    let r = t::run("sync", &["-h"], "");
    assert_eq!(r.code, 0);
    let r = t::run("sync", &["--version"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"sync (uutils coreutils) 0.10.0\n");
    let r = t::run("sync", &["-V"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"sync (uutils coreutils) 0.10.0\n");
}

#[test]
fn dash_dash_terminates_options() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/-weird"), b"x").unwrap();
    let r = t::run("sync", &["--", &format!("{dir}/-weird")], "");
    assert_eq!(r.code, 0);
}

#[test]
fn fuzz_invariants() {
    // 不變量：任意存在檔案組合（含重複、含目錄）都 rc 0 且零輸出（man 未定義 sync 的錯誤回報）；
    // 不存在路徑 → rc 1 + ENOENT 類訊息；clap 錯誤 rc 2。
    let dir = t::workdir().join(format!("sync_fuzz_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    for i in 0..8u32 {
        std::fs::write(dir.join(format!("f{i}")), b"data").expect("write");
    }
    let mut st: u64 = 0x51CC0CC5;
    for _ in 0..40u64 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st >> 50) % 4 + 1;
        let files: Vec<String> = (0..n)
            .map(|k| {
                let idx = ((st >> (k * 5)) % 8) as u32;
                let prefix = ["-d", "--data", ""][((st >> (k * 3)) % 3) as usize];
                let _ = prefix;
                format!("{}/f{}", dir.display(), idx)
            })
            .collect();
        let flag = ["-d", ""][(st as usize >> 1) % 2];
        let mut args: Vec<&str> = Vec::new();
        if !flag.is_empty() {
            args.push(flag);
        }
        for f in &files {
            args.push(f.as_str());
        }
        let r = t::run("sync", &args, "");
        assert_eq!(r.code, 0, "{:?} {}", args, r.err());
        assert_eq!(r.out(), "");
    }
    let r = t::run("sync", &["/nonexistent-sync-target"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("No such file"), "{}", r.err());
    assert_eq!(t::run("sync", &["--nope"], "").code, 2);
}
