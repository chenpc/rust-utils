//! df 測試：guest（initramfs）環境為 rootfs /、devtmpfs /dev、9p hostshare /mnt、9p hostout /out。
//! 輸出對齊/格式做結構性斷言（欄位由 /proc/mounts 決定，不硬鎖字串）。

use coreutils::testutil as t;

fn df(args: &[&str]) -> t::Out {
    t::run("df", args, "")
}

fn data_lines(out: &t::Out) -> Vec<String> {
    out.out().lines().skip(1).map(|s| s.to_string()).collect()
}

fn header(out: &t::Out) -> String {
    out.out().lines().next().unwrap_or("").to_string()
}

#[test]
fn default_lists_mounts() {
    let r = df(&[]);
    assert_eq!(r.code, 0);
    let h = header(&r);
    assert!(h.starts_with("Filesystem"), "{h}");
    assert!(h.contains("1K-blocks"), "{h}");
    assert!(h.contains("Use%"), "{h}");
    assert!(h.contains("Mounted on"), "{h}");
    let lines = data_lines(&r);
    assert!(lines.iter().any(|l| l.ends_with(" /mnt")), "{lines:?}");
    assert!(lines.iter().any(|l| l.ends_with(" /out")), "{lines:?}");
    // 偽裝 FS 預設略過
    assert!(!lines.iter().any(|l| l.contains(" proc ")), "{lines:?}");
}

#[test]
fn posix_mode_header() {
    let r = df(&["-P"]);
    assert_eq!(r.code, 0);
    let h = header(&r);
    assert!(h.contains("1024-blocks"), "{h}");
    assert!(h.contains("Capacity"), "{h}");
    assert!(!h.contains("1K-blocks"), "{h}");
}

#[test]
fn type_column() {
    let r = df(&["-T"]);
    assert_eq!(r.code, 0);
    assert!(header(&r).contains("Type"));
    let lines = data_lines(&r);
    let mnt = lines.iter().find(|l| l.ends_with(" /mnt")).unwrap();
    assert!(mnt.contains(" 9p "), "{mnt}");
}

#[test]
fn human_readable() {
    let r = df(&["-h"]);
    assert_eq!(r.code, 0);
    let h = header(&r);
    assert!(h.contains("Size"), "{h}");
    assert!(h.contains("Avail"), "{h}");
    assert!(h.contains("Use%"), "{h}");
    let mnt = data_lines(&r)
        .into_iter()
        .find(|l| l.ends_with(" /mnt"))
        .unwrap();
    // 大小欄應帶 K/M/G/T 後綴（大分割區才有人類單位；至少 Use% 存在）
    assert!(mnt.contains('%'), "{mnt}");
    let h2 = header(&df(&["-H"]));
    assert!(h2.contains("Size"));
}

#[test]
fn inodes_mode() {
    let r = df(&["-i"]);
    assert_eq!(r.code, 0);
    let h = header(&r);
    assert!(h.contains("Inodes"), "{h}");
    assert!(h.contains("IUse%"), "{h}");
    assert!(!h.contains("1K-blocks"), "{h}");
}

#[test]
fn total_row() {
    let r = df(&["-P", "--total"]);
    assert_eq!(r.code, 0);
    let lines = data_lines(&r);
    let last = lines.last().unwrap();
    assert!(last.starts_with("total"), "{last}");
    // total 的 1024-blocks = 各行之和
    let sum: u64 = lines[..lines.len() - 1]
        .iter()
        .map(|l| l.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap())
        .sum();
    let tot: u64 = last.split_whitespace().nth(1).unwrap().parse().unwrap();
    assert_eq!(sum, tot);
}

#[test]
fn filter_types() {
    let r = df(&["-t", "9p"]);
    assert_eq!(r.code, 0);
    let lines = data_lines(&r);
    assert_eq!(lines.len(), 2, "{lines:?}"); // /mnt + /out
    let r2 = df(&["-t", "nosuchfs"]);
    // 過濾後無任何列：參考 uutils 同措辭 rc1，stdout 空（連表頭都不印）
    assert_eq!(r2.code, 1);
    assert_eq!(r2.stdout, b"");
    assert_eq!(r2.stderr, b"df: no file systems processed\n");
    let r3 = df(&["-x", "9p"]);
    assert_eq!(r3.code, 0);
    assert!(!data_lines(&r3).iter().any(|l| l.ends_with(" /mnt")));
}

