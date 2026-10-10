//! vdir：ls 的 long  listing 別名（man: vdir = ls -l 类，本實時間欄為空）。
use coreutils::testutil as t;

const D: &str = "/tmp/vdir_t";

fn setup() {
    let _ = std::fs::remove_dir_all(D);
    std::fs::create_dir_all(format!("{}/sub", D)).unwrap();
    std::fs::write(format!("{}/f1", D), b"hello").unwrap();
    std::fs::write(format!("{}/big", D), vec![b'x'; 1234]).unwrap();
    std::os::unix::fs::symlink("f1", format!("{}/lnk", D)).unwrap();
}

#[test]
fn vdir_uses_long_columns_without_time() {
    setup();
    let r = t::run("vdir", &[D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.starts_with("total "), "vdir 目錄列表应有 total：{:?}", out);
    for line in out.lines().skip(1) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        // mode nlink user group size name（時間欄為空欄）
        assert!(toks.len() >= 6, "vdir 欄位：{}", line);
        assert_eq!(toks[0].len(), 10, "mode 欄 10 字元：{}", line);
        assert!(toks[1].parse::<u64>().is_ok(), "nlink：{}", line);
        assert!(toks[4].parse::<u64>().is_ok(), "size 欄為數字：{}", line);
        // 不應出現日期（YYYY-MM-DD 或 Mon DD）
        assert!(!line.contains(" 20"), "時間欄應為空：{}", line);
    }
    let name_cols: Vec<&str> = out.lines().skip(1).map(|l| l.split_whitespace().last().unwrap()).collect();
    assert!(name_cols.contains(&"big"));
    assert!(out.contains("lnk -> f1"), "symlink 目標：{:?}", out);
}

#[test]
fn vdir_flags_and_errors() {
    setup();
    let ra = t::run("vdir", &["-a", D], "");
    assert_eq!(ra.code, 0);
    let rb = t::run("vdir", &["-1", D], "");
    assert_eq!(rb.code, 0);
    assert!(rb.out().lines().any(|l| l == "f1"), "-1 單欄名稱：{:?}", rb.out());
    let rd = t::run("vdir", &["nonexistent"], "");
    assert_eq!(rd.code, 2);
    assert!(rd.err().contains("cannot access 'nonexistent'"), "{}", rd.err());
    let rh = t::run("vdir", &["--help"], "");
    assert_eq!(rh.code, 0);
}
