//! date e2e tests（guest 內執行）。
//! 期望值全由我方 spec（docs/assumptions.md「date」節）+ 公開曆算驗證，不比对參考實作。
use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("date", args, "")
}

/// TZ 覆蓋必須走子行程（in-process set_var 會跟 cargo 多執行緒打架）。
fn run_tz(args: &[&str], tz: &str) -> t::Out {
    let bin = std::env::var("COREUTILS_BIN").unwrap_or_else(|_| "coreutils".into());
    let mut cmd = std::process::Command::new(&bin);
    if std::env::var("COREUTILS_BIN").is_ok() {
        cmd.arg("date");
    }
    cmd.args(args);
    cmd.env("TZ", tz);
    let out = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|p| p.wait_with_output())
        .expect("spawn");
    t::Out {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

#[test]
fn help_and_version() {
    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().contains("Usage: date [OPTION]... [FORMAT]"));
    let v = run(&["-V"]);
    assert_eq!(v.code, 0);
    assert!(v.out().starts_with("date (uutils coreutils)"));
}

#[test]
fn default_format_shape() {
    // 預設格式：%a %b %e %H:%M:%S %Z %Y（guest TZ 未設 → UTC）
    let r = run(&[]);
    assert_eq!(r.code, 0, "{}", r.err());
    let __o = r.out();
    let toks: Vec<&str> = __o.split_whitespace().collect();
    assert_eq!(toks.len(), 6, "{__o}");
    assert!(
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].contains(&toks[0]),
        "{__o}"
    );
    assert_eq!(toks[4], "UTC");
    assert_eq!(toks[5].len(), 4);
    assert!(toks[3].matches(':').count() >= 2, "{__o}");
    assert_eq!(r.out().lines().count(), 1);
}

#[test]
fn epoch_roundtrip_and_resolution() {
    for e in [0i64, 946684800, -86400, 1757721600, 3000000000, -3000000000] {
        let r = run(&["-u", "-d", &format!("@{e}"), "--format", "%s"]);
        assert_eq!(r.code, 0, "{e} {}", r.err());
        assert_eq!(r.out().trim(), e.to_string());
    }
    let res = run(&["--resolution"]);
    assert_eq!(res.code, 0);
    assert_eq!(res.out(), "0.000000001\n");
}

#[test]
fn strftime_table_epoch2000() {
    // @946684800 = 2000-01-01T00:00:00Z（Sat）
    let one = |fmt: &str, epoch: i64| -> String {
        let r = run(&["-u", "-d", &format!("@{epoch}"), "--format", fmt]);
        assert_eq!(r.code, 0, "{fmt} {}", r.err());
        r.out()
    };
    assert_eq!(one("%F", 946684800), "2000-01-01\n");
    assert_eq!(one("%a", 946684800), "Sat\n");
    assert_eq!(one("%A", 946684800), "Saturday\n");
    assert_eq!(one("%b", 946684800), "Jan\n");
    assert_eq!(one("%B", 946684800), "January\n");
    assert_eq!(one("%h", 946684800), "Jan\n");
    assert_eq!(one("%j", 946684800), "001\n");
    assert_eq!(one("%V", 946684800), "52\n"); // 2000-01-01 屬於 1999-W52
    assert_eq!(one("%G", 946684800), "1999\n");
    assert_eq!(one("%g", 946684800), "99\n");
    assert_eq!(one("%U", 946684800), "00\n");
    assert_eq!(one("%W", 946684800), "00\n");
    assert_eq!(one("%C", 946684800), "20\n");
    assert_eq!(one("%y", 946684800), "00\n");
    assert_eq!(one("%u", 946684800), "6\n");
    assert_eq!(one("%w", 946684800), "6\n");
    assert_eq!(one("%I", 946684800), "12\n");
    assert_eq!(one("%p", 946684800), "AM\n");
    assert_eq!(one("%P", 946684800), "am\n");
    assert_eq!(one("%T", 946684800), "00:00:00\n");
    assert_eq!(one("%R", 946684800), "00:00\n");
    assert_eq!(one("%r", 946684800), "12:00:00 AM\n");
    assert_eq!(one("%D", 946684800), "01/01/00\n");
    assert_eq!(one("%c", 946684800), "Sat Jan  1 00:00:00 2000\n");
    assert_eq!(one("%%", 946684800), "%\n");
    assert_eq!(one("%e", 946684800), " 1\n");
    assert_eq!(one("%z", 946684800), "+0000\n");
    assert_eq!(one("%Z", 946684800), "UTC\n");
    assert_eq!(one("%n", 946684800), "\n\n"); // %n 換行 + 格式結尾的 newline
    assert_eq!(one("%t", 946684800), "\t\n");
    // ISO 週邊界：2017-01-01（Sun）→ 2016-W52；2009-01-01（Thu）→ 2009-W01
    assert_eq!(one("%V %G", 1483228800), "52 2016\n");
    assert_eq!(one("%V %G", 1230768000), "01 2009\n");
    // 閏年：2000-03-01 = doy 61；2000-12-31 = 366
    assert_eq!(one("%j", 951868800), "061\n");
    assert_eq!(one("%j", 978220800), "366\n");
}

