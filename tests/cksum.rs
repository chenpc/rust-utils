use coreutils::hashsum as h;
use coreutils::testutil as t;

#[test]
fn crc_format_and_blocks() {
    let input = b"hello world\n";
    let r = t::run_bytes("cksum", &["-"], input);
    assert_eq!(r.code, 0);
    // crc 的初始值/多項式由實作決定（man page 未定義），此處與核心保持一致
    let v = u32::from_le_bytes(h::digest(&h::CRC, b"hello world\n", 0)[..4].try_into().unwrap());
    assert_eq!(r.out(), format!("{} 12\n", v));
}

#[test]
fn tagged_and_untagged() {
    let input = b"hello world\n";
    let r = t::run_bytes("cksum", &["-a", "md5", "-"], input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "MD5 (-) = 6f5902ac237024bdd0c176cb93063dc4\n");
    let u = t::run_bytes("cksum", &["--untagged", "-a", "md5", "-"], input);
    assert_eq!(u.code, 0);
    assert_eq!(u.out(), "6f5902ac237024bdd0c176cb93063dc4  -\n");
}

#[test]
fn sha2_requires_length() {
    let r = t::run("cksum", &["-a", "sha2", "-"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "cksum: --algorithm=sha2 requires specifying --length 224, 256, 384, or 512\n");
}

#[test]
fn check_mode_round_trip() {
    let w = t::workdir();
    let f = w.join("in.bin");
    std::fs::write(&f, b"hello world\n").unwrap();
    let r = t::run("cksum", &["-a", "sha256", f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    let list = w.join("list.txt");
    std::fs::write(&list, &r.stdout).unwrap();
    let c = t::run("cksum", &["-c", list.to_str().unwrap()], "");
    assert_eq!(c.code, 0);
    assert_eq!(c.out(), format!("{}: OK\n", f.to_str().unwrap()));
}

#[test]
fn fuzz_numeric_and_tagged_are_stable() {
    let mut rng = t::Rng::new(0xC45A);
    for _ in 0..40 {
        let n = 1 + rng.below(300) as usize;
        let data = rng.bytes(n);
        let a = t::run_bytes("cksum", &["-"], &data);
        let b = t::run_bytes("cksum", &["-"], &data);
        assert_eq!(a.code, 0);
        assert_eq!(a.stdout, b.stdout);
        let o = a.out(); let parts: Vec<&str> = o.split_whitespace().collect();
        assert_eq!(parts[1].parse::<usize>().unwrap(), data.len());
        let v = parts[0].parse::<u32>().unwrap();
        assert_eq!(v, u32::from_le_bytes(h::digest(&h::CRC, &data, 0)[..4].try_into().unwrap()));
        // 不變量：tagged 輸出可被自己的 -c 讀回（check mode 會讀「檔案」，所以輸入要先落地成檔）
        let w = t::workdir();
        let f = w.join("fz.bin");
        std::fs::write(&f, &data).unwrap();
        let tagged = t::run("cksum", &["-a", "md5", f.to_str().unwrap()], "");
        assert_eq!(tagged.code, 0);
        let list = w.join("fz.txt");
        std::fs::write(&list, &tagged.stdout).unwrap();
        let c = t::run("cksum", &["-c", list.to_str().unwrap()], "");
        assert_eq!(c.code, 0);
        assert_eq!(c.out(), format!("{}: OK\n", f.to_str().unwrap()));
    }
}

#[test]
fn check_mode_gnu_warnings() {
    // man page 沒寫摘要警告；依實測參考（uutils 0.10.0）的 GNU 行為：
    // 順序 = improperly formatted → computed checksum did NOT match → listed file could not be read
    let w = t::workdir();
    let f = w.join("a1");
    std::fs::write(&f, b"hello world\n").unwrap();
    let list = w.join("list.txt");
    std::fs::write(
        &list,
        format!(
            "MD5 ({}) = 00000000000000000000000000000000\nMD5 ({}) = 6f5902ac237024bdd0c176cb93063dc4\nbadline\n",
            f.to_str().unwrap(),
            w.join("missing").to_str().unwrap()
        ),
    )
    .unwrap();
    let r = t::run("cksum", &["-c", list.to_str().unwrap()], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.out(),
        format!("{}: FAILED\n{}: FAILED open or read\n", f.to_str().unwrap(), w.join("missing").to_str().unwrap())
    );
    assert_eq!(
        r.err(),
        format!(
            "cksum: {}: No such file or directory\ncksum: WARNING: 1 line is improperly formatted\ncksum: WARNING: 1 computed checksum did NOT match\ncksum: WARNING: 1 listed file could not be read\n",
            w.join("missing").to_str().unwrap()
        )
    );

    // --status 抑制摘要警告，但不抑制 FAILED open or read
    let s = t::run("cksum", &["--status", "-c", list.to_str().unwrap()], "");
    assert_eq!(s.code, 1);
    assert_eq!(s.out(), format!("{}: FAILED open or read\n", w.join("missing").to_str().unwrap()));
    // --quiet 只抑制 OK：FAILED 與摘要警告都照印
    let q = t::run("cksum", &["--quiet", "-c", list.to_str().unwrap()], "");
    assert_eq!(q.code, 1);
    assert_eq!(q.out(), format!("{}: FAILED\n{}: FAILED open or read\n", f.to_str().unwrap(), w.join("missing").to_str().unwrap()));

    // 整份清單都不可解析 → 只有 no properly formatted，不再印摘要警告
    let bad = w.join("bad.txt");
    std::fs::write(&bad, "notahashline\n").unwrap();
    let b = t::run("cksum", &["-c", bad.to_str().unwrap()], "");
    assert_eq!(b.code, 1);
    assert_eq!(b.out(), "");
    assert_eq!(b.err(), format!("cksum: {}: no properly formatted checksum lines found\n", bad.to_str().unwrap()));

    // --ignore-missing 且完全沒驗證 → no file was verified
    let im = w.join("im.txt");
    std::fs::write(&im, format!("MD5 ({}) = 6f5902ac237024bdd0c176cb93063dc4\n", w.join("missing").to_str().unwrap())).unwrap();
    let i = t::run("cksum", &["-c", "--ignore-missing", im.to_str().unwrap()], "");
    assert_eq!(i.code, 1);
    assert_eq!(i.out(), "");
    assert_eq!(i.err(), format!("cksum: {}: no file was verified\n", im.to_str().unwrap()));
}
