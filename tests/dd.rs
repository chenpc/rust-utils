//! dd 測試：xfer 速率欄 uutils 參考數值不穩定 → 只斷言結構 regex；
//! 其餘（records 計數、資料位元組、錯誤措辭）精確斷言。

use coreutils::testutil as t;

fn dd(args: &[&str], input: &[u8]) -> t::Out {
    t::run_bytes("dd", args, input)
}

/// 去除 stderr 的可變速率/時間欄，留下結構
fn stats_normalized(err: &str) -> Vec<String> {
    err.lines()
        .map(|l| {
            if let Some(i) = l.find(" copied, ") {
                l[..i + 9].to_string() + "<T>, <R>"
            } else {
                l.to_string()
            }
        })
        .collect()
}

#[test]
fn default_copy_small() {
    let r = dd(&[], b"abc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"abc");
    assert_eq!(
        stats_normalized(&r.err()),
        vec![
            "0+1 records in",
            "0+1 records out",
            "3 bytes copied, <T>, <R>"
        ]
    );
}

#[test]
fn full_partial_records() {
    // 預設 ibs=obs=512：1024 bytes → 2+0；1025 → 2+1
    let data = vec![b'x'; 1024];
    let r = dd(&[], &data);
    assert_eq!(r.err().lines().next().unwrap(), "2+0 records in");
    assert_eq!(r.stdout.len(), 1024);
    let r = dd(&[], &[vec![b'y'; 1025], vec![]].concat());
    assert_eq!(r.err().lines().next().unwrap(), "2+1 records in");
}

#[test]
fn bs_count_block_size() {
    let r = dd(&["bs=4", "count=2"], b"abcdefghij");
    assert_eq!(r.stdout, b"abcdefgh");
    assert_eq!(r.err().lines().next().unwrap(), "2+0 records in");
    let r = dd(&["count=2"], b"");
    assert_eq!(r.stdout, b"");
    assert_eq!(r.err().lines().next().unwrap(), "0+0 records in");
}

#[test]
fn ibs_obs_split() {
    let r = dd(&["ibs=3", "obs=2", "count=2"], b"123456");
    assert_eq!(r.stdout, b"123456");
    // 2 full in, 3 full out
    assert_eq!(r.err().lines().next().unwrap(), "2+0 records in");
    assert_eq!(r.err().lines().nth(1).unwrap(), "3+0 records out");
}

#[test]
fn status_variants() {
    let r = dd(&["status=noxfer"], b"abc");
    assert_eq!(r.stdout, b"abc");
    assert_eq!(
        stats_normalized(&r.err()),
        vec!["0+1 records in", "0+1 records out"]
    );
    let r = dd(&["status=none"], b"abc");
    assert_eq!(r.stdout, b"abc");
    assert_eq!(r.err(), "");
}

#[test]
fn case_conversions() {
    let r = dd(&["conv=ucase"], b"abZ9\n");
    assert_eq!(r.stdout, b"ABZ9\n");
    let r = dd(&["conv=lcase"], b"AbZ9\n");
    assert_eq!(r.stdout, b"abz9\n");
}

#[test]
fn swab() {
    let r = dd(&["conv=swab"], b"ab\n");
    assert_eq!(r.stdout, b"ba\n");
    // 跨記錄：ibs=2 兩筆 "ab","cd" → "ba","dc"
    let r = dd(&["ibs=2", "conv=swab"], b"abcd");
    assert_eq!(r.stdout, b"badc");
    // 奇數殘留單 byte 直接吐出（不補 NUL）
    let r = dd(&["ibs=2", "conv=swab"], b"abcde");
    assert_eq!(r.stdout, b"badce");
}

#[test]
fn conv_sync() {
    // ibs=4, sync：短記錄補 NUL 至 ibs
    // uutils：EOF 即停（不補滿 count），短讀補 NUL 至 ibs 但計數仍 partial
    let r = dd(&["ibs=4", "conv=sync", "count=2"], b"ab");
    assert_eq!(r.stdout, b"ab\0\0");
    assert_eq!(r.err().lines().next().unwrap(), "0+1 records in");
}

#[test]
fn block_unblock() {
    // conv=block cbs=4："ab\ncd\n" → "ab  cd  "（去 \n 補空格）
    // uutils 實測：串流以 \n 切線，每線補空格至 cbs
    let r = dd(&["cbs=4", "conv=block"], b"ab\ncd\n");
    assert_eq!(r.stdout, b"ab  cd  ");
    let r = dd(&["ibs=4", "cbs=3", "conv=block"], b"abcd\nefgh\n");
    assert_eq!(r.stdout, b"abcefg");
    assert!(r.err().contains("2 truncated records"), "{}", r.err());
    // conv=unblock cbs=2："abcdef\n" → 每 2 字元一块、去尾空格加 \n
    // 串流 cbs 切塊：ab|cd|ef|\n，每塊去尾空格加 \n（"\n" 塊 → "\n\n"）
    let r = dd(&["cbs=2", "conv=unblock"], b"abcdef\n");
    assert_eq!(r.stdout, b"ab\ncd\nef\n\n\n");
    // 無 cbs → 錯誤
    let r = dd(&["conv=block"], b"x");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "dd: conv=block or conv=unblock specified without cbs=N\n"
    );
    // 記錄超過 cbs → 截斷 + truncated 訊息
    let r = dd(&["cbs=2", "conv=block"], b"abcd\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"ab");
    let e = r.err();
    assert!(e.contains("1 truncated record"), "{e}");
}

#[test]
fn conv_notrunc() {
    let dir = "/tmp/ddt_nt";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/out"), b"HELLOWORLD").unwrap();
    let r = dd(&["conv=notrunc", &format!("of={dir}/out")], b"ab");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read(format!("{dir}/out")).unwrap(), b"abLLOWORLD");
    // 預設截斷
    std::fs::write(format!("{dir}/out"), b"HELLOWORLD").unwrap();
    let _r = dd(&[&format!("of={dir}/out")], b"ab");
    assert_eq!(std::fs::read(format!("{dir}/out")).unwrap(), b"ab");
    // conv=excl + 已存在檔案 → rc1
    let r = dd(&["conv=excl", &format!("of={dir}/out")], b"x");
    assert_eq!(r.code, 1);
    let e = r.err();
    assert!(e.contains(&format!("{dir}/out")), "{e}");
}

