//! stat：預設輸出、-c directives、-t terse、-f 檔案系統、錯誤。
use coreutils::testutil as t;
use std::os::unix::fs::MetadataExt;

fn setup(d: &str) {
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
}

#[test]
fn format_directives() {
    let d = "/tmp/stat_t1";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"hello").unwrap();
    let m = std::fs::metadata(&p).unwrap();
    let r = t::run("stat", &["-c", "%n %s %b %h %i %u %g", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let o1 = r.out();
    let toks: Vec<&str> = o1.trim_end().split(' ').collect();
    assert_eq!(toks[0], p);
    assert_eq!(toks[1], m.len().to_string());
    assert_eq!(toks[2], m.blocks().to_string());
    assert_eq!(toks[3], m.nlink().to_string());
    assert_eq!(toks[4], m.ino().to_string());
    assert_eq!(toks[5], m.uid().to_string());
    assert_eq!(toks[6], m.gid().to_string());
    let r = t::run("stat", &["-c", "%a %A %F", &p], "");
    assert_eq!(r.code, 0);
    let o2 = r.out();
    let toks: Vec<&str> = o2.trim_end().split(' ').collect();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(u32::from_str_radix(toks[0], 8).unwrap(), m.permissions().mode() & 0o7777);
    assert_eq!(toks[1].chars().next().unwrap(), '-');
    assert_eq!(o2.trim_end().split(' ').nth(2), Some("regular"));
    // mode hex 含 S_IF 位
    let r = t::run("stat", &["-c", "%f", &p], "");
    assert_eq!(u32::from_str_radix(r.out().trim_end(), 16).unwrap() & 0o170000, 0o100000);
    // 時間指令可解析為數字
    let r = t::run("stat", &["-c", "%X %Y %Z %W %x %y", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let l1 = r.out().trim_end().lines().next().unwrap().to_string();
    let first: Vec<&str> = l1.split(' ').take(4).collect();
    for v in first {
        assert!(v.parse::<i64>().is_ok(), "epoch 欄應為數字：{:?}", r.out());
    }
    // %% 與 %s 混用
    let r = t::run("stat", &["-c", "x%%y=%s", &p], "");
    assert_eq!(r.out().trim_end(), format!("x%y={}", m.len()));
}

#[test]
fn default_output_shape() {
    let d = "/tmp/stat_t2";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"hi").unwrap();
    let r = t::run("stat", &[&p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains(&format!("  File: {}", p)), "{}", out);
    assert!(out.contains("  Size: "), "{}", out);
    assert!(out.contains("IO Block:"), "{}", out);
    assert!(out.contains("regular"), "{}", out);
    assert!(out.contains("Device: "), "{}", out);
    assert!(out.contains("Inode: "), "{}", out);
    assert!(out.contains(" Links: 1"), "{}", out);
    assert!(out.contains("Access: ("), "{}", out);
    assert!(out.contains("Uid:"), "{}", out);
    assert!(out.contains("Gid:"), "{}", out);
    assert!(out.contains("Modify: "), "{}", out);
    assert!(out.contains("Change: "), "{}", out);
}

#[test]
fn symlink_and_follow() {
    let d = "/tmp/stat_t3";
    setup(d);
    let tgt = format!("{}/tgt", d);
    std::fs::write(&tgt, b"12345").unwrap();
    let sl = format!("{}/sl", d);
    std::os::unix::fs::symlink(&tgt, &sl).unwrap();
    // 預設不追隨（uutils 行徑）
    let r = t::run("stat", &["-c", "%F %s", &sl], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim_end(), format!("symbolic link {}", tgt.len()));
    // -L 追隨
    let r = t::run("stat", &["-L", "-c", "%F %s", &sl], "");
    assert_eq!(r.out().trim_end(), "regular file 5");
    // %N 顯示箭頭
    let r = t::run("stat", &["-c", "%N", &sl], "");
    assert_eq!(r.out().trim_end(), format!("'{}' -> '{}'", sl, tgt));
    // 預設輸出含 'sl' -> 'tgt'
    let r = t::run("stat", &[&sl], "");
    assert!(r.out().contains(&format!("  File: '{}' -> '{}'", sl, tgt)), "{}", r.out());
    // dangling
    let dl = format!("{}/dangle", d);
    std::os::unix::fs::symlink("/tmp/stat_dangling_zz", &dl).unwrap();
    let r = t::run("stat", &["-L", "-c", "%s", &dl], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot statx"), "{}", r.err());
    let r = t::run("stat", &["-c", "%s", &dl], "");
    assert_eq!(r.code, 0, "預設不追隨應成功：{}", r.err());
}

#[test]
fn terse_and_multi() {
    let d = "/tmp/stat_t4";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"abcd").unwrap();
    let m = std::fs::metadata(&p).unwrap();
    let r = t::run("stat", &["-t", &p, format!("{}/f1", d).as_str()], "");
    let lines: Vec<String> = r.out().trim_end().lines().map(str::to_string).collect();
    assert_eq!(lines.len(), 2, "每 operand 各印一次：{}", r.out());
    let toks: Vec<&str> = lines[0].split(' ').collect();
    assert_eq!(toks.len(), 16, "{:?}", toks);
    assert_eq!(toks[0], p);
    assert_eq!(toks[1], "4");
    assert_eq!(toks[6], format!("{:x}", m.dev()), "%D hex makedev");
    assert_eq!(toks[7], m.ino().to_string());
    assert_eq!(toks[8], "1");
    let r = t::run("stat", &["--terse", format!("{}/f1", d).as_str(), format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end().lines().count(), 2, "重複 operand 各印一次");
}

#[test]
fn filesystem_block() {
    let d = "/tmp/stat_t5";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"x").unwrap();
    let r = t::run("stat", &["-f", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains(&format!("  File: \"{}\"", p)), "{}", out);
    assert!(out.contains("ID: "), "{}", out);
    assert!(out.contains("Namelen: "), "{}", out);
    assert!(out.contains("Type: "), "{}", out);
    assert!(out.contains("Block Size: "), "{}", out);
    assert!(out.contains("Fundamental block size: "), "{}", out);
    assert!(out.contains("Blocks: Total: "), "{}", out);
    assert!(out.contains("Inodes: Total: "), "{}", out);
    // -f 的 --format
    let r = t::run("stat", &["-f", "-c", "%T %s", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!r.out().trim_end().is_empty());
}

#[test]
fn errors_and_misc() {
    let r = t::run("stat", &[], "");
    assert_eq!(r.code, 2);
    assert_eq!(r.err().trim_end(), "stat: missing operand");
    let r = t::run("stat", &["/tmp/stat_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "stat: cannot statx '/tmp/stat_noexist_zz': No such file or directory"
    );
    let r = t::run("stat", &["-c", "%Q", "/tmp/stat_noexist_zz"], "");
    assert_eq!(r.code, 1);
    let r = t::run("stat", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"));
    let r = t::run("stat", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "stat (uutils coreutils) 0.10.0"));
    let r = t::run("stat", &["-x", "/tmp"], "");
    assert_eq!(r.code, 2);
    // 多 operand 部分失敗：rc1 但存在的仍印出
    let d = "/tmp/stat_t6";
    setup(d);
    let p = format!("{}/ok", d);
    std::fs::write(&p, b"z").unwrap();
    let r = t::run("stat", &["-c", "%n", format!("{}/nope_zz", d).as_str(), &p], "");
    assert_eq!(r.code, 1);
    assert!(r.out().contains(&p), "{}", r.out());
}

#[test]
fn fuzz_size_matches_fs() {
    let d = "/tmp/stat_fz";
    setup(d);
    let mut rng = t::Rng::new(20261015);
    for i in 0..25 {
        let n = rng.below(5000) as usize;
        let p = format!("{}/f{}", d, i);
        std::fs::write(&p, vec![b'x'; n]).unwrap();
        let r = t::run("stat", &["-c", "%s %A", &p], "");
        assert_eq!(r.code, 0, "{}", r.err());
        let got = r.out().trim_end().split(' ').next().unwrap().to_string();
        assert_eq!(got.parse::<usize>().unwrap(), n, "size of {}", p);
    }
}

extern "C" {
    fn mkfifo(path: *const i8, mode: u32) -> i32;
}

fn cstring(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap()
}

#[test]
fn file_types_and_perm_chars() {
    let d = "/tmp/stat_t7";
    setup(d);
    use std::os::unix::fs::PermissionsExt;
    let dir = format!("{}/dd", d);
    std::fs::create_dir(&dir).unwrap();
    let r = t::run("stat", &["-c", "%F %A", &dir], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().starts_with("directory d"), "{}", r.out());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o1777)).unwrap();
    let r = t::run("stat", &["-c", "%a %A", &dir], "");
    let o = r.out();
    assert!(o.contains("drwxrwxrwt"), "{}", o);
    let su = format!("{}/su", d);
    std::fs::write(&su, b"x").unwrap();
    std::fs::set_permissions(&su, std::fs::Permissions::from_mode(0o4755)).unwrap();
    let r = t::run("stat", &["-c", "%a %A %f", &su], "");
    let o = r.out();
    assert!(o.contains("-rwsr-xr-x"), "{}", o);
    let sg = format!("{}/sg", d);
    std::fs::write(&sg, b"x").unwrap();
    std::fs::set_permissions(&sg, std::fs::Permissions::from_mode(0o2744)).unwrap();
    let r = t::run("stat", &["-c", "%A", &sg], "");
    assert!(r.out().contains("-rwxr-Sr--"), "{}", r.out());
    // 空檔 vs 非空預設輸出
    let r = t::run("stat", &[&su], "");
    assert!(r.out().contains("regular file"), "{}", r.out());
    let empty = format!("{}/empty", d);
    std::fs::write(&empty, b"").unwrap();
    let r = t::run("stat", &[&empty], "");
    assert!(r.out().contains("regular empty file"), "{}", r.out());
    // FIFO
    let ff = format!("{}/fifo", d);
    let c = cstring(&ff);
    assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o644) }, 0);
    let r = t::run("stat", &["-c", "%F %A", &ff], "");
    assert!(r.out().starts_with("FIFO p"), "{}", r.out());
}

#[test]
fn directives_extra() {
    let d = "/tmp/stat_t8";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"abc").unwrap();
    let m = std::fs::metadata(&p).unwrap();
    let r = t::run("stat", &["-c", "%B %o %d %H %T %m %U %G", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let o8 = r.out();
    let toks: Vec<&str> = o8.trim_end().split(' ').collect();
    assert_eq!(toks[0], "512", "%B（參考 uutils 固定 512）");
    assert_eq!(toks[1], m.blksize().to_string(), "%o");
    assert_eq!(u32::from_str_radix(toks[2], 16).unwrap() as u64, m.dev(), "%d");
    assert_eq!(toks[3], "?", "%H（參考不實作）");
    assert_eq!(toks[4], "0", "%T fstype in statx ctx");
    assert!(toks[5].starts_with('/'), "%m mount: {}", toks[5]);
    assert!(!toks[6].is_empty(), "%U");
    assert!(!toks[7].is_empty(), "%G");
    // %N 非 symlink → 帶引號路徑（參考）
    let r = t::run("stat", &["-c", "%N", &p], "");
    assert_eq!(r.out().trim_end(), format!("'{}'", p));
    // %W birth：tmpfs 支援時為 epoch，不支援時 '-'
    let r = t::run("stat", &["-c", "%W", &p], "");
    let ow = r.out();
    let w = ow.trim_end();
    assert!(w == "-" || w.parse::<i64>().is_ok(), "{}", w);
}

#[test]
fn fs_directives() {
    let d = "/tmp/stat_t9";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"x").unwrap();
    let r = t::run("stat", &["-f", "-c", "%a %b %c %d %f %i %l %n %s %S %T %p", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let of = r.out();
    let toks: Vec<&str> = of.trim_end().split(' ').collect();
    assert_eq!(toks.len(), 12, "{:?}", r.out());
    assert_eq!(toks[7], p, "%n");
    for t2 in &toks[..7] {
        assert!(!t2.is_empty());
    }
    // --format 長選項與預設 -f 輸出
    let r = t::run("stat", &["-f", "--format", "%n %T", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().contains(&p));
}

#[test]
fn option_edges() {
    let d = "/tmp/stat_t10";
    setup(d);
    let p = format!("{}/f1", d);
    std::fs::write(&p, b"x").unwrap();
    let r = t::run("stat", &["-c"], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "stat: option requires an argument -- 'c'".to_string()));
    let r = t::run("stat", &["--bogus", &p], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "stat: unrecognized option '--bogus'".to_string()));
    // -- 之後 operand
    let r = t::run("stat", &["--", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // -c 緊貼值
    let r = t::run("stat", &["-c%n", &p], "");
    assert_eq!(r.out().trim_end(), p);
    // -Lc 叢集
    let sl = format!("{}/sl", d);
    std::os::unix::fs::symlink(&p, &sl).unwrap();
    let r = t::run("stat", &["-Lc", "%s", &sl], "");
    assert_eq!(r.out().trim_end(), "1");
    // --format= 長選項帶等號
    let r = t::run("stat", &["--format=%n", &p], "");
    assert_eq!(r.out().trim_end(), p);
}