#[test]
fn padding_flags() {
    // @946771200 = 2000-01-02T00:00:00Z
    let f = |fmt: &str| -> String {
        let r = run(&["-u", "-d", "@946771200", "--format", fmt]);
        assert_eq!(r.code, 0, "{fmt} {}", r.err());
        r.out()
    };
    assert_eq!(f("%-d"), "2\n");
    assert_eq!(f("%_d"), " 2\n");
    assert_eq!(f("%03d"), "002\n");
    assert_eq!(f("%-3d"), "2  \n");
    assert_eq!(f("%_3d"), "  2\n");
    assert_eq!(f("%d"), "02\n");
    assert_eq!(f("%-H"), "0\n");
    assert_eq!(f("%05j"), "00002\n"); // j 預設 3，寬度 5 覆寫
}

#[test]
fn iso8601_forms() {
    let r = run(&["-u", "-d", "@1757721600", "-Iseconds"]);
    assert_eq!(r.out().trim(), "2025-09-13T00:00:00+00:00");
    let r = run(&["-u", "-d", "@1757721600", "--iso-8601=minutes"]);
    assert_eq!(r.out().trim(), "2025-09-13T00:00");
    let r = run(&["-u", "-d", "@1757721600", "-I"]);
    assert_eq!(r.out().trim(), "2025-09-13");
    let r = run(&["-u", "-d", "@1757721600", "-I=hours"]);
    assert_eq!(r.out().trim(), "2025-09-13T00:00+00:00");
    let r = run(&["-u", "-d", "@1757721600", "--iso-8601=ns"]);
    assert_eq!(r.out().trim(), "2025-09-13T00:00:00.000000000+00:00");
    // FORMAT operand 優先於 -I（我方決定）
    let r = run(&["-u", "-d", "@1757721600", "-I", "%F"]);
    assert_eq!(r.out().trim(), "2025-09-13");
    // 非法 FMT → clap 值驗證 2
    for bad in ["-I=bogus", "--iso-8601=hoursx"] {
        let r = run(&[bad, "-d", "@0"]);
        assert_eq!(r.code, 2, "{bad}: {}", r.err());
    }
}

#[test]
fn rfc_email_and_rfc3339() {
    let r = run(&["-u", "-R", "-d", "@946684800"]);
    assert_eq!(r.out().trim(), "Sat, 01 Jan 2000 00:00:00 +0000");
    let r = run(&["-u", "--rfc-email", "-d", "@946684800"]);
    assert_eq!(r.out().trim(), "Sat, 01 Jan 2000 00:00:00 +0000");
    let r = run(&["-u", "--rfc-3339=seconds", "-d", "@946684800"]);
    assert_eq!(r.out().trim(), "2000-01-01 00:00:00+00:00");
    let r = run(&["-u", "--rfc-3339=date", "-d", "@946684800"]);
    assert_eq!(r.out().trim(), "2000-01-01");
    let r = run(&["--rfc-3339=bogus", "-d", "@0"]);
    assert_eq!(r.code, 2);
    let r = run(&["--rfc-3339"]);
    assert_eq!(r.code, 2); // 值必須用 = 附著或下一 token？clap: --rfc-3339 需要值
}

#[test]
fn explicit_suffix_and_relative_words() {
    // 顯式後綴：epoch 不受 TZ 影響
    let r = run(&["-d", "2000-01-01T05:30:00+05:30", "--format", "%s"]);
    assert_eq!(r.out().trim(), "946684800");
    let r = run(&["-u", "-d", "2000-01-01T05:30:00+05:30", "--format", "%F %T"]);
    assert_eq!(r.out().trim(), "2000-01-01 00:00:00");
    // 空格分隔的日期時間
    let r = run(&["-u", "-d", "2000-01-01 05:30:00", "--format", "%F %T"]);
    assert_eq!(r.out().trim(), "2000-01-01 05:30:00");
    // 只有日期 → 當地 00:00:00（-u 下就是 UTC 午夜）
    let r = run(&["-u", "-d", "2000-01-01", "--format", "%F %T"]);
    assert_eq!(r.out().trim(), "2000-01-01 00:00:00");
    // 只有時間 → 今天的日期、指定時刻
    let r = run(&["-u", "-d", "12:34:56", "--format", "%T"]);
    assert_eq!(r.out().trim(), "12:34:56");
    let today = run(&["-u", "--format", "%F"]);
    let r = run(&["-u", "-d", "12:34:56", "--format", "%F"]);
    assert_eq!(r.out(), today.out());
    // today/yesterday/tomorrow：保留當前時刻，日期 ±1 天
    let t0 = run(&["-u", "-d", "today", "--format", "%s"]).out();
    let y = run(&["-u", "-d", "yesterday", "--format", "%s"]).out();
    let tm = run(&["-u", "-d", "tomorrow", "--format", "%s"]).out();
    let d = |s: &str| s.trim().parse::<i64>().unwrap();
    assert_eq!(d(&t0) - d(&y), 86400);
    assert_eq!(d(&tm) - d(&t0), 86400);
    // 小寫 t 當分隔符
    let r = run(&["-u", "-d", "2000-01-01t12:00", "--format", "%T"]);
    assert_eq!(r.out().trim(), "12:00:00");
}