#[test]
fn seek_skip() {
    let dir = "/tmp/ddt_seek";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/in"), b"0123456789abcdefghij").unwrap();
    // skip=1 bs=10 → 從 10 讀起
    let r = dd(&["bs=10", "count=1", "skip=1", &format!("if={dir}/in")], b"");
    assert_eq!(r.stdout, b"abcdefghij");
    // iflag=skip_bytes
    let r = dd(
        &["ibs=4", "count=2", "iflag=skip_bytes", "skip=2", &format!("if={dir}/in")],
        b"",
    );
    assert_eq!(r.stdout, b"23456789");
    // seek=2 obs=4 → 輸出前補 8 個 NUL
    let r = dd(
        &["bs=4", "count=1", "oflag=seek_bytes", "seek=2", &format!("of={dir}/o1")],
        b"abcd",
    );
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read(format!("{dir}/o1")).unwrap(), b"\0\0abcd");
    // iflag=count_bytes：count=5 → 5 bytes
    let r = dd(
        &["ibs=4", "iflag=count_bytes", "count=5", &format!("if={dir}/in")],
        b"",
    );
    assert_eq!(r.stdout, b"01234");
    // stdin skip：讀掉丟棄
    let r = dd(&["ibs=2", "count=1", "skip=1"], b"xdata");
    assert_eq!(r.stdout, b"at");
}

#[test]
fn append_oflag() {
    let dir = "/tmp/ddt_app";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/out"), b"AB").unwrap();
    let r = dd(&["oflag=append", &format!("of={dir}/out")], b"CD");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read(format!("{dir}/out")).unwrap(), b"ABCD");
}

#[test]
fn fsync_convs() {
    let dir = "/tmp/ddt_sync";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let r = dd(&["conv=fsync", &format!("of={dir}/o")], b"xx");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read(format!("{dir}/o")).unwrap(), b"xx");
    let r = dd(&["conv=fdatasync", &format!("of={dir}/o2")], b"yy");
    assert_eq!(r.code, 0);
}

#[test]
fn if_missing_error() {
    let r = dd(&["if=/tmp/noexist_zz"], b"");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "dd: failed to open '/tmp/noexist_zz': No such file or directory"
    );
}

