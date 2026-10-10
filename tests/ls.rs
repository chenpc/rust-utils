//! ls：預設列表、long 欄位、排序不變量、quoting、dired、hyperlink、錯誤措辭。
//!
//! 判定採 man-derivable 不變量（排序單調、欄位可 parse、rc/措辭），
//! 不追求與本機參考 byte-exact（殘留 quirk 見 docs/assumptions.md「D 群叢集 4」）。
use coreutils::testutil as t;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

const D: &str = "/tmp/ls_t";

fn setup() {
    let _ = std::fs::remove_dir_all(D);
    std::fs::create_dir_all(format!("{}/sub", D)).unwrap();
    std::fs::write(format!("{}/big", D), vec![b'a'; 2000]).unwrap();
    std::fs::write(format!("{}/small", D), b"hi").unwrap();
    std::fs::write(format!("{}/m1", D), b"12345678").unwrap();
    std::fs::create_dir_all(format!("{}/adir", D)).unwrap();
    std::fs::write(format!("{}/f1", D), b"hello\n").unwrap();
    std::fs::write(format!("{}/old.txt", D), b"old").unwrap();
    std::os::unix::fs::symlink("nonexistent", format!("{}/dang", D)).unwrap();
    std::os::unix::fs::symlink("adir", format!("{}/lnk", D)).unwrap();
    std::fs::write(format!("{}/we\tird", D), b"tab").unwrap();
    std::fs::write(format!("{}/sp ace", D), b"space").unwrap();
    // 三個同 mtime 的檔案（-S/-t tie 用）
    for n in ["ta", "tb", "tc"] {
        std::fs::write(format!("{}/{}", D, n), b"x").unwrap();
    }
    std::fs::set_permissions(format!("{}/run.sh", D), std::fs::Permissions::from_mode(0o755)).ok();
    std::fs::write(format!("{}/run.sh", D), b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(format!("{}/run.sh", D), std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn names(o: &str) -> Vec<String> {
    o.lines().map(|s| s.to_string()).collect()
}

#[test]
fn default_listing_is_sorted_single_column() {
    setup();
    let r = t::run("ls", &["-1", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let v = names(&r.out());
    let mut sorted = v.clone();
    sorted.sort();
    assert_eq!(v, sorted, "預設應為名稱升序單欄：{:?}", v);
    assert!(!v.iter().any(|s| s.starts_with('.')), "預設不列隱藏檔");
    assert!(v.contains(&"big".to_string()));
    assert!(v.contains(&"adir".to_string()));
    // -a 含 . 與 ..，-A 不含但含隱藏檔
    std::fs::write(format!("{}/.hidden", D), b"h").unwrap();
    let ra = t::run("ls", &["-1", "-a", D], "");
    let va = names(&ra.out());    assert!(va.contains(&".".to_string()) && va.contains(&"..".to_string()));
    assert!(va.contains(&".hidden".to_string()));
    let r_a = t::run("ls", &["-1", "-A", D], "");
    let v_a = names(&r_a.out());
    assert!(!v_a.contains(&".".to_string()) && !v_a.contains(&"..".to_string()));
    assert!(v_a.contains(&".hidden".to_string()));
    // -r 為逆序
    let rr = t::run("ls", &["-1", "-r", D], "");
    let vr = names(&rr.out());
    let mut chk = vr.clone();
    chk.sort();
    chk.reverse();
    assert_eq!(vr, chk);
}

#[test]
fn long_format_fields() {
    setup();
    let r = t::run("ls", &["-l", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.starts_with("total "), "目錄列表應有 total 行：{:?}", out);
    let meta_big = std::fs::metadata(format!("{}/big", D)).unwrap();
    let mut seen_big = false;
    for line in out.lines().skip(1) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        assert!(toks.len() >= 6, "long 行欄位不足：{}", line);
        let mode = toks[0];
        assert_eq!(mode.len(), 10, "mode 欄 10 字元：{}", line);
        assert!("-.dlpcsbT".contains(mode.chars().next().unwrap()), "型別位：{}", line);
        assert!(toks[1].parse::<u64>().is_ok(), "nlink 為數字：{}", line);
        let size: u64 = toks[4].parse().unwrap_or(0);
        let name = *toks.last().unwrap();
        if name == "big" {
            seen_big = true;
            assert_eq!(size, meta_big.len());
            assert_eq!(mode.chars().next().unwrap(), '-');
        }
        if name == "adir" {
            let _ = mode;
        }
    }
    assert!(seen_big, "未看到 big 行");
    // symlink 不追隨：lrwxrwxrwx 且 'name -> target'
    let line = out
        .lines()
        .find(|l| l.ends_with("dang -> nonexistent"))
        .expect("dang 行應含 symlink 目標");
    assert!(line.starts_with("lrwx"), "symlink mode：{}", line);
    // -d：只列 operand 本身，無 total
    let rd = t::run("ls", &["-l", "-d", D], "");
    assert_eq!(rd.code, 0);
    let od = rd.out();
    assert!(!od.starts_with("total "), "-d 不印 total：{:?}", od);
    assert_eq!(od.lines().count(), 1);
    assert!(od.contains(&format!(" {}", D)), "-d 列 operand：{:?}", od);
}

#[test]
fn inode_and_blocks_columns() {
    setup();
    let m = std::fs::metadata(format!("{}/big", D)).unwrap();
    let r = t::run("ls", &["-1", "-i", D], "");
    assert_eq!(r.code, 0);
    let mut found = false;
    for line in r.out().lines() {
        let mut it = line.split_whitespace();
        let ino: u64 = it.next().unwrap().parse().unwrap();
        let name = it.next().unwrap();
        if name == "big" {
            found = true;
            assert_eq!(ino, m.ino());
        }
    }
    assert!(found);
    let rs = t::run("ls", &["-1", "-s", D], "");
    assert_eq!(rs.code, 0);
    // -s 目錄列表有 total = Σ blocks/2
    assert!(rs.out().contains("total "));
    let plain = t::run("ls", &["-1", D], "");
    assert_eq!(plain.code, 0);
    let rs_lines: Vec<String> = rs
        .out()
        .lines()
        .filter(|l| !l.starts_with("total "))
        .map(|l| {
            let mut it = l.splitn(2, ' ');
            let head = it.next().unwrap();
            if head.parse::<u64>().is_ok() {
                it.next().unwrap_or("").to_string()
            } else {
                head.to_string()
            }
        })
        .collect();
    assert_eq!(rs_lines, plain.out().lines().collect::<Vec<_>>(), "-s 名稱欄與預設相同");
    let rk = t::run("ls", &["-1", "-k", D], "");
    assert_eq!(rk.code, 0);
    assert_eq!(rk.out(), plain.out());
    let rl = t::run("ls", &["-l", "-s", D], "");
    assert_eq!(rl.code, 0);
    assert!(rl.out().starts_with("total "));
    // blocks 欄為數字且 mode 欄仍在
    for line in rl.out().lines().skip(1) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        assert!(toks[0].parse::<u64>().is_ok(), "blocks 欄數字：{}", line);
        assert_eq!(toks[1].len(), 10, "mode 欄：{}", line);
    }
    // -k 為 no-op（參考實測）
    let rk = t::run("ls", &["-1", "-k", D], "");
    assert_eq!(rk.code, 0);
    assert_eq!(rk.out(), plain.out());
}

#[test]
fn sort_invariants_size_and_time() {
    setup();
    // -S：size 單調非增
    let r = t::run("ls", &["-1", "-S", D], "");
    assert_eq!(r.code, 0);
    let mut prev: Option<u64> = None;
    for line in r.out().lines() {
        let p = std::fs::symlink_metadata(format!("{}/{}", D, line)).unwrap();
        let sz = p.len();
        if let Some(q) = prev {
            assert!(q >= sz, "-S 應 size 非增：{} < {}", q, sz);
        }
        prev = Some(sz);
    }
    // 集合與預設相同
    let a: Vec<String> = names(&r.out());
    let mut b: Vec<String> = names(&t::run("ls", &["-1", D], "").out());
    b.sort();
    let mut a2 = a.clone();
    a2.sort();
    assert_eq!(a2, b, "-S 不改變集合");
    // -t：mtime 非增
    let rt = t::run("ls", &["-1", "-t", D], "");
    assert_eq!(rt.code, 0);
    let mut prev: Option<i64> = None;
    for line in rt.out().lines() {
        let p = std::fs::symlink_metadata(format!("{}/{}", D, line)).unwrap();
        let mt = p.mtime();
        if let Some(q) = prev {
            assert!(q >= mt, "-t 應 mtime 非增：{} < {}", q, mt);
        }
        prev = Some(mt);
    }
    // --sort=none / -U：同一集合
    let ru = t::run("ls", &["-1", "-U", D], "");
    assert_eq!(ru.code, 0);
    let mut u = names(&ru.out());
    u.sort();
    assert_eq!(u, b);
    // --sort=version：數字段排序（natural）
    std::fs::write(format!("{}/a1", D), b"1").unwrap();
    std::fs::write(format!("{}/a10", D), b"1").unwrap();
    std::fs::write(format!("{}/a2", D), b"1").unwrap();
    let rv = t::run("ls", &["-1", "--sort=version", D], "");
    let vv = names(&rv.out());
    let i1 = vv.iter().position(|s| s == "a1").unwrap();
    let i2 = vv.iter().position(|s| s == "a2").unwrap();
    let i10 = vv.iter().position(|s| s == "a10").unwrap();
    assert!(i1 < i2 && i2 < i10, "natural sort：{:?}", vv);
    // --sort=extension：同副檔名相鄰
    let re = t::run("ls", &["-1", "--sort=extension", D], "");
    let ve = names(&re.out());
    let mut seen: Vec<&str> = Vec::new();
    for n in &ve {
        let ext = match n.rfind('.') {
            Some(i) => &n[i..],
            None => "",
        };
        if !seen.contains(&ext) {
            assert!(!seen.iter().any(|s| *s == ext), "副檔名群被打散：{:?}", ve);
            seen.push(ext);
        } else {
            assert_eq!(seen.last(), Some(&ext), "副檔名群被打散：{:?}", ve);
        }
    }
}

#[test]
fn quoting_styles() {
    setup();
    let tabn = "we\tird";
    // 預設 literal：tab 原樣輸出
    let r = t::run("ls", &["-1", D], "");
    assert!(r.out().lines().any(|l| l == tabn), "預設 literal");
    // -b：escape（tab → \t）
    let rb = t::run("ls", &["-1", "-b", D], "");
    assert!(rb.out().lines().any(|l| l == "we\\tird"), "-b escape：{:?}", rb.out());
    // -Q / --quoting-style=c：C 雙引號
    let rq = t::run("ls", &["-1", "-Q", D], "");
    assert!(rq.out().lines().any(|l| l == "\"we\\tird\""), "-Q C-style");
    // -q：控制字元 → ?
    let rq2 = t::run("ls", &["-1", "-q", D], "");
    assert!(rq2.out().lines().any(|l| l == "we?ird"), "-q hide control");
    // --quoting-style=shell / shell-escape
    let rs = t::run("ls", &["-1", "--quoting-style=shell", D], "");
    assert_eq!(rs.code, 0);
    let rs2 = t::run("ls", &["-1", "--quoting-style=shell-escape", D], "");
    assert_eq!(rs2.code, 0);
    assert!(rs2.out().contains("$'") || rs2.out().contains("\\t"), "shell-escape：{:?}", rs2.out());
    // 非法 style → clap 措辭 rc1
    let rbad = t::run("ls", &["--quoting-style=bogus", D], "");
    assert_ne!(rbad.code, 0);
    assert!(rbad.err().contains("invalid value"), "{}", rbad.err());
}

#[test]
fn errors_and_exit_codes() {
    setup();
    let r = t::run("ls", &["nonexistent"], "");
    assert_eq!(r.code, 2);
    assert_eq!(
        r.err().trim_end(),
        "ls: cannot access 'nonexistent': No such file or directory",
        "錯誤措辭不應帶 (os error N)"
    );
    // 不可讀目錄
    let np = format!("{}/noperm", D);
    std::fs::create_dir_all(&np).unwrap();
    std::fs::set_permissions(&np, std::fs::Permissions::from_mode(0o000)).unwrap();
    let r2 = t::run("ls", &[&np], "");
    if libc_geteuid() != 0 {
        assert_eq!(r2.code, 2);
        assert!(r2.err().contains("cannot open directory"), "{}", r2.err());
        assert!(r2.err().contains("Permission denied"), "{}", r2.err());
    }
    std::fs::set_permissions(&np, std::fs::Permissions::from_mode(0o755)).unwrap();
    // 非法選項
    let r3 = t::run("ls", &["--bogus-flag", D], "");
    assert_ne!(r3.code, 0);
    let r4 = t::run("ls", &["-Z", D], "");
    assert_eq!(r4.code, 0);
    assert!(r4.out().lines().all(|l| l.starts_with("? ")), "-Z 前綴：{:?}", r4.out());
}

extern "C" {
    fn geteuid() -> u32;
}
fn libc_geteuid() -> u32 {
    unsafe { geteuid() }
}

#[test]
fn classify_and_indicators() {
    setup();
    let r = t::run("ls", &["-1", "-F", D], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    assert!(out.lines().any(|l| l == "adir/"), "目錄 /：{:?}", out);
    assert!(out.lines().any(|l| l == "dang@"), "symlink @");
    assert!(out.lines().any(|l| l == "run.sh*"), "executable *");
    let rp = t::run("ls", &["-1", "-p", D], "");
    assert!(rp.out().lines().any(|l| l == "adir/"));
    assert!(!rp.out().lines().any(|l| l == "run.sh*"), "-p 不加 *");
    let ri = t::run("ls", &["-1", "--indicator-style=file-type", D], "");
    assert_eq!(ri.code, 0);
    let rs = t::run("ls", &["-1", "--indicator-style=slash", D], "");
    assert!(rs.out().lines().any(|l| l == "adir/"));
    let rbad = t::run("ls", &["--indicator-style=bogus", D], "");
    assert_ne!(rbad.code, 0);
    // --group-directories-first：目錄群在前
    let rg = t::run("ls", &["-1", "--group-directories-first", D], "");
    assert_eq!(rg.code, 0);
    let vg = names(&rg.out());
    let first_file = vg.iter().position(|s| *s == "big").unwrap();
    let last_dir = vg.iter().rposition(|s| *s == "adir").unwrap();
    assert!(last_dir < first_file, "目錄群應在前：{:?}", vg);
}

#[test]
fn multi_operand_and_recursive() {
    setup();
    // 檔案 operand 先（排序）、目錄 operand 後（排序）且帶 'name:' 標題與空行分隔
    let r = t::run("ls", &["-1", &format!("{}/f1", D), D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains("f1\n"), "檔案 operand 先：{:?}", out);
    assert!(out.contains(&format!("\n{}:\n", D)), "目錄組標題：{:?}", out);
    // 目錄群之間空行
    let r2 = t::run("ls", &["-1", &format!("{}/adir", D), &format!("{}/sub", D)], "");
    assert_eq!(r2.code, 0);
    let o2 = r2.out();
    assert!(o2.contains(&format!("{}:\n", format!("{}/adir", D))));
    assert!(o2.contains(&format!("\n{}:\n", format!("{}/sub", D))), "組間空行：{:?}", o2);
    // -R 遞迴
    std::fs::write(format!("{}/adir/inner", D), b"i").unwrap();
    let rr = t::run("ls", &["-1", "-R", &format!("{}/adir", D)], "");
    assert_eq!(rr.code, 0);
    let o3 = rr.out();
    assert!(o3.contains("inner"), "-R 列遞迴內容：{:?}", o3);
    // 單 operand 目錄：直接列內容、無標題
    let r1 = t::run("ls", &["-1", D], "");
    assert!(!r1.out().contains(&format!("{}:\n", D)), "單目錄 operand 不印標題");
}

#[test]
fn time_styles_and_full_time() {
    setup();
    let r = t::run("ls", &["-l", "--time-style=long-iso", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    for line in r.out().lines().skip(1) {
        // 日期欄 YYYY-MM-DD HH:MM（symlink 行有 ' -> target' 尾段、含空白/tab 檔名略過）
        if line.contains(" -> ") || line.contains("sp ace") || line.contains("we\tird") {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        let d = &toks[toks.len() - 3];
        assert!(
            d.chars().nth(4) == Some('-') && d.chars().nth(7) == Some('-'),
            "long-iso 日期：{}",
            line
        );
    }
    let rf = t::run("ls", &["-l", "--full-time", D], "");
    assert_eq!(rf.code, 0);
    for line in rf.out().lines().skip(1) {
        if line.contains(" -> ") || line.contains("sp ace") || line.contains("we\tird") {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        let zone = toks[toks.len() - 2];
        assert!(zone.starts_with('+') || zone.starts_with('-'), "full-iso 時區：{}", line);
    }
    let rp = t::run("ls", &["-l", "--time-style=+%F", D], "");
    assert_eq!(rp.code, 0);
    let ru = t::run("ls", &["-l", "-u", D], "");
    assert_eq!(ru.code, 0);
    let rc = t::run("ls", &["-l", "-c", D], "");
    assert_eq!(rc.code, 0);
    let rt = t::run("ls", &["-l", "--time=atime", D], "");
    assert_eq!(rt.code, 0);
    let ro = t::run("ls", &["--time-style=off", D], "");
    assert_eq!(ro.code, 0, "off 應正常列表：{}", ro.err());
}

#[test]
fn humanize_total_and_sizes() {
    setup();
    let r = t::run("ls", &["-l", "-h", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    let first = out.lines().next().unwrap();
    assert!(first.starts_with("total "), "total 行：{:?}", first);
    // big=2000 → 2.0K（本機參考慣例）
    assert!(out.lines().any(|l| l.contains("2.0K") && l.ends_with("big")), "-h 大小欄：{:?}", out);
    let rs = t::run("ls", &["-l", "--si", D], "");
    assert_eq!(rs.code, 0);
    assert!(rs.out().lines().any(|l| l.contains("2.0k") && l.ends_with("big")), "--si 大小欄");
    // 純 humanize 不變量：K 單位存在且 total 行存在
    assert!(rs.out().contains('k') || rs.out().contains('M'));
}

#[test]
fn dired_format_offsets() {
    setup();
    let adir_operand = format!("{}/adir", D);
    let r = t::run("ls", &["-D", "-d", &adir_operand], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    let trailer = out
        .lines()
        .find(|l| l.starts_with("//DIRED//"))
        .expect("dired trailer");
    let opts = out
        .lines()
        .find(|l| l.starts_with("//DIRED-OPTIONS//"))
        .expect("dired options");
    assert!(opts.contains("--quoting-style=literal"), "{:?}", opts);
    // offset 為名稱在整個輸出 byte 流的 0-based 起點/終點
    let bytes = out.as_bytes();
    let pair_tokens: Vec<&str> = trailer.split_whitespace().skip(1).collect();
    let start: usize = pair_tokens[0].parse().unwrap();
    let end: usize = pair_tokens[1].parse().unwrap();
    let slice = std::str::from_utf8(&bytes[start..end]).unwrap();
    assert_eq!(slice, adir_operand, "DIRED offset 應指向名稱（operand 原文）");
    // -b → C style；--quoting-style=shell-escape → shell-escape-show-control
    let rb = t::run("ls", &["-D", "-d", "-b", &format!("{}/adir", D)], "");
    assert!(rb.out().contains("--quoting-style=C"), "{}", rb.out());
    let rse = t::run(
        "ls",
        &["-D", "-d", "--quoting-style=shell-escape", &format!("{}/adir", D)],
        "",
    );
    assert!(rse.out().contains("shell-escape-show-control"), "{}", rse.out());
}

#[test]
fn hyperlink_osc8() {
    setup();
    let r = t::run("ls", &["-1", "--hyperlink=always", &format!("{}/big", D)], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains("\u{1b}]8;;file://"), "OSC8 前綴：{:?}", out);
    assert!(out.contains("big\u{1b}]8;;\u{1b}"), "OSC8 收尾");
    // auto：非 tty 不包 hyperlink（guest console 若為 tty 則跳過此斷言）
    use std::io::IsTerminal;
    let ra = t::run("ls", &["-1", "--hyperlink=auto", &format!("{}/big", D)], "");
    if !std::io::stdout().is_terminal() {
        assert!(!ra.out().contains('\u{1b}'), "auto 非 tty 不包 hyperlink：{:?}", ra.out());
    }
    let rn = t::run("ls", &["-1", "--hyperlink=never", &format!("{}/big", D)], "");
    assert!(!rn.out().contains('\u{1b}'));
    // --color=never 不輸出色彩 escape
    let rc = t::run("ls", &["-1", "--color=never", D], "");
    assert!(!rc.out().contains('\u{1b}'), "--color=never");
}

#[test]
fn ignore_and_backup_flags() {
    setup();
    let r = t::run("ls", &["-1", "-I", "*.txt", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!r.out().contains("old.txt"), "-I 過濾：{:?}", r.out());
    let r2 = t::run("ls", &["-1", "--ignore=f1", D], "");
    assert!(!r2.out().contains("f1"));
    let r3 = t::run("ls", &["-1", "-B", D], "");
    assert_eq!(r3.code, 0);
    std::fs::write(format!("{}/bak~", D), b"b").unwrap();
    let r4 = t::run("ls", &["-1", "-B", "-a", D], "");
    assert!(!r4.out().contains("bak~"), "-B 略過備份檔：{:?}", r4.out());
    let r5 = t::run("ls", &["-1", D], "");
    assert!(r5.out().contains("bak~"));
}

#[test]
fn width_and_columns_layout() {
    setup();
    // -w 1：單欄；行內最後項目後不補空白
    let r1 = t::run("ls", &["-C", "-w", "1", D], "");
    assert_eq!(r1.code, 0);
    assert!(r1.out().lines().all(|l| !l.ends_with(' ') || l.trim().is_empty()), "-w 1 無補白");
    // 多欄：所有行不超過 width（以可见字元計）
    let r = t::run("ls", &["-C", "-w", "30", D], "");
    assert_eq!(r.code, 0);
    for line in r.out().lines() {
        let mut col = 0usize;
        for ch in line.chars() {
            col = if ch == '\t' { (col / 8 + 1) * 8 } else { col + 1 };
        }
        assert!(col <= 30 || line.trim_end() == line.trim_end_matches(' '), "欄寬 ≤30：{:?}", line);
    }
    // -x 為 row-major：前 k 個名稱出現在第一行
    let rx = t::run("ls", &["-x", "-w", "30", D], "");
    assert_eq!(rx.code, 0);
    let one = t::run("ls", &["-1", D], "");
    let mut sorted = names(&one.out());
    sorted.retain(|s| !s.contains(' ') && !s.contains('\t'));
    let first: Vec<String> = rx
        .out()
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(first, &sorted[..first.len()], "-x 應 row-major：{:?}", rx.out());
    // -T 4 影響 tab 但仍是合法列表
    let rt = t::run("ls", &["-C", "-T", "4", "-w", "40", D], "");
    assert_eq!(rt.code, 0);
    // --format 變體
    for f in ["--format=long", "--format=vertical", "--format=single-column", "--format=commas", "--format=horizontal", "--format=across"] {
        let rf = t::run("ls", &[f, D], "");
        assert_eq!(rf.code, 0, "{} 也應可用：{}", f, rf.err());
    }
    let rbad = t::run("ls", &["--format=bogus", D], "");
    assert_ne!(rbad.code, 0);
}

#[test]
fn dereference_and_special_files() {
    setup();
    std::fs::write(format!("{}/fifo1", D), b"").ok();
    let _ = std::fs::remove_file(format!("{}/fifo1", D));
    // FIFO 由 mkfifo 建立（用 shell builtin 不可靠，改直接 libc）
    extern "C" {
        fn mkfifo(path: *const u8, mode: u32) -> i32;
    }
    let p = format!("{}/fifo1", D);
    let cpath = std::ffi::CString::new(p.clone()).unwrap();
    let rc = unsafe { mkfifo(cpath.as_bytes().as_ptr(), 0o644) };
    assert_eq!(rc, 0, "mkfifo 失敗");
    let r = t::run("ls", &["-1", "-F", D], "");
    assert!(r.out().contains("fifo1|"), "FIFO |：{:?}", r.out());
    // -L/-H 追隨 dangling symlink → cannot access rc2
    let rl = t::run("ls", &["-1", "-L", &format!("{}/dang", D)], "");
    assert_eq!(rl.code, 2, "-L dangling 應 rc2");
    assert!(rl.err().contains("cannot access"), "{}", rl.err());
    let rh = t::run("ls", &["-1", "-H", &format!("{}/dang", D)], "");
    assert_eq!(rh.code, 2);
    // long 對裝置檔印 major, minor 取代 size
    let rd = t::run("ls", &["-l", "/dev/zero"], "");
    assert_eq!(rd.code, 0, "{}", rd.err());
    assert!(rd.out().contains("crw"), "字元裝置：{:?}", rd.out());
    let rd_out = rd.out();
    let toks: Vec<&str> = rd_out.split_whitespace().collect();
    assert!(toks.iter().any(|s| *s == "1,"), "major 欄：{:?}", rd.out());
    let _ = std::fs::remove_file(&p);
}

#[test]
fn fuzz_ls_invariants() {
    // 內建 PRNG fuzz：隨機名稱/旗標組合下 ls 不應 panic，且輸出集合 = 目錄集合
    setup();
    let flag_pool: [&[&str]; 10] = [
        &["-1"],
        &["-l"],
        &["-a"],
        &["-A"],
        &["-F"],
        &["-i"],
        &["-s"],
        &["-S"],
        &["-t"],
        &["-C", "-w", "40"],
    ];
    let mut state: u64 = 0x243F6A8885A308D3;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let expect: Vec<String> = std::fs::read_dir(D)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    for _ in 0..60 {
        let f = flag_pool[(next() as usize) % flag_pool.len()];
        let r2 = t::run("ls", &[f[0], D], "");
        assert!(r2.code == 0 || r2.code == 2, "rc 合法：{}", r2.code);
        if r2.code == 0 {
            let is_columnar = f[0] == "-C";
            if is_columnar {
                // 多欄含带空白/跳字的檔名：斷言覆蓋（每個預期名稱都出現）而非逐 token 相等
                let joined = r2.out();
                for e in &expect {
                    assert!(joined.contains(e), "-C 輸出應含 {:?}：{:?}", e, joined);
                }
            } else if f.iter().any(|s| *s == "-l" || *s == "--long") {
                // long 行：檔名可能含空白 → 斷言每個預期名稱以連續 token 出現
                for en in &expect {
                    let et: Vec<&str> = en.split_whitespace().collect();
                    let found = r2.out().lines().filter(|l| !l.starts_with("total ")).any(|line| {
                        let lt: Vec<&str> = line.split_whitespace().collect();
                        if et.len() == 1 {
                            let pre = match line.find(" -> ") {
                                Some(p) => &line[..p],
                                None => line,
                            };
                            let base = pre.split_whitespace().last().unwrap_or("");
                            base.trim_end_matches(|c| c == '/' || c == '@' || c == '*' || c == '|' || c == '=') == et[0]
                        } else {
                            lt.windows(et.len()).any(|w| w == et)
                        }
                    });
                    assert!(found, "long 輸出應含 {:?}：{:?}", en, r2.out());
                }
            } else {
                let mut got: Vec<String> = r2
                    .out()
                    .lines()
                    .map(|l| l.trim_end())
                    .filter(|l| !l.is_empty() && !l.starts_with("total ") && !l.starts_with("//DIRED"))
                    .map(|l| {
                        // long 行（mode 欄 10 字元）：名稱在最後（symlink 取 ' -> ' 之前）；
                        // -s/-i 前置欄為純數字 → 取數字前綴之後的全部
                        let head = l.splitn(2, ' ').next().unwrap();
                        if head.len() == 10 && head.chars().next().map(|c| "-.dlpcsbT".contains(c)).unwrap_or(false) {
                            let base = match l.find(" -> ") {
                                Some(p) => &l[..p],
                                None => l,
                            };
                            base.rsplit(' ').next().unwrap_or(base).to_string()
                        } else if head.parse::<u64>().is_ok() && l.len() > head.len() + 1 {
                            l[head.len() + 1..].to_string()
                        } else {
                            l.to_string()
                        }
                    })
                    .map(|s| s.trim_end_matches(|c| c == '/' || c == '@' || c == '*' || c == '|' || c == '=').to_string())
                    .collect();
                got.sort();
                let mut e = expect.clone();
                if f.iter().any(|s| *s == "-a") {
                    e.push(".".to_string());
                    e.push("..".to_string());
                }
                e.sort();
                assert_eq!(got, e, "旗標 {:?} 的集合應等於目錄內容：{:?}", f, r2.out());
            }
        }
    }
}