#[test]
fn tz_env_fixed_offsets() {
    // TZ=固定偏移：裸 epoch 的顯示位移，%s 恆定
    let r = run_tz(&["-d", "@946684800", "--format", "%F %T %z %Z %s"], "+05:30");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim(), "2000-01-01 05:30:00 +0530 <+05:30> 946684800");
    let r = run_tz(&["-d", "@946684800", "--format", "%F %T %z %Z"], "-0700");
    assert_eq!(r.out().trim(), "1999-12-31 17:00:00 -0700 <-07:00>");
    let r = run_tz(&["-d", "@946684800", "--format", "%Z %z"], "UTC");
    assert_eq!(r.out().trim(), "UTC +0000");
    // 非固定偏移名稱（guest 無 tzdata）→ 退回系統 local（guest local = UTC）
    let r = run_tz(&["-d", "@946684800", "--format", "%F %T %s"], "America/New_York");
    assert_eq!(r.out().trim(), "2000-01-01 00:00:00 946684800");
    // -u 壓過 TZ
    let r = run_tz(&["-u", "-d", "@946684800", "--format", "%z"], "+05:30");
    assert_eq!(r.out().trim(), "+0000");
    // 裸日期字串在 TZ 下以該 offset 解釋
    let r = run_tz(&["-d", "2000-01-01", "--format", "%s"], "+05:30");
    assert_eq!(r.out().trim(), "946665000");
}