#[test]
fn unrecognized_operands() {
    let r = dd(&["foo=bar"], b"x");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "dd: Unrecognized operand 'foo=bar'\n");
    let r = dd(&["conv=bogus"], b"x");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "dd: Unrecognized conv=CONV -> bogus\n");
    // 本 build 不支援的 conv
    for c in ["ascii", "ebcdic", "ibm", "sparse"] {
        let r = dd(&[&format!("conv={c}")], b"x");
        assert_eq!(r.code, 1);
        assert_eq!(
            r.err(),
            format!("dd: conversion '{c}' is not supported in this build\n")
        );
    }
    // iflag/oflag 未知值（U+2018/2019 引號措辭）
    let r = dd(&["iflag=bogus"], b"x");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("dd: invalid input flag: \u{2018}bogus\u{2019}"), "{}", r.err());
    let r = dd(&["oflag=bogus"], b"x");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("dd: invalid output flag: \u{2018}bogus\u{2019}"), "{}", r.err());
    // 无效旗標
    let r = dd(&["ibs=abc"], b"x");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "dd: invalid number 'abc'\n");
}

#[test]
fn help_version() {
    let r = dd(&["--help"], b"");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"));
    let r = dd(&["--version"], b"");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"dd (uutils coreutils) 0.10.0\n");
}

#[test]
fn xfer_line_format() {
    // 3072 bytes → "(3.1 kB, 3.0 KiB)"（SI 與 IEC 各固定一位小數）
    // status=none 不印 xfer 行；xfer 行用固定 stdin 斷言
    let r = dd(&["status=none"], &vec![b'z'; 3072]);
    assert_eq!(r.stderr, b"");
    let r = dd(&[], &vec![b'z'; 3072]);
    let e = r.err();
    let l = e.lines().find(|l| l.contains("copied")).unwrap();
    assert!(l.starts_with("3072 bytes (3.1 kB, 3.0 KiB) copied, "), "{l}");
    // <512 bytes 無括號
    let r = dd(&[], b"abc");
    assert!(r.err().contains("3 bytes copied"), "{}", r.err());
}

#[test]
fn multi_conv_chain() {
    // swab + ucase 同時：先大小寫後交換（uutils 順序：ucase 套用在位元組上，swab 交换）
    let r = dd(&["conv=swab,ucase"], b"ab");
    assert_eq!(r.stdout, b"BA");
}

#[test]
fn conv_sync_truncated_records_absent() {
    // conv=sync 不產生 truncated 訊息
    let r = dd(&["ibs=8", "conv=sync", "count=1"], b"short");
    assert!(!r.err().contains("truncated"), "{}", r.err());
    assert_eq!(r.stdout, b"short\0\0\0");
}

#[test]
fn fuzz_identity_invariants() {
    // 不變量 1：任意位元組，bs=k、count 省略 → 輸出 == 輸入（搬運不分塊）
    // 不變量 2：count=n bs=k → 輸出 == input[..min(n*k,len)]
    // 不變量 3：conv=swab 兩次 == 原輸入（偶數長度）
    // 不變量 4：conv=sync（ibs=k）輸出長度為 k 的倍數且前綴 = 輸入
    use t::Rng;
    let mut rng = Rng::new(20260730);
    for _ in 0..8 {
        let n = 1 + rng.below(2000) as usize;
        let data: Vec<u8> = (0..n).map(|_| (rng.below(256)) as u8).collect();
        let k = 1 + rng.below(64) as usize;

        let r = dd(&[&format!("bs={k}")], &data);
        assert_eq!(r.code, 0);
        assert_eq!(r.stdout, data.as_slice(), "bs={k} n={n}");

        let cn = 1 + rng.below(5) as usize;
        let r = dd(&[&format!("bs={k}"), &format!("count={cn}")], &data);
        let want = std::cmp::min(cn * k, n);
        assert_eq!(r.stdout, &data[..want], "bs={k} count={cn}");

        if n % 2 == 0 {
            let once = dd(&["conv=swab"], &data);
            let twice = dd(&["conv=swab"], &once.stdout);
            assert_eq!(twice.stdout, data.as_slice(), "swab^2 n={n}");
        }

        let r = dd(
            &[&format!("ibs={k}"), "conv=sync", "status=none"],
            &data,
        );
        assert_eq!(r.stdout.len() % k, 0, "sync len {} k={k}", r.stdout.len());
        assert!(r.stdout.starts_with(&data), "sync prefix k={k} n={n}");
    }
}

