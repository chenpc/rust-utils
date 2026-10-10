//! ln guest 測試：hard/symbolic 措辭、-f/-b/-t/-T/-n/-r、單 operand。

use coreutils::testutil as t;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/lnt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

#[test]
fn hard_links() {
    fresh("hard");
    std::fs::write("src1", b"hi").unwrap();
    let r = t::run("ln", &["src1", "h1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    let r = t::run("ln", &["src1", "h1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create hard link 'src1' => 'h1': Already exists"));
    let r = t::run("ln", &["-f", "src1", "h1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    // 單 operand → './X'
    let r = t::run("ln", &["src1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create hard link 'src1' => './src1': Already exists"));
    // -v
    let r = t::run("ln", &["-v", "src1", "h2"], "");
    assert_eq!((r.code, r.out()), (0, "ln: linked 'src1' => 'h2'\n".to_string()));
    // -L 追隨 symlink source
    std::os::unix::fs::symlink("src1", "sl").unwrap();
    let r = t::run("ln", &["-L", "sl", "hl2"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read("hl2").unwrap(), b"hi");
    // -P（預設）對 symlink source 做 hard link：取決於核心
    // （本 guest/host 核心 link(2) on symlink 回 rc0；老核心 EPERM）→ 兩者皆接受
    let r = t::run("ln", &["sl", "hl3"], "");
    assert!(r.code == 0 || (r.code == 1 && r.err().contains("failed to create hard link 'sl' => 'hl3'")), "hard-on-symlink: rc={} {}", r.code, r.err());
}

#[test]
fn symbolic_links() {
    fresh("sym");
    std::fs::write("src1", b"x").unwrap();
    let r = t::run("ln", &["-s", "src1", "s1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert_eq!(std::fs::read_link("s1").unwrap().to_string_lossy(), "src1");
    let r = t::run("ln", &["-s", "src1", "s1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create symbolic link 's1': Already exists"));
    let r = t::run("ln", &["-s", "-f", "src2zz", "s1"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_link("s1").unwrap().to_string_lossy(), "src2zz");
    // dangling 允許
    let r = t::run("ln", &["--symbolic", "noexist_zz", "dang"], "");
    assert_eq!(r.code, 0);
    // 末 operand 為目錄 → 鏈入目錄
    std::fs::create_dir("d").unwrap();
    std::fs::write("src1", b"x").unwrap();
    let r = t::run("ln", &["-s", "src1", "d"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_link("d/src1").unwrap().to_string_lossy(), "src1");
    // -T：末 operand 視為普通檔名
    std::fs::create_dir("d2").unwrap();
    let r = t::run("ln", &["-sT", "src1", "d2"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create symbolic link 'd2': Already exists"));
    // ln -s 進既有 symlink-to-dir：預設追隨；-n 不追隨
    std::os::unix::fs::symlink("d", "dl").unwrap();
    let r = t::run("ln", &["-s", "newsrc", "dl"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_link("dl/newsrc").unwrap().to_string_lossy(), "newsrc");
    let r = t::run("ln", &["-n", "-s", "newsrc2", "dl"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create symbolic link 'dl': Already exists"));
}

#[test]
fn backup_force_suffix() {
    fresh("bk");
    std::fs::write("src1", b"a").unwrap();
    std::fs::write("dst", b"other").unwrap();
    // -b：備份後建立
    let r = t::run("ln", &["-b", "src1", "dst"], "");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read("dst~").unwrap(), b"other");
    assert_eq!(std::fs::read("dst").unwrap(), b"a");
    // -S 自訂後綴
    std::fs::write("dst2", b"x").unwrap();
    let r = t::run("ln", &["-b", "-S", ".bak", "src1", "dst2"], "");
    assert_eq!(r.code, 0);
    assert!(std::path::Path::new("dst2.bak").exists());
    // --backup --suffix= 長選項
    std::fs::write("dst3", b"x").unwrap();
    let r = t::run("ln", &["--backup", "--suffix=_bk", "src1", "dst3"], "");
    assert_eq!(r.code, 0);
    assert!(std::path::Path::new("dst3_bk").exists());
    // -f 不備份直接覆蓋
    std::fs::write("dst4", b"gone").unwrap();
    let r = t::run("ln", &["-f", "src1", "dst4"], "");
    assert_eq!(r.code, 0);
    assert!(!std::path::Path::new("dst4~").exists());
}

#[test]
fn target_dir_and_relative() {
    fresh("tdir");
    std::fs::write("s1", b"1").unwrap();
    std::fs::write("s2", b"2").unwrap();
    std::fs::create_dir("td").unwrap();
    let r = t::run("ln", &["-t", "td", "s1", "s2"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert!(std::path::Path::new("td/s1").exists() && std::path::Path::new("td/s2").exists());
    // 不存在的 operand → hard link 錯誤（'x' => './x' 措辭用於 tdir 亦同 linkpath=td/x）
    let r = t::run("ln", &["-t", "td", "noexist_zz"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: failed to create hard link 'noexist_zz' => 'td/noexist_zz': No such file or directory"));
    // -t 缺值
    let r = t::run("ln", &["-t"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "ln: option requires an argument -- 't'"));
    std::fs::write("s3", b"3").unwrap();
    std::fs::write("s4", b"4").unwrap();
    let r = t::run("ln", &["--target-directory=td", "s3", "s4"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert!(std::path::Path::new("td/s3").exists() && std::path::Path::new("td/s4").exists());
    // -rs：相對 symlink 目標
    std::fs::write("srcrel", b"1").unwrap();
    std::fs::create_dir("sub").unwrap();
    let r = t::run("ln", &["-rs", "srcrel", "sub/lnk"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    let tgt = std::fs::read_link("sub/lnk").unwrap();
    assert_eq!(tgt.to_string_lossy(), "../srcrel");
    assert_eq!(std::fs::read("sub/lnk").unwrap(), b"1");
    // -r 未搭配 -s → rc2（clap 措辭不追隨）
    let r = t::run("ln", &["-r", "s1", "hardrel"], "");
    assert_eq!((r.code, r.err().contains("--relative")), (2, true));
}

#[test]
fn misc_errors_and_opts() {
    fresh("misc");
    std::fs::write("s1", b"1").unwrap();
    // 多 operand 末非目錄
    let r = t::run("ln", &["s1", "zz1", "notadir_zz"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "ln: target 'notadir_zz': Not a directory"));
    let r = t::run("ln", &[], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "ln: missing operand"));
    let r = t::run("ln", &["--bogus", "s1"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
    let r = t::run("ln", &["-q", "s1"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "ln: invalid option -- 'q'"));
    let r = t::run("ln", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: ln")), (0, true));
    let r = t::run("ln", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "ln (uutils coreutils) 0.10.0"));
    let r = t::run("ln", &["-h"], "");
    assert_eq!((r.code, r.out().contains("Usage")), (0, true));
    let r = t::run("ln", &["-V"], "");
    assert_eq!((r.code, r.out().contains("uutils")), (0, true));
    // -i 接受（非互動環境不提示）
    let r = t::run("ln", &["-i", "s1", "ii"], "");
    assert_eq!(r.code, 0);
    // -S 缺值
    let r = t::run("ln", &["-S"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "ln: option requires an argument -- 'S'"));
}

#[test]
fn fuzz_symlink_roundtrip() {
    fresh("fz");
    let mut rng = t::Rng::new(777333);
    for i in 0..80 {
        let v = format!("val_{}", rng.below(1_000_000));
        let name = format!("l{}", i);
        let r = t::run("ln", &["-s", &v, &name], "");
        assert_eq!(r.code, 0, "case {i}: {}", r.err());
        assert_eq!(std::fs::read_link(&name).unwrap().to_string_lossy(), v);
    }
}
