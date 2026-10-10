//! du 測試：guest /tmp 為 tmpfs，區塊數以 tmpfs 實際配置為準 →
//! 用不變量 + apparent(-b) 精確值斷言，不硬鎖 tmpfs 區塊數字。

use coreutils::testutil as t;

fn du(args: &[&str]) -> t::Out {
    t::run("du", args, "")
}

fn rows(o: &t::Out) -> Vec<(u64, String)> {
    o.out()
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(2, '\t');
            let v = it.next()?.parse().ok()?;
            Some((v, it.next()?.to_string()))
        })
        .collect()
}

/// 建 /tmp/dut_<name>：d/{f1 12B, f2 0B, sub/{f3 100B}, lnk->f1, loop->.}
fn fixture(name: &str) -> String {
    let root = format!("/tmp/dut_{name}");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(format!("{root}/d/sub")).unwrap();
    std::fs::write(format!("{root}/d/f1"), b"hello world!").unwrap();
    std::fs::write(format!("{root}/d/f2"), b"").unwrap();
    std::fs::write(format!("{root}/d/sub/f3"), vec![b'x'; 100]).unwrap();
    std::os::unix::fs::symlink("f1", format!("{root}/d/lnk")).unwrap();
    std::os::unix::fs::symlink(".", format!("{root}/d/loop")).unwrap();
    root
}

