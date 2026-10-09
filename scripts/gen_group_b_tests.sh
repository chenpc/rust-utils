#!/usr/bin/env bash
# 產生 Group B 的 applet 測試：每個 applet 三類斷言
#   1) 輸出與核心 digest 一致（格式由 man page 推導）
#   2) check mode 往返（man page 的 -c 語意）
#   3) fuzz 不變量：確定性 + 輸出長度 + 自己產生的清單能被自己驗證
set -u
cd "$(dirname "$0")/.."

gen() {
  applet="$1"; algo="$2"
  cat > "tests/${applet}.rs" << 'EOF'
use coreutils::hashsum as h;
use coreutils::testutil as t;

const APPLET: &str = "APPLET_NAME";
const ALGO: &h::Algo = &h::ALGO_NAME;

#[test]
fn output_matches_core_digest() {
    let input = b"hello world\n";
    let r = t::run_bytes(APPLET, &[], input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), format!("{}  -\n", h::hex_of(&h::digest(ALGO, input, 0))));
}

#[test]
fn check_mode_round_trip() {
    let w = t::workdir();
    let f = w.join("in.bin");
    std::fs::write(&f, b"hello world\n").unwrap();
    let r = t::run(APPLET, &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    let list = w.join("list.txt");
    std::fs::write(&list, &r.stdout).unwrap();
    let c = t::run(APPLET, &["-c", list.to_str().unwrap()], "");
    assert_eq!(c.code, 0);
    assert_eq!(c.out(), format!("{}: OK\n", f.to_str().unwrap()));
}

#[test]
fn fuzz_is_deterministic_and_self_checkable() {
    let mut rng = t::Rng::new(0xB0B0);
    let w = t::workdir();
    for i in 0..40 {
        let n = 1 + rng.below(200) as usize;
        let data = rng.bytes(n);
        let a = t::run_bytes(APPLET, &[], &data);
        let b = t::run_bytes(APPLET, &[], &data);
        assert_eq!(a.code, 0);
        assert_eq!(a.stdout, b.stdout, "not deterministic at iteration {}", i);
        assert_eq!(a.stdout.len(), ALGO.bits / 4 + 4, "hex length wrong at iteration {}", i);

        // 不變量：applet 自己印出的清單，必須能被自己的 -c 驗證通過
        let f = w.join(format!("fz{}.bin", i));
        std::fs::write(&f, &data).unwrap();
        let r = t::run(APPLET, &[f.to_str().unwrap()], "");
        assert_eq!(r.code, 0);
        assert_eq!(r.out(), format!("{}  {}\n", h::hex_of(&h::digest(ALGO, &data, 0)), f.to_str().unwrap()));
        let list = w.join(format!("fz{}.txt", i));
        std::fs::write(&list, &r.stdout).unwrap();
        let c = t::run(APPLET, &["-c", list.to_str().unwrap()], "");
        assert_eq!(c.code, 0, "self-check failed at iteration {}", i);
    }
}
EOF
  sed -i "s/APPLET_NAME/$applet/; s/ALGO_NAME/$algo/" "tests/${applet}.rs"
}

for pair in "md5sum:MD5" "sha1sum:SHA1" "sha224sum:SHA224" "sha256sum:SHA256" "sha384sum:SHA384" "sha512sum:SHA512" "b2sum:BLAKE2B" "b3sum:BLAKE3" "sha3sum:SHA3_256" "sha3-224sum:SHA3_224" "sha3-256sum:SHA3_256" "sha3-384sum:SHA3_384" "sha3-512sum:SHA3_512" "shake128sum:SHAKE128" "shake256sum:SHAKE256"; do
  gen "${pair%%:*}" "${pair##*:}"
done

# hashsum：演算法由 selector 決定，另外測 --no-names 與 --bits
cat > tests/hashsum.rs << 'EOF'
use coreutils::hashsum as h;
use coreutils::testutil as t;

#[test]
fn requires_an_algorithm() {
    let r = t::run_bytes("hashsum", &["-"], b"hello world\n");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "hashsum: no algorithm specified\n");
}

#[test]
fn selector_last_wins() {
    let r = t::run_bytes("hashsum", &["--sha256", "--md5", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "6f5902ac237024bdd0c176cb93063dc4  -\n");
}

#[test]
fn no_names_omits_filename() {
    let r = t::run_bytes("hashsum", &["--no-names", "--sha256", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447\n");
}

#[test]
fn bits_only_for_shake() {
    let r = t::run_bytes("hashsum", &["--bits", "64", "--shake128", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().len(), 16 + 4);
    let bad = t::run_bytes("hashsum", &["--bits", "64", "--sha256", "-"], b"hello world\n");
    assert_eq!(bad.code, 1);
}

#[test]
fn fuzz_selector_matches_core() {
    let mut rng = t::Rng::new(0x4A5B);
    for _ in 0..40 {
        let n = 1 + rng.below(200) as usize;
        let data = rng.bytes(n);
        for (sel, algo) in [("--md5", &h::MD5), ("--sha256", &h::SHA256), ("--b3sum", &h::BLAKE3), ("--shake256", &h::SHAKE256)] {
            let r = t::run_bytes("hashsum", &[sel, "-"], &data);
            assert_eq!(r.code, 0);
            assert_eq!(r.out(), format!("{}  -\n", h::hex_of(&h::digest(algo, &data, 0))));
        }
    }
}
EOF

# sum：blocks 不變量（man page：BSD 1024、System V 512）
cat > tests/sum.rs << 'EOF'
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
EOF

# cksum：數值格式 + check 往返
cat > tests/cksum.rs << 'EOF'
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
        // 不變量：tagged 輸出可被自己的 -c 讀回
        let tagged = t::run_bytes("cksum", &["-a", "md5", "-"], &data);
        assert_eq!(tagged.code, 0);
        let w = t::workdir();
        let list = w.join("fz.txt");
        std::fs::write(&list, &tagged.stdout).unwrap();
        let c = t::run("cksum", &["-c", list.to_str().unwrap()], "");
        assert_eq!(c.code, 0);
    }
}
EOF
echo "generated $(ls tests/*.rs | wc -l) test files"