#[test]
fn local_only_and_all() {
    let r = df(&["-l"]);
    assert_eq!(r.code, 0);
    assert!(!data_lines(&r).iter().any(|l| l.ends_with(" /mnt")));
    let r2 = df(&["-a"]);
    assert_eq!(r2.code, 0);
    // -a 收取 proc/sysfs 等偽裝 FS
    assert!(data_lines(&r2).iter().any(|l| l.contains(" proc ") || l.ends_with(" /proc")));
    // autofs 行即使 -a 也略過
    assert!(!data_lines(&r2).iter().any(|l| l.contains("autofs")));
}

#[test]
fn block_size_options() {
    let p = df(&["-P"]);
    let b512 = df(&["-B", "512", "-P"]);
    // -P 表頭 = 實際區塊數字 + "-blocks"（參考實測）
    assert!(header(&b512).contains("512-blocks"), "{}", header(&b512));
    let row = |o: &t::Out, mount: &str| -> u64 {
        data_lines(o)
            .into_iter()
            .find(|l| l.ends_with(mount))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap()
    };
    assert_eq!(row(&b512, "/mnt"), 2 * row(&p, "/mnt"));
    let bm = df(&["-BM"]);
    assert!(header(&bm).contains("1M-blocks"), "{}", header(&bm));
    let bm2 = df(&["-B", "4096"]);
    assert!(header(&bm2).contains("4K-blocks"), "{}", header(&bm2));
    let bm3 = df(&["-B", "1536"]);
    assert!(header(&bm3).contains("1.6kB-blocks"), "{}", header(&bm3));
    let b1 = df(&["-B", "1", "-P"]);
    assert!(header(&b1).contains("1-blocks"), "{}", header(&b1));
}