#[test]
fn default_recursive() {
    let root = fixture("def");
    let r = du(&[&format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    // 子目錄列 + operand 列，operand 列最後且 total >= 子目錄 total
    assert!(rs.iter().any(|(_, p)| p.ends_with("/sub")));
    assert_eq!(rs.last().unwrap().1, format!("{root}/d"));
    let sub = rs.iter().find(|(_, p)| p.ends_with("/sub")).unwrap().0;
    let top = rs.last().unwrap().0;
    assert!(top >= sub, "{top} >= {sub}");
}

#[test]
fn summarize() {
    let root = fixture("sum");
    let r = du(&["-s", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0].1, format!("{root}/d"));
}

#[test]
fn all_lists_files() {
    let root = fixture("all");
    let r = du(&["-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    assert!(rs.iter().any(|(_, p)| p.ends_with("/f1")));
    assert!(rs.iter().any(|(_, p)| p.ends_with("/f2")));
    assert!(rs.iter().any(|(_, p)| p.ends_with("/sub/f3")));
    // symlink 本身以 lstat 計（預設 -P 不追隨）：lnk 列出
    assert!(rs.iter().any(|(_, p)| p.ends_with("/lnk")));
    // loop symlink 不造成遞迴（不會出現 /loop/... 深路徑）
    assert!(!rs.iter().any(|(_, p)| p.contains("/loop/")));
}

#[test]
fn apparent_sizes_exact() {
    let root = fixture("app");
    let r = du(&["-a", "-b", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    let v = |suf: &str| -> u64 {
        rs.iter().find(|(_, p)| p.ends_with(suf)).unwrap().0
    };
    assert_eq!(v("/f1"), 12);
    assert_eq!(v("/f2"), 0);
    assert_eq!(v("/f3"), 100);
    assert_eq!(v("/lnk"), 2); // symlink 目標名稱 "f1" 長度
    // -b 下總和 = 所有檔案 apparent（目錄 own 0）
    let total = rs.iter().find(|(_, p)| *p == format!("{root}/d")).unwrap().0;
    // GNU -b：entry apparent = lstat st_size（目錄 own = st_size，tmpfs 上非 0）
    let mut expect = 0u64;
    for e in ["", "/sub", "/f1", "/f2", "/sub/f3", "/lnk", "/loop"] {
        expect += std::fs::symlink_metadata(format!("{root}/d{e}")).unwrap().len();
    }
    assert_eq!(total, expect);
}

#[test]
fn depth_limit() {
    let root = fixture("depth");
    let r = du(&["-d", "1", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    assert!(rs.iter().any(|(_, p)| p.ends_with("/sub")));
    assert!(!rs.iter().any(|(_, p)| p.ends_with("/sub/f3")));
    // --max-depth=0 == -s
    let r2 = du(&["--max-depth=0", &format!("{root}/d")]);
    assert_eq!(rows(&r2).len(), 1);
}

#[test]
fn total_line() {
    let root = fixture("total");
    let r = du(&["-c", "-s", &format!("{root}/d/sub"), &format!("{root}/d/f1")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    assert_eq!(rs.len(), 3);
    assert_eq!(rs[2].1, "total");
    assert_eq!(rs[2].0, rs[0].0 + rs[1].0);
}

#[test]
fn nul_terminated() {
    let root = fixture("nul");
    let r = du(&["-s", "-0", &format!("{root}/d")]);
    let s = String::from_utf8(r.stdout.clone()).unwrap();
    assert!(s.ends_with('\0'));
    assert_eq!(s.matches('\0').count(), 1);
}

#[test]
fn exclude_patterns() {
    let root = fixture("exc");
    let r = du(&["-a", "--exclude", "f1", &format!("{root}/d")]);
    let rs = rows(&r);
    assert!(!rs.iter().any(|(_, p)| p.ends_with("/f1")));
    // glob 與 {a,b}
    let r2 = du(&["-a", "--exclude", "?3", &format!("{root}/d")]);
    assert!(!rows(&r2).iter().any(|(_, p)| p.ends_with("/f3")));
    let r3 = du(&["-a", "--exclude", "{f1,f2}", &format!("{root}/d")]);
    let rs3 = rows(&r3);
    assert!(!rs3.iter().any(|(_, p)| p.ends_with("/f1") || p.ends_with("/f2")));
    // -X 檔案
    std::fs::write(format!("{root}/pat"), "f1\n").unwrap();
    let r4 = du(&["-a", "-X", &format!("{root}/pat"), &format!("{root}/d")]);
    assert!(!rows(&r4).iter().any(|(_, p)| p.ends_with("/f1")));
}

#[test]
fn inodes_count() {
    let root = fixture("ino");
    let r = du(&["--inodes", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    // d: d + sub + f1 + f2 + f3 + lnk + loop = 7 個 inode
    assert_eq!(rs.last().unwrap().0, 7);
}

#[test]
fn symlink_deref() {
    let root = fixture("sym");
    // -L：lnk 指向 f1(12B) → apparent 12；-P 預設 2
    let rl = du(&["-a", "-b", "-L", &format!("{root}/d")]);
    let rs = rows(&rl);
    assert_eq!(rs.iter().find(|(_, p)| p.ends_with("/lnk")).unwrap().0, 12);
    let rp = du(&["-a", "-b", "-P", &format!("{root}/d")]);
    assert_eq!(
        rows(&rp).iter().find(|(_, p)| p.ends_with("/lnk")).unwrap().0,
        2
    );
}

#[test]
fn hardlink_counted_once() {
    let root = fixture("hard");
    std::fs::hard_link(format!("{root}/d/f1"), format!("{root}/d/hard")).unwrap();
    let r = du(&["-b", "-a", &format!("{root}/d")]);
    let rs = rows(&r);
    // -b apparent 每檔都計（語意如此）；區塊模式下去重由 --inodes 驗證
    assert!(rs.iter().any(|(_, p)| p.ends_with("/hard")));
    let ri = du(&["--inodes", &format!("{root}/d")]);
    // 8 條目但 f1/hard 同 inode → 7
    assert_eq!(rows(&ri).last().unwrap().0, 7);
}

#[test]
fn dup_operand_dedup() {
    let root = fixture("dup");
    let r = du(&["-s", &format!("{root}/d"), &format!("{root}/d")]);
    assert_eq!(rows(&r).len(), 1);
    let r2 = du(&["-s", &format!("{root}/d"), &format!("{root}/d/sub")]);
    // sub 已被 d 涵蓋：只印 d（sub 為 d 的子孙、(dev,ino) 已見）
    let rs = rows(&r2);
    assert_eq!(rs.len(), 1, "{rs:?}");
}

#[test]
fn threshold() {
    let root = fixture("thr");
    // tmpfs 上 f3(100B) 可能佔 1 頁 4K；用 -b 語意的 -t（比較位元組）
    let r = du(&["-a", "-b", "-t", "50", &format!("{root}/d")]);
    let rs = rows(&r);
    assert!(rs.iter().any(|(_, p)| p.ends_with("/f3")));
    assert!(!rs.iter().any(|(_, p)| p.ends_with("/f1"))); // 12 < 50
    let r2 = du(&["-a", "-b", "-t", "-50", &format!("{root}/d")]);
    let rs2 = rows(&r2);
    assert!(rs2.iter().any(|(_, p)| p.ends_with("/f1")));
    assert!(!rs2.iter().any(|(_, p)| p.ends_with("/f3")));
}

#[test]
fn human_format() {
    let root = fixture("hum");
    let r = du(&["-a", "-b", "-h", &format!("{root}/d")]);
    let out = r.out();
    // 人類單位：100 → "100"、12 → "12"（<1K 純數字）、大檔帶 K/M
    assert!(out.lines().any(|l| l.starts_with("100\t")));
    let r2 = du(&["-a", "-b", "--si", &format!("{root}/d/sub")]);
    assert_eq!(r2.code, 0);
}

#[test]
fn separate_dirs_and_all_conflict() {
    let root = fixture("sep");
    let r = du(&["-S", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let rs = rows(&r);
    // -S：目錄 own 不含子孙 → top 列 own <= default top
    let rd = du(&[&format!("{root}/d")]);
    assert!(rs.last().unwrap().0 <= rows(&rd).last().unwrap().0);
    // -a 與 -s 衝突
    let r2 = du(&["-a", "-s", &format!("{root}/d")]);
    assert_eq!(r2.code, 1);
    assert!(r2.stderr.starts_with(b"du: the argument '--all' cannot be used with '--summarize'"));
}

#[test]
fn block_size_option() {
    let root = fixture("blk");
    let a = du(&["-s", &format!("{root}/d")]);
    let b = du(&["-s", "-B", "512", &format!("{root}/d")]);
    // 512 區塊數字 = 1024 的兩倍（區塊以 512B 計）
    assert_eq!(rows(&b)[0].0, 2 * rows(&a)[0].0);
    let bm = du(&["-s", "-B", "1024", &format!("{root}/d")]);
    assert_eq!(rows(&bm)[0].0, rows(&a)[0].0);
}

#[test]
fn time_columns() {
    let root = fixture("time");
    let r = du(&["--time", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    // 預設 long-iso：YYYY-MM-DD HH:MM
    let out = r.out();
    let l = out.lines().find(|l| l.ends_with("/f1")).unwrap();
    let tcol = l.split('\t').nth(1).unwrap();
    assert!(tcol.len() >= 16, "{tcol}");
    assert_eq!(&tcol[4..5], "-");
    let ri = du(&["--time", "--time-style=iso", "-a", &format!("{root}/d")]);
    let ro = ri.out();
    let li = ro.lines().find(|l| l.ends_with("/f1")).unwrap();
    let ti = li.split('\t').nth(1).unwrap();
    assert_eq!(ti.len(), 10, "{ti}");
    let rf = du(&["--time", "--time-style=full-iso", "-a", &format!("{root}/d")]);
    assert_eq!(rf.code, 0);
    let rf2 = rf.out();
    let lf = rf2.lines().find(|l| l.ends_with("/f1")).unwrap();
    assert_eq!(lf.split('\t').nth(1).unwrap().len(), 25);
    let rc = du(&["--time=ctime", "-a", &format!("{root}/d")]);
    assert_eq!(rc.code, 0);
}

#[test]
fn files0_from() {
    let root = fixture("f0");
    let input = format!("{root}/d\0");
    let r = t::run("du", &["--files0-from", &format!("{root}/list0")], &input);
    // list0 不存在 → 錯誤 rc1
    assert_eq!(r.code, 1);
    std::fs::write(format!("{root}/list0"), format!("{root}/d\0").as_bytes()).unwrap();
    let r2 = du(&["--files0-from", &format!("{root}/list0")]);
    assert_eq!(r2.code, 0);
    assert_eq!(rows(&r2).last().unwrap().1, format!("{root}/d"));
}

#[test]
fn errors_and_options() {
    let r = du(&["/tmp/noexist_zz"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stderr,
        b"du: cannot access '/tmp/noexist_zz': No such file or directory\n"
    );
    let r = du(&["-d", "bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid maximum depth 'bogus'\n");
    let r = du(&["-t", "bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid threshold value 'bogus'\n");
    let r = du(&["-B", "bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid block size 'bogus'\n");
    let r = du(&["--exclude"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"du: option requires an argument -- 'exclude'"));
    let r = du(&["--bogus"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"du: unrecognized option '--bogus'"));
    let r = du(&["-y"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"du: invalid option -- 'y'"));
}

#[test]
fn help_version() {
    let r = du(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with(b"Usage"));
    let r = du(&["--version"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"du (uutils coreutils) 0.10.0\n");
    let r = du(&["-A"]);
    assert_eq!(r.code, 0); // -A apparent == -b
}

#[test]
fn fuzz_apparent_sum_invariant() {
    // 不變量：任意樹下 du -b 的 operand total = 所有檔案 apparent 和（目錄 own 0）
    use t::Rng;
    let root = "/tmp/dut_fuzz";
    let _ = std::fs::remove_dir_all(root);
    let mut rng = Rng::new(20260729);
    let mut expect = 0u64;
    for i in 0..12 {
        let depth = rng.below(3) as usize;
        let mut p = format!("{root}/t{i}");
        for d in 0..depth {
            std::fs::create_dir_all(&p).unwrap();
            p = format!("{p}/c{d}");
        }
        std::fs::create_dir_all(&p).unwrap();
        let sz = rng.below(500) as usize;
        std::fs::write(format!("{p}/data"), vec![b'z'; sz]).unwrap();
    }
    // 預期 = 樹內所有 entry 的 lstat size 和（含目錄本身）
    let mut stack = vec![root.to_string()];
    let mut nentries = 1usize;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            nentries += 1;
            expect += std::fs::symlink_metadata(&p).unwrap().len();
            if p.is_dir() { stack.push(p.to_string_lossy().into_owned()); }
        }
    }
    expect += std::fs::symlink_metadata(root).unwrap().len();
    let r = du(&["-b", "-s", root]);
    assert_eq!(r.code, 0);
    assert_eq!(rows(&r)[0].0, expect);
    // -a 的最後一列（root）== -s total；列數 = entry 數（每 entry 一列）
    let ra = du(&["-b", "-a", root]);
    assert_eq!(rows(&ra).last().unwrap().0, expect);
    assert_eq!(rows(&ra).len(), nentries);
}

#[test]
fn long_options_and_terminator() {
    let root = fixture("long");
    // 長選項：--summarize --apparent-size --human-readable --si --inodes --null
    // --separate-dirs --one-file-system --verbose --total --no-dereference
    let r = du(&["--summarize", &format!("{root}/d")]);
    assert_eq!(rows(&r).len(), 1);
    let r = du(&["--apparent-size", "-a", "--", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--human-readable", "--si", "-s", "--", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--inodes", "--null", "-s", &format!("{root}/d")]);
    assert!(r.out_bytes().ends_with(b"\0"));
    let r = du(&["--separate-dirs", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--one-file-system", "--verbose", "-s", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--no-dereference", "-s", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    // --summarize 與 --all 衝突（長選項 arm）
    let r = du(&["--apparent-size", "--all", "--summarize", &format!("{root}/d")]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.starts_with(b"du: the argument '--all' cannot be used with '--summarize'"));
}

#[test]
fn long_option_values_and_errors() {
    let root = fixture("lopt");
    // --block-size=N 長選項 arm
    let a = du(&["-s", &format!("{root}/d")]);
    let b = du(&["-s", "--block-size=512", &format!("{root}/d")]);
    assert_eq!(rows(&b)[0].0, 2 * rows(&a)[0].0);
    let r = du(&["--block-size=bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid block size 'bogus'\n");
    let r = du(&["--max-depth=bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid maximum depth 'bogus'\n");
    let r = du(&["--threshold=bogus", "/tmp"]);
    assert_eq!(r.code, 2);
    assert_eq!(r.stderr, b"du: invalid threshold value 'bogus'\n");
    // -X 檔案讀取失敗
    let r = du(&["-X", "/tmp/noexist_pat_zz", "/tmp"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stderr,
        b"du: cannot access '/tmp/noexist_pat_zz': No such file or directory\n"
    );
    // --exclude-from= 失敗
    let r = du(&["--exclude-from", "/tmp/noexist_pat2_zz", "/tmp"]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.starts_with(b"du: cannot access '/tmp/noexist_pat2_zz'"));
    // --no-such-long
    let r = du(&["--no-such-option"]);
    assert_eq!(r.code, 2);
}

#[test]
fn short_option_letters() {
    let root = fixture("letters");
    let a = du(&["-s", "-k", &format!("{root}/d")]);
    assert_eq!(rows(&a)[0].0, rows(&du(&["-s", &format!("{root}/d")]))[0].0);
    let m = du(&["-s", "-m", &format!("{root}/d")]);
    assert_eq!(m.code, 0);
    let r = du(&["-s", "-l", "-L", "-D", "-v", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["-s", "-x", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    // -B 內嵌值與下參
    let r = du(&["-s", "-B512", &format!("{root}/d")]);
    assert_eq!(rows(&r)[0].0, 2 * rows(&a)[0].0);
    let r = du(&["-d1", "-t", "0", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    // -d 內嵌
    let r = du(&["--max-depth=1", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
}

#[test]
fn env_block_size_and_stdin_list() {
    let root = fixture("envblk");
    let a = du(&["-s", &format!("{root}/d")]);
    std::env::set_var("DU_BLOCK_SIZE", "512");
    let b = du(&["-s", &format!("{root}/d")]);
    std::env::remove_var("DU_BLOCK_SIZE");
    assert_eq!(rows(&b)[0].0, 2 * rows(&a)[0].0);
    // --files0-from - 從 stdin 讀 NUL 分隔清單
    let r = t::run("du", &["--files0-from", "-"], &format!("{root}/d\0"));
    assert_eq!(r.code, 0);
    assert_eq!(rows(&r).last().unwrap().1, format!("{root}/d"));
    // --inodes --total 的 total 列
    let r = du(&["--inodes", "-c", "-s", &format!("{root}/d/sub"), &format!("{root}/d/f1")]);
    let rs = rows(&r);
    assert_eq!(rs.last().unwrap().1, "total");
    assert_eq!(rs.last().unwrap().0, rs[0].0 + rs[1].0);
}

#[test]
fn deref_args_and_dangling_time_null() {
    let root = fixture("deref");
    // operand 本身是 symlink：-D/-H/-L 追隨、-P 不追隨
    std::os::unix::fs::symlink(format!("{root}/d"), format!("{root}/link")).unwrap();
    let rp = du(&["-s", "-P", &format!("{root}/link")]);
    assert_eq!(rp.code, 0);
    let rd = du(&["-s", "-D", &format!("{root}/link")]);
    assert_eq!(rd.code, 0);
    assert!(rows(&rd)[0].0 > rows(&rp)[0].0, "{} > {}", rows(&rd)[0].0, rows(&rp)[0].0);
    // -L 下断鏈 symlink：報錯 rc1
    std::os::unix::fs::symlink("noexist_zz", format!("{root}/d/dang")).unwrap();
    let r = du(&["-L", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot access"), "{}", r.err());
    // --time=birth/creation 與 --time -0 組合
    let r = du(&["--time=birth", "--time-style=full-iso", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--time=creation", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let r = du(&["--time=access", "-a", "-0", &format!("{root}/d")]);
    assert!(r.out_bytes().ends_with(b"\0"));
    // uutils 對未知 time WORD 不報錯，退回 mtime
    let r = du(&["--time=bogus", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    // --time-style=+FORMAT 自訂
    let r = du(&["--time", "--time-style=+%Y", "-a", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    let out = r.out();
    let yr = chrono_year();
    assert!(out.contains(&yr.to_string()), "{out}");
}

fn chrono_year() -> u64 {
    // /etc/localtime 可能有/沒有；直接用 du 輸出解析太繞，取 system time 近似
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / (365 * 86400)
        + 1970
}

#[test]
fn glob_braces_and_star_mid() {
    let root = fixture("globx");
    // {a,b} 與中段 *
    // 我方 --exclude 比對相對全路徑**或** basename（assumptions F 群節）：
    // "s*b/f3" 在 uutils 只對 basename 有意义 → 我方以 **f3 的 basename** 案例驗證
    let r = du(&["-a", "--exclude", "{f1,f2}", "--exclude", "*f3", &format!("{root}/d")]);
    let rs = rows(&r);
    assert!(!rs.iter().any(|(_, p)| p.ends_with("/f1") || p.ends_with("/f2")));
    assert!(!rs.iter().any(|(_, p)| p.ends_with("/f3")));
    let r = du(&["-a", "--exclude", "*x1", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
    std::fs::write(format!("{root}/d/ax1"), b"z").unwrap();
    let r = du(&["-a", "--exclude", "*x1", &format!("{root}/d")]);
    assert!(!rows(&r).iter().any(|(_, p)| p.ends_with("/ax1")));
    // brace 內空 alternate（{a,}）
    let r = du(&["-a", "--exclude", "{ax1,}", &format!("{root}/d")]);
    assert_eq!(r.code, 0);
}
