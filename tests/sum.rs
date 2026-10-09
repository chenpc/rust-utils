use coreutils::testutil as t;

#[test]
fn bsd_and_sysv_known_formats() {
    let input = b"hello world\n";
    let r = t::run_bytes("sum", &["-",], input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "03762     1\n");
    let s = t::run_bytes("sum", &["-s", "-"], input);
    assert_eq!(s.code, 0);
    assert_eq!(s.out(), "1126 1\n");
}

#[test]
fn check_mode_is_not_supported() {
    // sum 的 man page SYNOPSIS 只有 -r / -s / --sysv，沒有 -c
    let r = t::run("sum", &["-c", "-"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "sum: invalid option -- 'c'\n");
}

#[test]
fn fuzz_blocks_match_man_page_sizes() {
    let mut rng = t::Rng::new(0x51A2);
    for _ in 0..60 {
        let n = 1 + rng.below(4000) as usize;
        let data = rng.bytes(n);
        let len = data.len();
        let bsd = t::run_bytes("sum", &["-r", "-"], &data);
        let sysv = t::run_bytes("sum", &["-s", "-"], &data);
        assert_eq!(bsd.code, 0);
        assert_eq!(sysv.code, 0);
        // 不變量：BSD 用 1024 位元組 block，System V 用 512
        let o1 = bsd.out(); let parts: Vec<&str> = o1.split_whitespace().collect();
        assert_eq!(parts[1].parse::<usize>().unwrap(), (len + 1023) / 1024, "bsd blocks wrong out={}", bsd.out());
        let o2 = sysv.out(); let parts2: Vec<&str> = o2.split_whitespace().collect(); assert_eq!(parts2[1].parse::<usize>().unwrap(), (len + 511) / 512, "sysv blocks wrong out={}", sysv.out());
        // 確定性
        let again = t::run_bytes("sum", &["-r", "-"], &data);
        assert_eq!(again.stdout, bsd.stdout);
    }
}
