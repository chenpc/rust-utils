//! touch：-d/-t/-r/-a/-m/-c、建檔、錯誤措辭。
use coreutils::testutil as t;
use std::os::unix::fs::MetadataExt;

fn times(p: &str) -> (i64, i64) {
    let m = std::fs::symlink_metadata(p).unwrap();
    (m.atime(), m.mtime())
}

fn setup(d: &str) {
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
}

#[test]
fn create_and_epoch() {
    let d = "/tmp/touch_t1";
    setup(d);
    let p = format!("{}/new", d);
    let r = t::run("touch", &[&p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let m = std::fs::metadata(&p).unwrap();
    assert_eq!(m.len(), 0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (_, mt) = times(&p);
    assert!((mt - now).abs() < 120, "mtime 應接近現在：{} vs {}", mt, now);
    let p2 = format!("{}/ep", d);
    let r = t::run("touch", &["-d", "@100", &p2], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p2), (100, 100));
}

#[test]
fn date_and_stamp() {
    let d = "/tmp/touch_t2";
    setup(d);
    // ISO 本地時間：期望值由實作同一語意推算 → 用 date 本地解析可預期 946782245（TZ=Asia/Taipei 環境）
    // guest/host 時區不確定，改測往返一致性：寫入後讀回再觸碰比較
    let p = format!("{}/iso", d);
    let r = t::run("touch", &["-d", "2000-01-02 03:04:05", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let (_, mt) = times(&p);
    // 與 epoch 版本比較：@946753445 = 2000-01-02T03:04:05Z
    let p2 = format!("{}/utc", d);
    let r = t::run("touch", &["-d", "@946753445", &p2], "");
    assert_eq!(r.code, 0);
    let (_, mt2) = times(&p2);
    let delta = mt - mt2;
    assert!(delta % 3600 == 0 && delta.abs() <= 14 * 3600, "ISO 本地解析偏移應為整小時：{}", delta);
    // -t [[CC]YY]MMDDhhmm[.SS]：與等價 -d ISO（本地時區）比較，規避 TZ 差異
    let p3 = format!("{}/stamp", d);
    let r = t::run("touch", &["-t", "200001020304.05", &p3], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p3), (mt, mt), "-t 與 -d ISO 同一刻");
    let p4 = format!("{}/yy", d);
    let r = t::run("touch", &["-t", "9901010101", &p4], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let p5 = format!("{}/yy_d", d);
    let r = t::run("touch", &["-d", "1999-01-01 01:01:00", &p5], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p4).1, times(&p5).1, "YY>=69 → 19xx");
    let r = t::run("touch", &["-t", "garbage", format!("{}/iso", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "touch: invalid date format 'garbage'");
    let r = t::run("touch", &["-d", "garbage zz", &p], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "touch: Unable to parse date: garbage zz");
}

#[test]
fn atime_mtime_and_reference() {
    let d = "/tmp/touch_t3";
    setup(d);
    let p = format!("{}/f", d);
    let _ = t::run("touch", &["-d", "@500", &p], "");
    let r = t::run("touch", &["-a", "-d", "@100", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p), (100, 500), "-a 只改 atime");
    let r = t::run("touch", &["-m", "-d", "@200", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p), (100, 200), "-m 只改 mtime");
    let rf = format!("{}/ref", d);
    let _ = t::run("touch", &["-d", "@300", &rf], "");
    let r = t::run("touch", &["-r", &rf, &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p), (300, 300));
    let r = t::run("touch", &["--date=@400", "--no-create", format!("{}/newer", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(!std::path::Path::new(&format!("{}/newer", d)).exists(), "-c 不应建档");
}

#[test]
fn errors_and_misc() {
    let d = "/tmp/touch_t4";
    setup(d);
    let r = t::run("touch", &[format!("{}/nodir_zz/f", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!("touch: cannot touch '{}/nodir_zz/f': No such file or directory", d)
    );
    let r = t::run("touch", &["-c", format!("{}/nodir_zz/f", d).as_str()], "");
    assert_eq!(r.code, 0, "-c 不存在檔應成功：{}", r.err());
    let r = t::run("touch", &[], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "touch: missing file operand");
    let r = t::run("touch", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"));
    let r = t::run("touch", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "touch (uutils coreutils) 0.10.0"));
    let r = t::run("touch", &["-x", format!("{}/f", d).as_str()], "");
    assert_eq!(r.code, 2);
    // -h 對 symlink 觸碰本體
    let sl = format!("{}/sl", d);
    let tgt = format!("{}/tgt", d);
    let _ = t::run("touch", &["-d", "@600", &tgt], "");
    std::os::unix::fs::symlink(&tgt, &sl).unwrap();
    let r = t::run("touch", &["-h", "-d", "@700", &sl], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&sl).1, 700);
    assert_eq!(times(&tgt).1, 600, "-h 不影響目標");
}

#[test]
fn fuzz_epoch_roundtrip() {
    let d = "/tmp/touch_fz";
    setup(d);
    let mut rng = t::Rng::new(20261014);
    for i in 0..30 {
        let e = rng.below(2_000_000_000) as i64;
        let p = format!("{}/f{}", d, i);
        let spec = format!("@{}", e);
        let r = t::run("touch", &["-d", &spec, &p], "");
        assert_eq!(r.code, 0, "{}: {}", spec, r.err());
        assert_eq!(times(&p), (e, e), "spec {}", spec);
    }
}

#[test]
fn option_edges() {
    let d = "/tmp/touch_t5";
    setup(d);
    // -- 之後全是 operand
    let p = format!("{}/--x", d);
    let r = t::run("touch", &["--", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(std::path::Path::new(&p).exists());
    // 長選項缺值
    let r = t::run("touch", &["--date"], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "touch: option requires an argument -- 'date'".to_string()));
    let r = t::run("touch", &["--reference"], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "touch: option requires an argument -- 'reference'".to_string()));
    // --date 吃掉下一參數當值 → 解析失敗
    let r = t::run("touch", &["--date", &format!("{}/f", d)], "");
    assert_eq!(r.code, 1);
    let r = t::run("touch", &["--reference", &format!("{}/f", d)], "");
    assert_eq!(r.code, 1);
    let r = t::run("touch", &["-d"], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "touch: option requires an argument -- 'd'".to_string()));
    // 長選項無效日期 / 未知選項
    let r = t::run("touch", &["--date=garbage zz", &format!("{}/f", d)], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (1, "touch: Unable to parse date: garbage zz".to_string()));
    let r = t::run("touch", &["--bogus", &format!("{}/f", d)], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (2, "touch: unrecognized option '--bogus'".to_string()));
    // -r 參考檔不存在
    let r = t::run("touch", &["-r", "/tmp/touch_noexist_zz", &format!("{}/f", d)], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (1, "touch: failed to get attributes of '/tmp/touch_noexist_zz': No such file or directory".to_string()));
}

#[test]
fn date_forms() {
    let d = "/tmp/touch_t6";
    setup(d);
    let eqtouch = |a: &str, b: &str, tag: &str| {
        let pa = format!("{}/a_{}", d, tag);
        let pb = format!("{}/b_{}", d, tag);
        let ra = t::run("touch", &["-d", a, &pa], "");
        let rb = t::run("touch", &["-d", b, &pb], "");
        assert_eq!(ra.code, 0, "{}: {}", a, ra.err());
        assert_eq!(rb.code, 0, "{}: {}", b, rb.err());
        assert_eq!(times(&pa), times(&pb), "{} vs {}", a, b);
    };
    eqtouch("2000-01-02", "2000-01-02 00:00:00", "dateonly");
    eqtouch("2000-01-02T03:04", "2000-01-02 03:04:00", "tsep");
    eqtouch("2000-01-02 03", "2000-01-02 03:00:00", "houronly");
    eqtouch("2000-01-02 03:04", "2000-01-02 03:04:00", "minsec");
    // 4 段时间 / 月越界 / 日期段不足 → 解析失敗
    for bad in ["2000-13-45 00:00:00", "01-02", "2000-01"] {
        let r = t::run("touch", &["-d", bad, format!("{}/x", d).as_str()], "");
        assert_eq!(r.code, 1, "bad {:?}", bad);
        assert!(r.err().contains("Unable to parse date"), "{}", r.err());
    }
    // -t 8 位（本年）與 12 位 CCYY
    let y = crate_year();
    let p8 = format!("{}/st8", d);
    let r = t::run("touch", &["-t", "01010101", &p8], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let p9 = format!("{}/st12", d);
    let spec12 = format!("{}01010101", y);
    let r = t::run("touch", &["-t", &spec12, &p9], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(times(&p8).1, times(&p9).1, "8 位stamp 用本年");
    let r = t::run("touch", &["-t", "123", format!("{}/x", d).as_str()], "");
    assert_eq!((r.code, r.err().trim_end().to_string()), (1, "touch: invalid date format '123'".to_string()));
}

fn crate_year() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // 400 年轮回的 civil 演算（只用來取今年年份，誤差一天無妨）
    let days = now.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp >= 10 { mp - 9 } else { mp + 3 };
    let y = if m <= 2 { y + 1 } else { y };
    y.to_string()
}