#[test]
fn reference_file_mtime() {
    let dir = t::workdir();
    let f = dir.join("date_ref.txt");
    std::fs::write(&f, "hello\n").expect("write");
    let md = std::fs::metadata(&f).expect("metadata");
    use std::os::unix::fs::MetadataExt;
    let r = run(&["-r", f.to_str().unwrap(), "--format", "%s"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim().parse::<i64>().unwrap(), md.mtime());
    let missing = dir.join("nope");
    let r = run(&["-r", missing.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("No such file or directory"), "{}", r.err());
}

#[test]
fn datefile_lines_and_comments() {
    let dir = t::workdir();
    let f = dir.join("dates.txt");
    std::fs::write(&f, "@0\n# comment line\n2000-01-01 00:00:00\n\n@60\n").expect("write");
    let r = run(&["-u", "-f", f.to_str().unwrap(), "--format", "%s"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "0\n946684800\n60\n");
    // DATEFILE 裡有非法行 → rc 1
    let bad = dir.join("bad.txt");
    std::fs::write(&bad, "@0\nnot-a-date\n").expect("write");
    let r = run(&["-f", bad.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid date 'not-a-date'"), "{}", r.err());
    // -f 與 -d 同時給：依序輸出（f 先 d 後；我方決定）
    let r = run(&["-u", "-f", f.to_str().unwrap(), "-d", "@7", "--format", "%s"]);
    assert_eq!(r.out(), "0\n946684800\n60\n7\n");
    let r = run(&["-f", dir.join("missing.txt").to_str().unwrap()]);
    assert_eq!(r.code, 1);
}

#[test]
fn invalid_dates_and_clap_errors() {
    // 非法 date 字串 → rc 1 + GNU 訊息形狀
    for bad in ["not-a-date", "2000-13-01", "2000-01-32", "99:99:99", ""] {
        let r = run(&["-d", bad]);
        assert_eq!(r.code, 1, "{bad}");
        assert!(r.err().contains(&format!("date: invalid date '{bad}'")), "{}", r.err());
    }
    // clap 錯誤群 → rc 2
    assert_eq!(run(&["--nope"]).code, 2);
    assert_eq!(run(&["-x"]).code, 2);
    assert_eq!(run(&["-d"]).code, 2); // 缺值
    assert_eq!(run(&["--date"]).code, 2);
    assert_eq!(run(&["--date=x", "--date=y"]).code, 2); // 重複
    assert_eq!(run(&["--rfc-email", "-R"]).code, 0); // 短/長同旗標不報 dup（我方決定）
    assert_eq!(run(&["%F", "%T"]).code, 2); // 兩個 FORMAT operand
    assert_eq!(run(&["--", "%F", "%T"]).code, 2);
    assert_eq!(run(&["--", "--format", "%T"]).code, 2); // -- 之後不解析旗標
    // 歧義前綴 → tip（resolve_long 慣例）；唯一前綴 → 直接接受
    let r = run(&["--r"]);
    assert_eq!(r.code, 2);
    assert!(r.err().contains("tip: a similar argument exists: '--reference'"), "{}", r.err());
    let r = run(&["--univer", "-d", "@946684800", "--format", "%F"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim(), "2000-01-01");
    // -- 之後一律是 operand
    let r = run(&["-u", "-d", "@0", "--", "%F"]);
    assert_eq!(r.out().trim(), "1970-01-01");
    // -R 短旗標可與其他集群併用
    let r = run(&["-uR", "-d", "@946684800"]);
    assert_eq!(r.out().trim(), "Sat, 01 Jan 2000 00:00:00 +0000");
}

#[test]
fn debug_annotates_parse() {
    let r = run(&["-d", "@123", "--debug", "--format", "%s"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim(), "123");
    assert_eq!(r.err(), "date: parsed '@123' as epoch 123\n");
}

#[test]
fn set_time_to_current_is_noop() {
    // -s：guest 為 root，settimeofday 可成功；把鐘設回「現在」不影響其他測試
    let now: i64 = run(&["-u", "--format", "%s"]).out().trim().parse().unwrap();
    let r = run(&["-s", &format!("@{now}")]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "");
    let after: i64 = run(&["-u", "--format", "%s"]).out().trim().parse().unwrap();
    assert!((after - now).abs() <= 5, "clock moved {now} -> {after}");
    let bad = run(&["-s", "definitely-not-a-date"]);
    assert_eq!(bad.code, 1);
    assert!(bad.err().contains("invalid date"), "{}", bad.err());
}

#[test]
fn fuzz_invariants() {
    // 不變量 1：%s 與輸入 epoch 逐個相等；-Iseconds 可再解析回同一 epoch（round-trip）
    let mut st: u64 = 0xC0FFEE;
    for _ in 0..150 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let e = ((st >> 8) as i64).rem_euclid(6_000_000_000) - 3_000_000_000;
        let r = run(&["-u", "-d", &format!("@{e}"), "--format", "%s"]);
        assert_eq!(r.code, 0, "{e}");
        assert_eq!(r.out().trim().parse::<i64>().unwrap(), e, "epoch {e}");
        let iso = run(&["-u", "-d", &format!("@{e}"), "-Iseconds"]).out();
        let iso = iso.trim();
        let b = iso.as_bytes();
        assert_eq!(b.len(), 25, "{iso}");
        assert!(b[0..4].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[4], b'-');
        assert!(b[5..7].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[7], b'-');
        assert!(b[8..10].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[10], b'T');
        assert!(b[11..13].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[13], b':');
        assert!(b[14..16].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[16], b':');
        assert!(b[17..19].iter().all(|c| c.is_ascii_digit()));
        assert!(b[19] == b'+' || b[19] == b'-');
        assert!(b[20..22].iter().all(|c| c.is_ascii_digit()));
        assert_eq!(b[22], b':');
        assert!(b[23..25].iter().all(|c| c.is_ascii_digit()));
        let back = run(&["-u", "-d", iso, "--format", "%s"]);
        assert_eq!(back.out().trim().parse::<i64>().unwrap(), e, "round-trip {iso}");
        // 不變量 2：%j ∈ 1..366
        let j: i64 = run(&["-u", "-d", &format!("@{e}"), "--format", "%j"])
            .out()
            .trim()
            .parse()
            .unwrap();
        assert!((1..=366).contains(&j), "{e} {j}");
    }
    // 不變量 3：任意垃圾字串不 panic，rc ∈ {1,2}；且單筆輸入恆 1 行輸出
    let alphabet = "0123456789-:+ TZHMSTYabcr%.x/@#".chars().collect::<Vec<char>>();
    let mut st2: u64 = 0xBADF00D;
    for _ in 0..120 {
        st2 = st2.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st2 >> 50) % 12 + 1;
        let mut s = String::new();
        for k in 0..n {
            s.push(alphabet[((st2 >> (k * 5)) % alphabet.len() as u64) as usize]);
        }
        let r = run(&["-d", &s, "--format", "%s"]);
        assert!(r.code == 0 || r.code == 1, "garbage {s:?} -> {}", r.code);
        if r.code == 0 {
            assert_eq!(r.out().lines().count(), 1, "{s:?}");
            let v = r.out().trim().parse::<i64>();
            assert!(v.is_ok(), "{s:?} -> {:?}", r.out());
        } else {
            assert!(r.err().contains("invalid date"), "{s:?}");
        }
    }
}