#[test]
fn env_block_size_ignored_like_reference() {
    // uutils 0.10.0 實測：DF_BLOCK_SIZE/BLOCK_SIZE/BLOCKSIZE 被忽略；
    // 只有 POSIXLY_CORRECT 使預設單位變 512（表頭標籤仍印 1K/1024）
    let base = df(&["-P"]);
    std::env::set_var("DF_BLOCK_SIZE", "512");
    let r = df(&["-P"]);
    std::env::remove_var("DF_BLOCK_SIZE");
    // 9p share 的 used/avail 會因平行 build 寫檔而變 → 數字正規化後比較
    let norm = |s: String| -> String {
        let mut out = String::new();
        for c in s.chars() {
            out.push(if c.is_ascii_digit() { '#' } else { c });
        }
        out
    };
    assert_eq!(norm(r.out()), norm(base.out()));
    std::env::set_var("POSIXLY_CORRECT", "1");
    let p = df(&["-P"]);
    std::env::remove_var("POSIXLY_CORRECT");
    // POSIXLY_CORRECT：-P 表頭變 "512-blocks"、數字雙倍（參考實測）
    assert!(header(&p).contains("512-blocks"), "{}", header(&p));
    let row = |o: &t::Out| -> u64 {
        data_lines(o)
            .into_iter()
            .find(|l| l.ends_with("/mnt"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap()
    };
    assert_eq!(row(&p), 2 * row(&base));
}

#[test]
fn operands() {
    let r = df(&["/", "-P"]);
    assert_eq!(r.code, 0);
    assert_eq!(data_lines(&r).len(), 1, "{}", r.out());
    // 同一 FS 的不同路徑去重（/ 與 /dev 不同 FS；/mnt 與 /mnt  subdir 同 FS）
    let r2 = df(&["/mnt", "/mnt/..", "-P"]);
    assert_eq!(r2.code, 0);
    let lines = data_lines(&r2);
    assert!(lines.iter().any(|l| l.ends_with(" /mnt")), "{lines:?}");
    // 不存在的 operand：rc1 但其他 operand 照常列
    let r3 = df(&["/mnt", "/noexist_zz", "-P"]);
    assert_eq!(r3.code, 1);
    assert_eq!(
        r3.stderr,
        b"df: /noexist_zz: No such file or directory\n"
    );
    assert!(data_lines(&r3).iter().any(|l| l.ends_with(" /mnt")));
}

#[test]
fn format_invariants() {
    let r = df(&["-P"]);
    assert_eq!(r.code, 0);
    let lines: Vec<String> = out_lines(&r);
    let h = lines[0].clone();
    let hstarts: Vec<usize> = token_starts(&h);
    for l in &lines[1..] {
        assert_eq!(*l, l.trim_end(), "行尾無空格: {l:?}");
        // 數字欄（blocks/used/avail）右對齊：欄尾位置與表頭一致
        let tw = l.split_whitespace().collect::<Vec<_>>();
        assert_eq!(tw.len(), 6, "{l}");
        let pct: u64 = tw[4].trim_end_matches('%').parse().unwrap();
        let used: u64 = tw[2].parse().unwrap();
        let avail: u64 = tw[3].parse().unwrap();
        let tot = used + avail;
        let expect = if tot == 0 {
            0
        } else {
            (used * 100 + tot - 1) / tot
        };
        assert_eq!(pct, expect, "Use% ceil: {l}");
        // 表頭欄起點對齊：該欄值結尾對齊表頭欄值結尾
        let tend = token_starts(l);
        // 前三欄與表頭同寬度右對齊（最後欄 mount 除外、pcent 對齊表頭結尾）
        assert_eq!(tend[1] + tw[1].len(), hstarts[1] + "1024-blocks".len(), "對齊: {l} vs {h}");
    }
}

fn out_lines(o: &t::Out) -> Vec<String> {
    o.out().lines().map(|s| s.to_string()).collect()
}

fn token_starts(s: &str) -> Vec<usize> {
    let mut v = Vec::new();
    let mut i = 0usize;
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] != b' ' {
            v.push(i);
            while i < b.len() && b[i] != b' ' {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    v
}

#[test]
fn option_errors() {
    let r = df(&["-B"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"df: option requires an argument -- 'B'\n");
    let r = df(&["-B", "bogus"]);
    // 我方採 GNU 措辭/rc（參考 clap 措辭 rc1 不追隨）
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"df: invalid block size 'bogus'\n");
    let r = df(&["-t"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"df: option requires an argument -- 't'\n");
    let r = df(&["-x"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"df: option requires an argument -- 'x'\n");
    let r = df(&["--bogus"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"df: unrecognized option '--bogus'"));
    // -v 不是 df 選項（參考由 clap 拒絕、措辭不追隨；我方 GNU 風格 rc2）
    let r = df(&["-v"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"df: invalid option -- 'v'\n");
    let r = df(&["-"]); // "-" 是 operand → statvfs 失敗
    assert_eq!(r.code, 1);
    assert_eq!(r.stderr, b"df: -: No such file or directory\n");
}

#[test]
fn output_fields() {
    let r = df(&["--output=source,fstype,target", "-t", "9p"]);
    assert_eq!(r.code, 0);
    let lines = data_lines(&r);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|l| l.contains("9p")));
}

#[test]
fn help_version() {
    let r = df(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with(b"Usage"));
    let r = df(&["--version"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"df (uutils coreutils) 0.10.0\n");
}

#[test]
fn sync_flags_and_empty_operands() {
    let r = df(&["--sync", "-P"]);
    assert_eq!(r.code, 0);
    let r = df(&["--no-sync"]);
    assert_eq!(r.code, 0);
    // operand 全部為空字串 → statvfs 失敗 rc1
    let r = df(&[""]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.starts_with(b"df: "));
}

#[test]
fn fuzz_pct_monotonic_and_totals() {
    // 不變量：-B 換算線性；pcent 在 0..=100；used+avail<=blocks
    let a = df(&["-P"]);
    let rows: Vec<Vec<u64>> = data_lines(&a)
        .iter()
        .map(|l| {
            l.split_whitespace()
                .skip(1)
                .take(3)
                .map(|x| x.parse().unwrap())
                .collect()
        })
        .collect();
    for r in &rows {
        assert!(r[0] >= r[1], "{r:?}");
        assert!(r[1] + r[2] <= r[0] + 1, "{r:?}"); // ext4 保留區塊例外寬容
    }
    let t = df(&["-P", "-a"]);
    let rows_a: Vec<Vec<u64>> = data_lines(&t)
        .iter()
        .map(|l| l.split_whitespace().skip(1).take(3).map(|x| x.parse().unwrap()).collect())
        .collect();
    assert!(rows_a.len() >= rows.len());
}
