//! ls 第二輪：parse 分支補測（-o/-g/-n/-f/--zero/--hide、-h 語意、多 operand、-R、錯誤措辭）。
use coreutils::testutil as t;

const D: &str = "/tmp/ls_x";

fn setup() {
    let _ = std::fs::remove_dir_all(D);
    std::fs::create_dir_all(format!("{}/sub", D)).unwrap();
    std::fs::write(format!("{}/big", D), vec![b'a'; 2000]).unwrap();
    std::fs::write(format!("{}/f1", D), b"hello\n").unwrap();
    std::fs::write(format!("{}/b~", D), b"backup").unwrap();
    std::os::unix::fs::symlink("f1", format!("{}/lnk", D)).unwrap();
    std::fs::write(format!("{}/.hid", D), b"h").unwrap();
    std::fs::write(format!("{}/exe", D), b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(format!("{}/exe", D), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

#[allow(dead_code)]
fn toks(o: &str) -> Vec<String> {
    o.lines()
        .flat_map(|l| l.split_whitespace())
        .filter(|s| *s != "total")
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn short_h_is_human_not_help() {
    setup();
    // GNU man：-h = --human-readable；--help 才是說明
    let r = t::run("ls", &["-h", D], "");
    assert_eq!(r.code, 0, "-h 不應印 usage：{}", r.out());
    assert!(!r.out().contains("Usage"), "-h 不應印 usage：{:?}", r.out());
    assert!(r.out().contains("big"));
    let rh = t::run("ls", &["--help"], "");
    assert_eq!(rh.code, 0);
    assert!(rh.out().contains("Usage"));
    let rl = t::run("ls", &["-l", "-h", D], "");
    assert_eq!(rl.code, 0);
    assert!(rl.out().starts_with("total "), "{:?}", rl.out());
    assert!(rl.out().contains("2.0K"), "-l -h 大小欄 humanize：{:?}", rl.out());
    let rs = t::run("ls", &["-l", "--si", D], "");
    assert_eq!(rs.code, 0);
    assert!(rs.out().contains("2.0k"), "--si 小寫 k：{:?}", rs.out());
}

#[test]
fn owner_group_numeric_variants() {
    setup();
    // -o：long 但無 group 欄（參考 uutils 0.10.0：-o/-g 皆省略 group）
    let ro = t::run("ls", &["-o", D], "");
    assert_eq!(ro.code, 0, "{}", ro.err());
    let rg = t::run("ls", &["-g", D], "");
    assert_eq!(rg.code, 0);
    // -g：long 格式、只省略一個所有權欄（GNU man 省略擁有者；本機參考 uutils 0.10.0 省略群組
    // → 依政策採 GNU，記 assumptions）。用欄數不變量避免 user/group 同名歧義。
    for line in rg.out().lines().skip(1) {
        if line.contains(" -> ") || line.contains("sp ace") || line.contains("we\tird") {
            continue;
        }
        let tk: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tk[0].len(), 10, "-g 亦 long 格式：{}", line);
    }
    let rl = t::run("ls", &["-l", D], "");
    let la: Vec<String> = rl.out().lines().filter(|l| !l.starts_with("total ")).map(|s| s.to_string()).collect();
    let lb: Vec<String> = rg.out().lines().filter(|l| !l.starts_with("total ")).map(|s| s.to_string()).collect();
    for (a, b) in la.iter().zip(lb.iter()) {
        if a.contains(" -> ") || a.contains("sp ace") || a.contains("we\tird") {
            continue;
        }
        let ta = a.split_whitespace().count();
        let tb = b.split_whitespace().count();
        assert_eq!(tb + 1, ta, "-g 比 -l 少一個所有權欄：{} vs {}", a, b);
    }
    let rn = t::run("ls", &["-n", D], "");
    assert_eq!(rn.code, 0);
    for line in rn.out().lines().skip(1) {
        if line.contains(" -> ") {
            continue;
        }
        let tk: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tk[0].len(), 10, "mode：{}", line);
        assert!(tk[2].parse::<u32>().is_ok(), "-n 用戶欄為數值：{}", line);
        assert!(tk[3].parse::<u32>().is_ok(), "-n 群組欄為數值：{}", line);
    }
    for line in ro.out().lines().skip(1) {
        if line.contains(" -> ") {
            continue;
        }
        let tk: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tk[0].len(), 10, "mode：{}", line);
        // -o：mode nlink user size date.. name → size 欄在 user 之後
        assert!(tk[2].parse::<u32>().is_ok() || tk[2].parse::<u64>().is_ok(), "欄序：{}", line);
    }
    // 欄數不變量：-o/-g 比 -l 少一個所有權欄（GNU：-o 省 group、-g 省 owner）
    let rl2 = t::run("ls", &["-l", D], "");
    let lc: Vec<String> = rl2.out().lines().filter(|l| !l.starts_with("total ")).map(|s| s.to_string()).collect();
    for other in [&rg, &ro] {
        let oc: Vec<String> = other.out().lines().filter(|l| !l.starts_with("total ")).map(|s| s.to_string()).collect();
        for (a, b) in lc.iter().zip(oc.iter()) {
            if a.contains(" -> ") || a.contains("sp ace") || a.contains("we\tird") {
                continue;
            }
            assert_eq!(b.split_whitespace().count() + 1, a.split_whitespace().count(), "{} vs {}", a, b);
        }
    }
}

#[test]
fn zero_hide_and_fast() {
    setup();
    // --zero：NUL 分隔（-1 亦同）
    let r = t::run("ls", &["-1", "--zero", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let raw = String::from_utf8_lossy(&r.stdout);
    assert!(raw.contains('\0'), "--zero 應為 NUL 分隔：{:?}", raw);
    assert!(raw.contains("big\0"));
    // --hide / -I glob
    let rh = t::run("ls", &["-1", "--hide=*.txt", "--hide=f1", D], "");
    assert_eq!(rh.code, 0, "{}", rh.err());
    assert!(!rh.out().contains("f1"), "--hide：{:?}", rh.out());
    // -B 略過 *~
    let rb = t::run("ls", &["-1", "-B", "-a", D], "");
    assert_eq!(rb.code, 0);
    assert!(!rb.out().contains("b~"), "-B：{:?}", rb.out());
    // -f = -a -U（含 . ..、不排序）
    let rf = t::run("ls", &["-1", "-f", D], "");
    assert_eq!(rf.code, 0, "{}", rf.err());
    let vf: Vec<String> = rf.out().lines().map(|s| s.to_string()).collect();
    assert!(vf.contains(&".".to_string()) && vf.contains(&"..".to_string()));
    assert!(vf.contains(&".hid".to_string()));
    let mut s = vf.clone();
    s.sort();
    assert_ne!(vf, s, "-f 不排序（readdir 序）：{:?}", vf);
    // -A 含 .hid 不含 . ..
    let r_a = t::run("ls", &["-1", "-A", D], "");
    let v_a: Vec<String> = r_a.out().lines().map(|s| s.to_string()).collect();
    assert!(v_a.contains(&".hid".to_string()));
    assert!(!v_a.contains(&"..".to_string()));
}

#[test]
fn recursive_multi_operand_and_dired() {
    setup();
    // -R 遞迴 + 多 operand：每個目錄組有標題
    std::fs::create_dir_all(format!("{}/sub/deep", D)).unwrap();
    std::fs::write(format!("{}/sub/deep/x", D), b"x").unwrap();
    let r = t::run("ls", &["-1", "-R", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains(&format!("{}:", D)), "根組標題：{:?}", out);
    assert!(out.contains(&format!("{}/sub:", D)), "sub 組標題：{:?}", out);
    assert!(out.contains("deep"), "deep 內容：{:?}", out);
    assert!(out.contains("x"), "深層檔案：{:?}", out);
    // -d 多 operand：每個目錄印自身長行
    let rd = t::run("ls", &["-d", &format!("{}/sub", D), D], "");
    assert_eq!(rd.code, 0, "{}", rd.err());
    let od = rd.out();
    assert!(od.contains(&format!("{}/sub", D)), "{:?}", od);
    assert!(od.contains(D), "{:?}", od);
    // -D 目錄內容：//DIRED// 與 //DIRED-OPTIONS// trailer
    let rr = t::run("ls", &["-D", D], "");
    assert_eq!(rr.code, 0, "{}", rr.err());
    let od2 = rr.out();
    assert!(od2.contains("//DIRED// "), "dired trailer：{:?}", od2);
    assert!(od2.contains("//DIRED-OPTIONS// --quoting-style=literal"), "{:?}", od2);
    // trailer offset 全部成對且指向名稱byte流
    let bytes = od2.as_bytes();
    let line = od2.lines().find(|l| l.starts_with("//DIRED//")).unwrap();
    let nums: Vec<usize> = line.split_whitespace().skip(1).map(|s| s.parse().unwrap()).collect();
    assert!(nums.len() % 2 == 0 && !nums.is_empty());
    for pair in nums.chunks(2) {
        let s = std::str::from_utf8(&bytes[pair[0]..pair[1]]).unwrap();
        assert!(!s.is_empty() && !s.contains('\n'), "offset 對：{:?}", s);
    }
}

#[test]
fn parse_errors_and_option_forms() {
    setup();
    // 本機參考（uutils 0.10.0 = clap）：選項缺值 → rc1 "a value is required for ..."
    for (f, rc, needle) in [
        ("--format", 1, "a value is required for '--format <format>'"),
        ("--sort", 1, "a value is required for '--sort <field>'"),
        ("--time", 1, "a value is required for '--time <field>'"),
        ("-I", 1, "a value is required for '--ignore <PATTERN>'"),
        ("--block-size", 1, "a value is required for '--block-size <SIZE>'"),
        ("--hyperlink", 0, ""),
    ] {
        let r = t::run("ls", &["-1", f], "");
        assert_eq!(r.code, rc, "{} 缺值：{}", f, r.err());
        if !needle.is_empty() {
            assert!(r.err().contains(needle), "{}: {}", f, r.err());
        }
    }
    // 選項消耗 operand 作為值（參考 quirk）：驗證後的錯誤措辭
    let r = t::run("ls", &["-1", "--format", D], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid value"), "{}", r.err());
    let r = t::run("ls", &["-1", "--block-size", D], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid --block-size argument"), "{}", r.err());
    let r = t::run("ls", &["-1", "-w", "abc", D], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid line width: 'abc'"), "{}", r.err());
    let r = t::run("ls", &["-1", "-T", "abc", D], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid tab size: 'abc'"), "{}", r.err());
    // 非法值（clap invalid value rc1）
    for f in ["--format=bogus", "--sort=bogus", "--sort=type", "--time=bogus", "--color=bogus", "--hyperlink=bogus"] {
        let r = t::run("ls", &[f, D], "");
        assert_eq!(r.code, 1, "{} 非法值應 rc1（clap 措辭）：{}", f, r.err());
        assert!(r.err().contains("invalid value"), "{}: {}", f, r.err());
    }
    // --time-style 非法值：參考不驗證（採預設） rc0
    let r = t::run("ls", &["--time-style=bogus", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // 未知旗標
    let r = t::run("ls", &["-~", D], "");
    assert_eq!(r.code, 2, "{}", r.err());
    assert!(r.err().contains("option"), "{}", r.err());
    // --absolute-paths：本機參考不支援（clap unexpected argument rc2）
    let r2 = t::run("ls", &["--absolute-paths", D], "");
    assert_eq!(r2.code, 2, "參考不支援 --absolute-paths");
    assert!(r2.err().contains("unexpected argument"), "{}", r2.err());
    // -- 結束選項
    let r3 = t::run("ls", &["-1", "--", D], "");
    assert_eq!(r3.code, 0);
}

#[test]
fn color_and_context_flags() {
    setup();
    // --color=never / 預設（非 tty）：不輸出 escape
    let r = t::run("ls", &["-1", "--color=never", D], "");
    assert!(!r.out().contains('\u{1b}'), "--color=never");
    let r2 = t::run("ls", &["-1", D], "");
    assert!(!r2.out().contains('\u{1b}'), "非 tty 預設不上色");
    // --color=always：有 escape（僅有/無層級，見 assumptions）
    let r3 = t::run("ls", &["-1", "--color=always", D], "");
    assert_eq!(r3.code, 0);
    assert!(r3.out().contains("\u{1b}[") || r3.out().contains("\u{1b}[0m"), "--color=always 應有色：{:?}", r3.out());
    // LS_COLORS 覆蓋（目錄色）
    // -Z context 前綴
    let r4 = t::run("ls", &["-1", "-Z", D], "");
    assert_eq!(r4.code, 0);
    assert!(r4.out().lines().all(|l| l.starts_with("? ")), "-Z：{:?}", r4.out());
    // --context 同義
    let r5 = t::run("ls", &["-1", "--context", D], "");
    assert_eq!(r5.code, 0);
}

#[test]
fn version_and_misc_format_flags() {
    setup();
    let rv = t::run("ls", &["-v", D], "");
    assert_eq!(rv.code, 0);
    let rt = t::run("ls", &["-l", "-t", D], "");
    assert_eq!(rt.code, 0);
    let rs = t::run("ls", &["-1", "-S", D], "");
    assert_eq!(rs.code, 0);
    // --time 變體
    for v in ["--time=atime", "--time=ctime", "--time=mtime", "--time=birth"] {
        let r = t::run("ls", &["-l", v, D], "");
        assert_eq!(r.code, 0, "{}：{}", v, r.err());
    }
    // --version
    let rvv = t::run("ls", &["--version"], "");
    assert_eq!(rvv.code, 0);
    assert!(rvv.out().contains("coreutils"), "{:?}", rvv.out());
    // -k 不影響輸出（參考 quirk）
    let a = t::run("ls", &["-1", D], "");
    let b = t::run("ls", &["-1", "-k", D], "");
    assert_eq!(a.out(), b.out());
    // 目錄本身作為唯一 operand 與當前目錄等效
    let cur = t::run("ls", &["-1", "."], "");
    let dotdot = t::run("ls", &["-1", ".."], "");
    assert_eq!(cur.code, 0);
    assert_eq!(dotdot.code, 0);
    // --hyperlink=bogus 已在 parse_errors；此处驗 auto/never 不輸出
    let r1 = t::run("ls", &["-1", "--hyperlink=never", D], "");
    assert!(!r1.out().contains('\u{1b}'));
}
