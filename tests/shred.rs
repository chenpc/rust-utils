//! shred e2e（guest 內執行，tmpfs /tmp）
//! 行為規格：docs/man/shred.1.gz ＋本機參考逐字措辭（docs/probes/cluster5.md 記錄）；
//! man 未載的措辭/邊界記於 docs/assumptions.md。
//! 逐字訊息取自我方 binary 已定案的輸出（`shred:` 前綴、verbose 行走 stderr）。
use coreutils::testutil as t;
use std::path::{Path, PathBuf};

fn fresh(tag: &str) -> PathBuf {
    let d = PathBuf::from(format!("/tmp/shred_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn m(args: &[&str]) -> t::Out {
    t::run("shred", args, "")
}

fn data_of(len: usize) -> Vec<u8> {
    (0..len).map(|i| b'A' + (i % 26) as u8).collect()
}

fn put(d: &Path, name: &str, len: usize) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, data_of(len)).unwrap();
    p
}

fn size_of(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn lines(s: &str) -> Vec<String> {
    s.lines().map(|x| x.to_string()).collect()
}

fn pass_lines(err: &str) -> Vec<String> {
    err.lines().filter(|l| l.contains(": pass ")).map(|x| x.to_string()).collect()
}

#[test]
fn three_default_passes_verbose() {
    let d = fresh("def");
    let f = put(&d, "a", 10);
    let r = m(&["-v", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        pass_lines(&r.err()),
        vec![
            format!("shred: {}: pass 1/3 (random)...", f.display()),
            format!("shred: {}: pass 2/3 (random)...", f.display()),
            format!("shred: {}: pass 3/3 (random)...", f.display()),
        ]
    );
    // 預設 size 進位到 4096 的倍數
    assert_eq!(size_of(&f), 4096);
    // 內容已被覆蓋
    let b = std::fs::read(&f).unwrap();
    assert!(b.iter().any(|&x| x != b'A'), "content must be overwritten");
}

#[test]
fn zero_pass_appended_and_file_zeroed() {
    let d = fresh("zero");
    let f = put(&d, "b", 3);
    let r = m(&["-v", "-z", "-n", "1", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        pass_lines(&r.err()),
        vec![
            format!("shred: {}: pass 1/2 (random)...", f.display()),
            format!("shred: {}: pass 2/2 (000000)...", f.display()),
        ],
        "-z 的分母須含 zero pass"
    );
    let b = std::fs::read(&f).unwrap();
    assert_eq!(b.len(), 4096);
    assert!(b.iter().all(|&x| x == 0), "-z 後整檔須為 0");
}

#[test]
fn n_zero_writes_nothing() {
    let d = fresh("n0");
    let f = put(&d, "e", 3);
    let r = m(&["-v", "-n", "0", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(pass_lines(&r.err()).is_empty(), "n=0 不得有 pass 行");
    assert_eq!(size_of(&f), 3);
    assert_eq!(std::fs::read(&f).unwrap(), data_of(3));
}

#[test]
fn n_zero_with_remove_still_removes() {
    let d = fresh("n0u");
    let f = put(&d, "f", 3);
    let r = m(&["-v", "-n", "0", "-u", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!f.exists(), "-n 0 -u 仍須移除檔案");
    assert_eq!(
        lines(&r.err()),
        vec![
            format!("shred: {}: removing", f.display()),
            format!("shred: {}: renamed to {}/0", f.display(), d.display()),
            format!("shred: {}: removed", f.display()),
        ],
        "單字元檔名只應rename 一次"
    );
}

#[test]
fn remove_rename_sequence_five_chars() {
    let d = fresh("seq");
    let f = put(&d, "abcde", 5);
    let r = m(&["-v", "-n", "1", "-u", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!f.exists());
    assert_eq!(
        lines(&r.err()),
        vec![
            format!("shred: {}: pass 1/1 (random)...", f.display()),
            format!("shred: {}: removing", f.display()),
            format!("shred: {}: renamed to {}/00000", f.display(), d.display()),
            format!("shred: {}: renamed to {}/0000", d.join("00000").display(), d.display()),
            format!("shred: {}: renamed to {}/000", d.join("0000").display(), d.display()),
            format!("shred: {}: renamed to {}/00", d.join("000").display(), d.display()),
            format!("shred: {}: renamed to {}/0", d.join("00").display(), d.display()),
            format!("shred: {}: removed", f.display()),
        ],
        "rename 序列逐字（最後一行的主語回到原名）"
    );
}

#[test]
fn remove_unlink_has_no_rename_sequence() {
    let d = fresh("ul");
    let f = put(&d, "g", 3);
    let r = m(&["-v", "-n", "1", "--remove=unlink", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!f.exists());
    assert!(r.err().contains("removing"));
    assert!(r.err().contains("removed"));
    assert!(!r.err().contains("renamed to"), "--remove=unlink 不得 rename");
}

#[test]
fn size_rounding_exact_and_explicit() {
    let d = fresh("sz");
    // 預設：進位到 4096 的倍數
    let a = put(&d, "h", 3);
    assert_eq!(m(&["-n", "1", &a.to_string_lossy()]).code, 0);
    assert_eq!(size_of(&a), 4096, "3B 須進位為 4096");

    let b = put(&d, "i", 4100);
    assert_eq!(m(&["-n", "1", &b.to_string_lossy()]).code, 0);
    assert_eq!(size_of(&b), 8192, "4100B 須進位為 8192");

    // -x：不進位、不對齊
    let c = put(&d, "j", 4100);
    assert_eq!(m(&["-n", "1", "-x", &c.to_string_lossy()]).code, 0);
    assert_eq!(size_of(&c), 4100, "-x 不得進位");

    // -s：顯式 size（不縮小既有大小、不進位）
    let e = put(&d, "k", 3);
    assert_eq!(m(&["-n", "1", "-s", "100", &e.to_string_lossy()]).code, 0);
    assert_eq!(size_of(&e), 100);

    let f = put(&d, "l", 5000);
    assert_eq!(m(&["-n", "1", "-s", "100", &f.to_string_lossy()]).code, 0);
    assert_eq!(size_of(&f), 5000, "-s 不得縮小既有檔案");
}

#[test]
fn directory_and_missing_operand() {
    let d = fresh("dir");
    let sub = d.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let r = m(&[&sub.to_string_lossy()]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), format!("shred: {}: Not a file", sub.display()));

    let missing = d.join("nope").to_string_lossy().into_owned();
    let r = m(&[&missing]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), format!("shred: {}: No such file or directory", missing));

    let r = m(&[]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("missing file operand"), "{}", r.err());
}

#[test]
fn invalid_value_messages() {
    let d = fresh("inv");
    let f = put(&d, "a", 4);
    let p = f.to_string_lossy().into_owned();

    let r = m(&["-n", "bogus", &p]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "shred: invalid number of passes: 'bogus'");

    let r = m(&["-s", "bogus", &p]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "shred: invalid file size: 'bogus'");

    let r = m(&["--remove=bogus", &p]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("invalid value 'bogus' for '--remove[=<HOW>]'"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("[possible values: unlink, wipe, wipesync]"));
    assert!(r.err().contains("For more information, try '--help'."));

    // 不存在檔案與不存在 device 一律失敗並保留原檔（guest tmpfs 無 labels）
    let r = m(&["--random-source=/nonexistent", &p]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "shred: /nonexistent: No such file or directory");

    // random-source 太小 → fill whole buffer
    let src = d.join("src");
    std::fs::write(&src, b"abcd").unwrap();
    let r = m(&[
        "-n",
        "1",
        "-s",
        "100",
        &format!("--random-source={}", src.display()),
        &p,
    ]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(
        r.err().contains("File write pass failed: failed to fill whole buffer"),
        "{}",
        r.err()
    );
}

#[test]
fn help_and_version() {
    let r = m(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(
        r.out().starts_with(
            "Overwrite the specified FILE(s) repeatedly, in order to make it harder for even\nvery expensive hardware probing to recover the data.\n\nUsage: shred [OPTION]... FILE..."
        ),
        "{}",
        r.out()
    );
    for o in [
        "-f, --force",
        "-n, --iterations <NUMBER>",
        "-s, --size <N>",
        "--remove[=<HOW>]",
        "-v, --verbose",
        "-x, --exact",
        "-z, --zero",
        "--random-source <random-source>",
    ] {
        assert!(r.out().contains(o), "help 須列 {}", o);
    }
    assert!(r.out().contains("CAUTION: Note that shred relies on a very important assumption"));

    let r = m(&["-V"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end(), "shred (uutils coreutils) 0.10.0");

    let r = m(&["-vh"]);
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: shred"));
}

#[test]
fn fuzz_shred_invariants() {
    let d = fresh("fz");
    let mut rng = t::Rng::new(20261110);
    for k in 0..24 {
        let len = 1 + (rng.below(9000) as usize);
        let name = format!("fz{}", k % 7);
        let f = put(&d, &name, len);
        let size = len as u64;

        let n = 1 + rng.below(3);
        let zero = rng.below(2) == 1;
        let exact = rng.below(2) == 1;
        let explicit = rng.below(3) == 0;
        let rm = rng.below(2) == 1;

        let mut args: Vec<String> = vec!["-v".to_string(), "-n".to_string(), n.to_string()];
        if zero {
            args.push("-z".to_string());
        }
        if exact {
            args.push("-x".to_string());
        }
        if explicit {
            args.push("-s".to_string());
            args.push((1 + rng.below(500)).to_string());
        }
        if rm {
            args.push("-u".to_string());
        }
        // -x 與 -s 不同時出現（規格未定義該組合）
        let s_val: Option<u64> = if explicit {
            args.iter().position(|a| a == "-s").map(|i| args[i + 1].parse().unwrap())
        } else {
            None
        };
        args.push(f.to_string_lossy().into_owned());

        let r = m(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        assert_eq!(r.code, 0, "case {}: {}", k, r.err());

        // verbose pass 行數 = n（+1 為 zero pass）
        let expect_passes = n + if zero { 1 } else { 0 };
        assert_eq!(
            pass_lines(&r.err()).len() as u64,
            expect_passes,
            "case {} n={} zero={} err={}",
            k,
            n,
            zero,
            r.err()
        );
        for (i, l) in pass_lines(&r.err()).iter().enumerate() {
            let kind = if zero && i + 1 == expect_passes as usize {
                "000000"
            } else {
                "random"
            };
            assert_eq!(
                *l,
                format!("shred: {}: pass {}/{} ({})...", f.display(), i + 1, expect_passes, kind),
                "case {} pass 行格式",
                k
            );
        }

        if rm {
            assert!(!f.exists(), "case {}: -u 須移除檔案", k);
            continue;
        }

        // size 規則：-x → 原 size；-s → max(原, S)；否則進位到 4096 倍數
        let got = size_of(&f);
        if exact {
            assert_eq!(got, size, "case {}: -x 不得改變 size", k);
        } else if let Some(s) = s_val {
            assert_eq!(got, std::cmp::max(size, s), "case {}: -s 規則", k);
        } else {
            assert_eq!(got, ((size + 4095) / 4096) * 4096, "case {}: 進位規則", k);
        }

        let b = std::fs::read(&f).unwrap();
        if zero {
            assert!(b.iter().all(|&x| x == 0), "case {}: -z 末輪須全 0", k);
        } else if b.len() == size as usize {
            // -x（size 不變）：內容須與原檔不同
            assert_ne!(b, data_of(size as usize), "case {}: 覆寫後內容必須改變", k);
        }
    }
}

#[test]
fn random_source_variants() {
    let d = fresh("rs");
    let src_dir = d.join("srcdir");
    std::fs::create_dir(&src_dir).unwrap();
    let small = d.join("small");
    std::fs::write(&small, b"zzz").unwrap();
    let big = d.join("big");
    std::fs::write(&big, vec![0x5Au8; 8192]).unwrap();
    let p = d.join("f1");

    // 來源是目錄：訊息報目標檔（不是來源路徑）
    std::fs::write(&p, vec![b'a'; 4096]).unwrap();
    let src1 = format!("--random-source={}", src_dir.display());
    let r = m(&[src1.as_str(), p.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!("shred: {}: File write pass failed: Is a directory", p.display())
    );

    // 來源太小：同样報目標檔
    std::fs::write(&p, vec![b'a'; 4096]).unwrap();
    let src2 = format!("--random-source={}", small.display());
    let r = m(&[src2.as_str(), p.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!(
            "shred: {}: File write pass failed: failed to fill whole buffer",
            p.display()
        )
    );

    // 來源開檔失敗：訊息報來源路徑
    let missing = d.join("nope").to_string_lossy().into_owned();
    let src3 = format!("--random-source={}", missing);
    let r = m(&[src3.as_str(), p.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!("shred: {}: No such file or directory", missing)
    );

    // 來源充足：成功覆寫且內容等於來源前 size 個 byte
    std::fs::write(&p, vec![b'a'; 100]).unwrap();
    let src4 = format!("--random-source={}", big.display());
    let r = m(&[src4.as_str(), "-n", "1", p.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(std::fs::read(&p).unwrap(), vec![0x5Au8; 4096]);
}

#[test]
fn remove_modes_and_zero_size() {
    let d = fresh("mode");
    // --remove=wipesync：與 wipe 相同的 rename 序列（名字長度由 len 递减到 1）
    let f = d.join("h2");
    std::fs::write(&f, b"x").unwrap();
    let r = m(&["--remove=wipesync", "-v", "-u", "-n", "1", f.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    let fp = f.to_string_lossy().into_owned();
    let dirp = d.to_string_lossy().into_owned();
    assert_eq!(
        lines(&r.err()),
        vec![
            format!("shred: {}: pass 1/1 (random)...", fp),
            format!("shred: {}: removing", fp),
            format!("shred: {}: renamed to {}/00", fp, dirp),
            format!("shred: {}/00: renamed to {}/0", dirp, dirp),
            format!("shred: {}: removed", fp),
        ]
    );
    assert!(!f.exists());

    // -s 0：等同檔案實際大小（size 不變、仍有一輪 random pass）
    let g = d.join("h1");
    std::fs::write(&g, vec![b'a'; 100]).unwrap();
    let r = m(&["-s", "0", "-n", "1", "-v", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        r.err().trim_end(),
        format!("shred: {}: pass 1/1 (random)...", g.to_string_lossy())
    );
    assert_eq!(size_of(&g), 100);
    assert_ne!(std::fs::read(&g).unwrap(), vec![b'a'; 100]);

    // 多個 operand 一律處理
    let a = d.join("m1");
    let b = d.join("m2");
    std::fs::write(&a, b"aaa").unwrap();
    std::fs::write(&b, b"bbb").unwrap();
    let r = m(&["-n", "1", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "");
    assert_eq!(size_of(&a), 4096);
    assert_eq!(size_of(&b), 4096);
}

#[test]
fn cli_numeric_and_metavar_errors() {
    let d = fresh("n25");
    let f = put(&d, "f", 3);
    // 數值驗證排在 operand 檢查之後（參考實測：`shred -n <file>` 先報 missing file operand）
    let r = m(&["-n", f.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("shred: missing file operand\n"), "{}", r.err());
    assert_eq!(std::fs::read(&f).unwrap(), vec![b'A', b'B', b'C'], "檔案不應被改動");

    let r = m(&["-n", "x"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("shred: missing file operand"), "{}", r.err());

    // 缺值時用長選項名與參考 metavar
    let r = m(&["-n"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: a value is required for '--iterations <NUMBER>' but none was supplied\n\nFor more information, try '--help'.\n",
        "stderr mismatch"
    );
    let r = m(&["-s"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: a value is required for '--size <N>' but none was supplied\n\nFor more information, try '--help'.\n",
        "stderr mismatch"
    );
    let r = m(&["--size"]);
    assert!(
        r.err()
            .starts_with("error: a value is required for '--size <N>' but none was supplied"),
        "{}",
        r.err()
    );

    let r = m(&["-n", "abc", f.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("shred: invalid number of passes: 'abc'"),
        "{}",
        r.err()
    );
    let r = m(&["-s", "xyz", f.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("shred: invalid file size: 'xyz'"),
        "{}",
        r.err()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn exact_flag_and_size_growth() {
    let d = fresh("n26");
    // -x：大小不變（即使不是 4096 的倍數）
    let e1 = put(&d, "e1", 3);
    let r = m(&["-x", "-n", "1", e1.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&e1), 3);
    // -x 搭配比實際大小小的 -s：也不縮小
    let e2 = put(&d, "e2", 3);
    let r = m(&["-x", "-n", "1", "-s", "2", e2.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&e2), 3);
    // 無 -x：小檔案無條件進位到 4096
    let e3 = put(&d, "e3", 3);
    let r = m(&["-n", "1", e3.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&e3), 4096);
    // -s N 只在實際 size < N 時成長
    let e4 = put(&d, "e4", 4100);
    let r = m(&["-n", "1", "-s", "10000", e4.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&e4), 10000);
    // 不存在的檔案
    let missing = d.join("nope");
    let r = m(&["-n", "1", missing.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .contains(&format!("shred: {}: No such file or directory", missing.display())),
        "{}",
        r.err()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn unknown_options_and_missing_values() {
    let d = fresh("n27");
    let f = put(&d, "g1", 3);
    let fp = f.to_str().unwrap();

    // 未知長選項：error + tip + Usage + For more information
    let r = m(&["--bogus", fp]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("Usage: shred [OPTION]... FILE...\n"), "{}", r.err());
    assert!(r.err().contains("For more information, try '--help'."), "{}", r.err());

    // 可推斷的前綴：--verb → --verbose（旗標不吃值，fp 仍是 operand）
    let r = m(&["--verb", fp]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().contains("pass 1/3 (random)"), "{}", r.err());

    let r = m(&["-w", fp]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected argument '-w' found\n"),
        "{}",
        r.err()
    );

    // 長選項缺值：用長選項名與參考 metavar
    let r = m(&["--iterations"]);
    assert!(
        r.err()
            .starts_with("error: a value is required for '--iterations <NUMBER>' but none was supplied"),
        "{}",
        r.err()
    );
    let r = m(&["--random-source"]);
    assert!(
        r.err()
            .starts_with("error: a value is required for '--random-source <random-source>' but none was supplied"),
        "{}",
        r.err()
    );

    // --remove 不给 HOW：等同 -u（ wipe ）
    let g = put(&d, "g2", 3);
    let r = m(&["--remove", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!g.exists());

    // --remove=unlink
    let g = put(&d, "g3", 3);
    let r = m(&["-n", "1", "--remove=unlink", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!g.exists());

    // -u 单独（不指定 -n）
    let g = put(&d, "g4", 3);
    let r = m(&["-u", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!g.exists());

    // size 後綴
    let g = put(&d, "g5", 3);
    let r = m(&["-n", "1", "-s", "1K", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&g), 1024);
    // 小寫 b = 512；大寫 B 不是後綴（參考實測 `1B` 被拒）
    let g = put(&d, "g6", 3);
    let r = m(&["-n", "1", "-s", "1b", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&g), 512);
    let g = put(&d, "g7", 3);
    let r = m(&["-n", "1", "-s", "1B", g.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("shred: invalid file size: '1B'"),
        "{}",
        r.err()
    );
    assert_eq!(size_of(&g), 3);
    // KB = 1000、K = 1024（參考實測）
    let g = put(&d, "g8", 3);
    let r = m(&["-n", "1", "-s", "1KB", g.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(size_of(&g), 1000);
    let _ = std::fs::remove_dir_all(&d);
}