#[test]
fn fuzz_block_unblock_roundtrip() {
    // 不變量：conv=block cbs=k 後 conv=unblock cbs=k，輸入行皆短於 k 且無尾隨空格
    // → 還原原始行（去掉行尾 \n 後每行補回）
    use t::Rng;
    let mut rng = Rng::new(20260731);
    for _ in 0..6 {
        let k = 4 + rng.below(12) as usize;
        let nl = 1 + rng.below(4) as usize;
        let mut input = Vec::new();
        for _ in 0..nl {
            let len = rng.below((k - 1) as u64) as usize;
            let word: Vec<u8> = (0..len)
                .map(|_| b'a' + (rng.below(26) as u8))
                .collect();
            input.extend_from_slice(&word);
            input.push(b'\n');
        }
        let blocked = dd(&[&format!("cbs={k}"), "conv=block", "status=none"], &input);
        assert_eq!(blocked.stdout.len(), nl * k, "block len k={k}");
        let back = dd(&[&format!("cbs={k}"), "conv=unblock", "status=none"], &blocked.stdout);
        assert_eq!(back.stdout, input, "roundtrip k={k} input={:?}", String::from_utf8_lossy(&input));
    }
}

#[test]
fn invalid_numbers_every_key() {
    for key in ["bs", "ibs", "obs", "cbs", "count", "skip", "seek"] {
        let r = dd(&[&format!("{key}=bogus")], b"x");
        assert_eq!(r.code, 1, "{key}");
        assert_eq!(r.err(), format!("dd: invalid number 'bogus'\n"), "{key}");
    }
    let r = dd(&["status=bogus"], b"x");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "dd: invalid status state 'bogus'\n");
    // block 與 unblock 不能同時
    let r = dd(&["conv=block,unblock", "cbs=2"], b"x\n");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "dd: conv=block and conv=unblock are not both supported\n"
    );
    // if= 目錄
    let dir = "/tmp/ddt_dir";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let r = dd(&[&format!("if={dir}")], b"");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), format!("dd: failed to open '{dir}': Is a directory\n"));
}

#[test]
fn seek_without_of_stdout() {
    // 無 of：stdout（t::run 的 Vec）不可 seek → 寫 NUL 補齊
    let r = dd(&["bs=4", "count=1", "seek=2", "status=none"], b"abcd");
    assert_eq!(r.stdout, b"\0\0\0\0\0\0\0\0abcd");
    // oflag=seek_bytes 無 of
    let r = dd(&["bs=4", "count=1", "seek=3", "oflag=seek_bytes", "status=none"], b"abcd");
    assert_eq!(r.stdout, b"\0\0\0abcd");
}

#[test]
fn block_eof_flush_partial_line() {
    // 無尾 \n 的輸入：EOF 時殘留線也補齊至 cbs
    let r = dd(&["cbs=5", "conv=block", "status=none"], b"abc");
    assert_eq!(r.stdout, b"abc  ");
    // 混雜：有 \n 的線 + EOF 殘線
    let r = dd(&["cbs=3", "conv=block", "status=none"], b"ab\nc");
    assert_eq!(r.stdout, b"ab c  ");
    // 超長殘線 EOF → 截斷 + truncated
    let r = dd(&["cbs=3", "conv=block"], b"abcde");
    assert_eq!(r.stdout, b"abc");
    assert!(r.err().contains("1 truncated record"), "{}", r.err());
}

#[test]
fn write_error_branch() {
    // FailWriter：stdout 寫到 5 byte 後失敗 → 進入 write_fail、rc1
    let mut fw = t::FailWriter::new(5);
    let mut err = Vec::new();
    let data = vec![b'q'; 1000];
    let code = t::run_with("dd", &["bs=100", "status=none"], &data, &mut fw, &mut err);
    assert_eq!(code, 1);
    assert_eq!(fw.out.len(), 5);
    let msg = String::from_utf8_lossy(&err);
    assert!(msg.contains("dd: writing to 'standard output'"), "{msg}");
    // conv=noerror 時 write_fail 不設 rc（實作把 noerror 同時套用於讀寫錯誤）
    let mut fw2 = t::FailWriter::new(3);
    let mut err2 = Vec::new();
    let code = t::run_with("dd", &["bs=100", "conv=noerror", "status=none"], &data, &mut fw2, &mut err2);
    assert_eq!(code, 0);
    assert_eq!(fw2.out.len(), 3);
}

#[test]
fn truncated_records_plural_and_stats() {
    // 兩筆超長 → "2 truncated records"，位置在 records out 之後、copied 之前
    let r = dd(&["cbs=2", "conv=block"], b"abcd\nefg\n");
    let estr = r.err();
    let e: Vec<&str> = estr.lines().collect();
    assert_eq!(e[0], "0+1 records in");
    assert_eq!(e[1], "0+1 records out");
    assert_eq!(e[2], "2 truncated records");
    assert!(e[3].starts_with("4 bytes copied"), "{:?}", e[3]);
}
