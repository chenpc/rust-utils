//! pr(1) 測試（guest 內執行）。行為決定記錄於 docs/assumptions.md「pr」節：
//! - 頁首 5 行（日期頁碼、標題、3 空行）／頁尾 5 空行；-F 時頁首 3 行＋一個 \f。
//! - -l ≤ 頁首+頁尾深度 → 如同 -t。
//! - 單欄預設 -W 72 截斷（-J 除外）；多欄寬度：-w > (-s→512) > 72。
//! - \f 結尾的輸入行強制換頁（-T 解除）。
//! - 行號左填空至 width、超寬截左側數字。

use coreutils::testutil as t;
use std::path::PathBuf;

fn run(args: &[&str], input: &str) -> t::Out {
    t::run("pr", args, input)
}

fn fresh(tag: &str) -> PathBuf {
    let dir = t::workdir().join(format!("pr_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

fn write_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, content).expect("write");
    p
}

fn padw(s: &str, w: usize) -> String {
    format!("{:<width$}", s, width = w)
}

#[test]
fn help_and_version() {
    let h = run(&["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().starts_with("paginate or columnate FILE(s) for printing"));
    assert!(h.out().contains("--pages <FIRST_PAGE[:LAST_PAGE]>"));
    // -h 是 --header（man SYNOPSIS），help 只有 --help
    assert_eq!(run(&["--version"], "").out(), "pr (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["-V"], "").out(), "pr (uutils coreutils) 0.10.0\n");
}

#[test]
fn default_single_page_has_five_line_header_and_trailer() {
    let input = "1\n2\n3\n4\n5\n";
    let r = run(&[], input);
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    let lines: Vec<&str> = out.split('\n').collect();
    // 5 header + 5 text + 5 trailer + 終端空元素
    assert_eq!(lines.len(), 16, "{:?}", out);
    assert!(lines[0].ends_with("  (page 1)"), "{}", lines[0]);
    assert_eq!(lines[1], "-"); // stdin 的標題
    assert_eq!((lines[2], lines[3], lines[4]), ("", "", ""));
    assert_eq!(&lines[5..10], ["1", "2", "3", "4", "5"]);
    assert_eq!(&lines[10..15], ["", "", "", "", ""]);
    assert_eq!(lines[15], "");
}

#[test]
fn omit_header_exact_echo_and_small_length_suppression() {
    let input = "a\nb\nc\n";
    let r = run(&["-t"], input);
    assert_eq!(r.out(), input);
    // -l 9 ≤ 5+5 → 自動如同 -t
    let r2 = run(&["-l", "9"], input);
    assert_eq!(r2.out(), input);
    // -l 10 恰好 10 → 也抑制
    let r3 = run(&["-l", "10"], input);
    assert_eq!(r3.out(), input);
    // -l 11 → 頁首 5 + 文字 1 + 頁尾 5
    let r4 = run(&["-l", "11"], "x\n");
    let __o = r4.out();
    let lines: Vec<&str> = __o.split('\n').collect();
    assert_eq!(lines.len(), 12, "{:?}", r4.out());
    assert_eq!(lines[5], "x");
}

#[test]
fn pagination_multiple_pages() {
    let input: String = (1..=10).map(|i| format!("L{i}\n")).collect();
    // -l 14 → 每頁文字 4 行 → 3 頁（4+4+2）
    let r = run(&["-l", "14"], &input);
    let out = r.out();
    assert_eq!(out.matches("(page ").count(), 3, "{out}");
    for p in 1..=3 {
        assert!(out.contains(&format!("(page {p})")), "{out}");
    }
    // 頁尾空行都在：總行數 (5+4+5)*2 + (5+2+5) + 1 = 41
    assert_eq!(out.split('\n').count(), 41, "{out}");
    assert!(out.contains("L1\n") && out.contains("L10\n"));
}

#[test]
fn form_feed_pages() {
    let input: String = (1..=10).map(|i| format!("L{i}\n")).collect();
    // -F -l 8 → 每頁文字 4（8-4）→ 3 頁，每頁頁首 3 行 + \f
    let r = run(&["-F", "-l", "8"], &input);
    let out = r.out();
    assert_eq!(out.matches('\u{0C}').count(), 3, "{out}");
    assert_eq!(out.matches("(page ").count(), 3, "{out}");
    assert!(out.starts_with(&out.lines().next().unwrap().to_string()));
}

#[test]
fn pages_range() {
    let input: String = (1..=10).map(|i| format!("L{i}\n")).collect();
    // -t -l 4 → 每頁 4 行；--pages=2:3 → 印第 5..10 行
    let r = run(&["-t", "-l", "4", "--pages=2:3"], &input);
    assert_eq!(
        r.out(),
        "L5\nL6\nL7\nL8\nL9\nL10\n",
        "{}",
        r.out()
    );
    // 非 -t：頁碼計數照算，跳過的頁不輸出
    let r2 = run(&["-l", "14", "--pages=2:2"], &input);
    let out = r2.out();
    assert_eq!(out.matches("(page ").count(), 1, "{out}");
    assert!(out.contains("(page 2)"));
    assert!(out.contains("L5\n") && !out.contains("L1\n"));
}

#[test]
fn column_down_and_across() {
    let input = "a1\na2\na3\na4\na5\na6\n";
    // --column 2 -t：整檔分欄（down）：col0=[a1,a2,a3] col1=[a4,a5,a6]，欄寬 (72-1)/2=35
    let r = run(&["-t", "--column", "2"], input);
    let __o = r.out();
    let lines: Vec<&str> = __o.lines().collect();
    assert_eq!(lines.len(), 3, "{:?}", r.out());
    assert_eq!(lines[0], padw("a1", 35) + &padw("a4", 35));
    assert_eq!(lines[1], padw("a2", 35) + &padw("a5", 35));
    // -a：round-robin 填列
    let ra = run(&["-t", "-a", "--column", "2"], input);
    let __o = ra.out();
    let alines: Vec<&str> = __o.lines().collect();
    assert_eq!(alines[0], padw("a1", 35) + &padw("a2", 35));
    assert_eq!(alines[1], padw("a3", 35) + &padw("a4", 35));
    assert_eq!(alines[2], padw("a5", 35) + &padw("a6", 35));
    // -s 字元分隔 + -w 控制欄寬：(20-1)/2=9
    let rs = run(&["-t", "-a", "--column", "2", "-s", ",", "-w", "20"], input);
    let __o = rs.out();
    let slines: Vec<&str> = __o.lines().collect();
    assert_eq!(slines[0], padw("a1", 9) + "," + &padw("a2", 9));
    // 三欄
    let r3 = run(&["-t", "--column", "3", "-w", "30", "-a"], input);
    let __o = r3.out();
    let l3: Vec<&str> = __o.lines().collect();
    assert_eq!(l3.len(), 2);
    let cw = (30 - 2) / 3; // 9
    assert_eq!(l3[0], padw("a1", cw) + &padw("a2", cw) + &padw("a3", cw));
    assert_eq!(l3[1], padw("a4", cw) + &padw("a5", cw) + &padw("a6", cw));
}

#[test]
fn line_numbering() {
    let input = "x1\nx2\nx3\n";
    // 預設 width 5、TAB 分隔、左填空
    let r = run(&["-t", "-n"], input);
    assert_eq!(r.out(), "    1\tx1\n    2\tx2\n    3\tx3\n", "{}", r.out());
    // -n3,：寬 3、分隔 ','
    let r = run(&["-t", "-n3,"], input);
    assert_eq!(r.out(), "  1,x1\n  2,x2\n  3,x3\n", "{}", r.out());
    // -N100 起算 + -n2 超寬截左側
    let r = run(&["-t", "-n2", "-N999998"], input);
    assert_eq!(r.out(), "98\tx1\n99\tx2\n00\tx3\n", "{}", r.out());
}

#[test]
fn merge_files() {
    let dir = fresh("merge");
    let f1 = write_file(&dir, "f1", "A\nB\n");
    let f2 = write_file(&dir, "f2", "1\n2\n3\n");
    let p1 = f1.to_str().unwrap();
    let p2 = f2.to_str().unwrap();
    // -m -t：寬 72、兩檔 → 每欄 36，無 -S 時以補齊空格相接（檔案走 operands）
    let r = run(&["-m", "-t", p1, p2], "");
    let out = r.out();
    assert_eq!(out.lines().count(), 3, "{out}");
    assert_eq!(out.lines().next().unwrap(), padw("A", 36) + &padw("1", 36));
    assert_eq!(out.lines().nth(2).unwrap(), padw("", 36) + &padw("3", 36));
    // -J：不合併截斷、TAB 分隔
    let rj = run(&["-m", "-t", "-J", p1, p2], "");
    assert_eq!(rj.out().lines().next().unwrap(), "A\t1");
    // 頁首標題是兩檔名以空格相連（不給 -t 時）
    let rh = run(&["-m", "-l", "11", p1, p2], "");
    let __o = rh.out();
    let lines: Vec<&str> = __o.split('\n').collect();
    assert_eq!(lines[1], format!("{p1} {p2}"), "{:?}", lines);
    // -h 取代標題（不給 -t 時）
    let rt = run(&["-m", "-h", "CUSTOM", "-l", "11", p1, p2], "");
    let __o = rt.out();
    let lines: Vec<&str> = __o.split('\n').collect();
    assert_eq!(lines[1], "CUSTOM");
}

#[test]
fn indent_expand_width() {
    // -o 3：每行前置 3 空格（含 -t 與頁首）
    let r = run(&["-t", "-o", "3"], "a\nb\n");
    assert_eq!(r.out(), "   a\n   b\n");
    let rh = run(&["-l", "11", "-o", "3"], "a\n");
    let __o = rh.out();
    let lines: Vec<&str> = __o.split('\n').collect();
    assert!(lines[0].starts_with("   ") && lines[0].ends_with("(page 1)"), "{:?}", lines[0]);
    assert_eq!(lines[1], "   -");

    // -e：TAB 展開到 8
    let r = run(&["-t", "-e"], "a\tb\n\tc\n");
    assert_eq!(r.out(), "a       b\n        c\n", "{}", r.out());
    // -e4 自訂寬
    let r = run(&["-t", "-e4"], "a\tb\n");
    assert_eq!(r.out(), "a   b\n", "{}", r.out());

    // -W 截斷（單欄）與預設 72
    let long = "z".repeat(100);
    let r = run(&["-t", "-W", "10"], &long);
    assert_eq!(r.out(), "z".repeat(10) + "\n");
    let r = run(&["-t"], &long);
    assert_eq!(r.out(), "z".repeat(72) + "\n");
    let rj = run(&["-t", "-J"], &long);
    assert_eq!(rj.out(), long.clone() + "\n");
    // -d 雙行距
    let r = run(&["-t", "-d"], "a\nb\n");
    assert_eq!(r.out(), "a\n\nb\n\n");
}

#[test]
fn file_errors_and_warnings() {
    let dir = fresh("err");
    let good = write_file(&dir, "ok", "g1\n");
    let gp = good.to_str().unwrap();
    let r = run(&[gp, "/tmp/pr_no_such_file_xyz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "pr: /tmp/pr_no_such_file_xyz: No such file or directory\n");
    // 好檔仍輸出（預設頁首存在）
    assert!(r.out().contains("g1\n"));

    let r = run(&["-r", "/tmp/pr_no_such_file_xyz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "");

    // 每檔頁碼重新計算
    let f1 = write_file(&dir, "p1", &"1\n".repeat(60));
    let f2 = write_file(&dir, "p2", "x\n");
    let r = run(&[f1.to_str().unwrap(), f2.to_str().unwrap()], "");
    let out = r.out();
    assert!(out.contains("(page 1)") && out.contains("(page 2)"), "{out}");
    // 第二檔又從 page 1 開始 → "(page 1)" 出現兩次
    assert_eq!(out.matches("(page 1)").count(), 2, "{out}");
}

#[test]
fn form_feed_in_input_breaks_page() {
    let r = run(&["-l", "14"], "a\nb\u{0C}\nc\n");
    let out = r.out();
    assert!(out.contains("(page 1)") && out.contains("(page 2)"), "{out}");
    // -T：不換頁、保留原樣、無頁首
    let r = run(&["-T"], "a\nb\u{0C}\nc\n");
    assert_eq!(r.out(), "a\nb\u{0C}\nc\n");
    assert!(!r.out().contains("(page"));
}

#[test]
fn clap_style_errors() {
    let r = run(&["--bogus"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '--bogus' found\n\n"), "{}", r.err());
    assert!(r.err().contains("Usage: pr [OPTION]... [FILE]..."));

    let r = run(&["-Z"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '-Z' found\n\n"));

    let r = run(&["--length"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: a value is required for '--length <PAGE_LENGTH>' but none was supplied"), "{}", r.err());

    for (args, msg) in [
        (vec!["--column=abc"], "pr: invalid number of columns: 'abc'"),
        (vec!["--column=0"], "pr: invalid number of columns: '0'"),
        (vec!["--pages=abc"], "pr: invalid page range: 'abc'"),
        (vec!["-l0"], "pr: invalid page length: '0'"),
        (vec!["-W0"], "pr: invalid page width: '0'"),
        (vec!["-o", "xx"], "pr: invalid margin: 'xx'"),
    ] {
        let r = run(&args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        assert!(r.err().contains(msg), "{:?} -> {}", args, r.err());
    }

    // 長選項重複
    for args in [vec!["--column", "2", "--column", "3"], vec!["--merge", "--merge"]] {
        let r = run(&args, "");
        assert_eq!(r.code, 2, "{:?}", args);
        let opt = if args[0] == "--column" { "--column" } else { "--merge" };
        assert!(
            r.err().starts_with(&format!("error: the argument '{opt}' cannot be used multiple times\n\nFor more information, try '--help'.\n")),
            "{:?} -> {}",
            args,
            r.err()
        );
    }
    // 前綴解析：--omit-h → --omit-header
    let r = run(&["--omit-h"], "a\n");
    assert_eq!(r.out(), "a\n");
    // 附件值拒收
    let r = run(&["--merge=1"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected value '1' for '--merge' found"), "{}", r.err());
    // -- 之後 positional（即使是旗標樣式）
    let r = run(&["--", "-t"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("pr: -t: "), "{:?}", r.err());
}

#[test]
fn fuzz_invariants() {
    // 不變量 1：-t 單欄無 -d：輸出列數恒等輸入列數
    for n in [1usize, 3, 7, 120] {
        let input: String = (1..=n).map(|i| format!("row{i}\n")).collect();
        let r = run(&["-t"], &input);
        assert_eq!(r.out().lines().count(), n, "n={n}");
    }
    // 不變量 2：-t 多欄：輸出列數 = ceil(n/cols)
    for (n, cols) in [(6usize, 2usize), (7, 3), (10, 4), (5, 1)] {
        let input: String = (1..=n).map(|i| format!("r{i}\n")).collect();
        let r = run(&["-t", "--column", &cols.to_string()], &input);
        assert_eq!(r.out().lines().count(), (n + cols - 1) / cols, "n={n} cols={cols} out={}", r.out());
    }
    // 不變量 3：-o k → 每條輸出列以 k 空格起頭
    let r = run(&["-t", "-o", "4"], "a\nb\n");
    for line in r.out().lines() {
        assert!(line.starts_with("    "), "{line}");
    }
    // 不變量 4：任意選項組合不 panic（exit ∈ {0,1,2}）
    let pool = ["-t", "-T", "-d", "-a", "-F", "-J", "-m", "-n", "-e", "-l8", "-w40", "-W20", "-o2", "--column=2", "-s", "-S-", "-N5", "--pages=1:2", "-r", "-h=x", "--", "-x", "nosuchfile"];
    let mut st: u64 = 0xF00D;
    for iter in 0..80u64 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let k = (st >> 40) % 6 + 1;
        let mut args: Vec<&str> = Vec::new();
        for j in 0..k {
            args.push(pool[((st >> (j * 6)) % pool.len() as u64) as usize]);
        }
        let r = run(&args, "l1\nl2\nl3\n");
        assert!(r.code == 0 || r.code == 1 || r.code == 2, "iter {} {:?} -> {}", iter, args, r.code);
    }
}
