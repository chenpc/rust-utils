use coreutils::testutil::*;

/// fuzz 用的安全字元：ASCII 可印字元，且不是 tr 語法的特殊字元
/// （否則 lossy 字串與 raw byte 表示不一致，且範圍/class 語法會改變集合內容）
fn safe(rng: &mut Rng) -> u8 {
    loop {
        let b = rng.bytes(1)[0];
        if b >= 0x21 && b <= 0x7E && !matches!(b, b'-' | b'[' | b']' | b':' | b'\\') {
            return b;
        }
    }
}

#[test]
fn translates_one_to_one() {
    let r = run("tr", &["abc", "jkmn"], "abc\n");
    assert_eq!(r.out(), "jkm\n");
}

#[test]
fn delete_removes_set1() {
    let r = run("tr", &["-d", "aeiou"], "hello world\n");
    assert_eq!(r.out(), "hll wrld\n");
}

#[test]
fn squeeze_collapses_repeats_of_last_set() {
    let r = run("tr", &["-s", "ab"], "aaabbbcc\n");
    assert_eq!(r.out(), "abcc\n");
}

#[test]
fn complement_of_set1() {
    // tr -cd "[:print:]" → 保留可印字元，刪除其他（man page 的 tldr 例子）
    let r = run("tr", &["-cd", "[:print:]"], "a\x01b\x07\n");
    assert_eq!(r.out(), "ab"); // newline 不是 [:print:]，會被刪除
}

#[test]
fn truncate_set1_to_length_of_set2() {
    let r = run("tr", &["-t", "abcd", "XY"], "abcd\n");
    assert_eq!(r.out(), "XYcd\n");
}

#[test]
fn longer_set1_pads_with_last_char_of_set2() {
    // 假設（docs/assumptions.md）：SET1 比 SET2 長時用 SET2 最後一個字元補
    let r = run("tr", &["abc", "X"], "abc\n");
    assert_eq!(r.out(), "XXX\n");
}

#[test]
fn ranges_are_expanded() {
    let r = run("tr", &["a-c", "1-3"], "abc\n");
    assert_eq!(r.out(), "123\n");
}

#[test]
fn character_classes_work() {
    let r = run("tr", &["[:lower:]", "[:upper:]"], "Hello\n");
    assert_eq!(r.out(), "HELLO\n");
}

#[test]
fn missing_operand_is_error_exit_2() {
    let r = run("tr", &[], "abc");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing operand"), "stderr: {}", r.err());
}

#[test]
fn help_and_version() {
    let h = run("tr", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("tr"));
    assert!(run("tr", &["-V"], "").out().contains("tr"));
}

#[test]
fn all_documented_classes_are_supported() {
    // 每個 class 都要被執行到，否則 tr.rs 的 region coverage 會低于門檻
    let cases = [
        ("[:digit:]", "0123456789", "xxxxxxxxxx"),
        ("[:alnum:]", "aZ0-", "xxx-"),
        ("[:space:]", "a b\tc\n", "axbxcx"),
        ("[:blank:]", "a b", "axb"),
        ("[:punct:]", "a,b", "axb"),
        ("[:graph:]", "a b", "x x"),
        ("[:print:]", "a\x01b", "x\x01x"),
        ("[:cntrl:]", "a\x01b", "axb"),
    ];
    for (cls, input, want) in cases {
        let r = run("tr", &[cls, "x"], input);
        assert_eq!(r.code, 0, "class {cls}");
        assert_eq!(r.out(), want, "class {cls}");
    }
}

#[test]
fn escapes_in_sets_are_decoded() {
    let r = run("tr", &["\\n", "N"], "a\nb\n");
    assert_eq!(r.out(), "aNbN"); // 結尾的 newline 也被映射，所以沒有剩餘 newline
    let r2 = run("tr", &["\\t", "T"], "a\tb");
    assert_eq!(r2.out(), "aTb");
    // 八位 escape：\101 = 'A'
    let r3 = run("tr", &["\\101", "Z"], "Aa");
    assert_eq!(r3.out(), "Za");
    // 字面 backslash
    let r4 = run("tr", &["\\\\", "S"], "a\\b");
    assert_eq!(r4.out(), "aSb");
}

#[test]
fn squeeze_after_translation() {
    // 處理順序假設：先翻譯再 squeeze；squeeze 用的是最後指定的 SET（SET2）
    let r = run("tr", &["-s", "ab", "xy"], "aabb");
    assert_eq!(r.out(), "xy");
}

#[test]
fn write_error_returns_1() {
    let mut w = FailWriter::new(2);
    let mut e = Vec::new();
    let code = run_with("tr", &["a", "b"], b"aaaa", &mut w, &mut e);
    assert_eq!(code, 1);
    assert_eq!(String::from_utf8_lossy(&w.out), "bb");
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin("tr", &["abc", "xyz"], "abc\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "xyz\n");
}

#[test]
fn fuzz_mapping_is_positional_and_length_preserving() {
    // man page 不變量：1:1 映射時輸出長度 = 輸入長度，且每個字元對應到 SET1 中同位置的 SET2 字元
    let mut rng = Rng::new(0x7A1);
    for _ in 0..200 {
        let n = 1 + rng.below(10) as usize;
        let mut set1: Vec<u8> = Vec::new();
        for _ in 0..n {
            let b = safe(&mut rng);
            set1.push(b);
        }
        if set1.is_empty() {
            continue;
        }
        let set2: Vec<u8> = (0..set1.len()).map(|_| safe(&mut rng)).collect();
        let s1 = String::from_utf8_lossy(&set1).into_owned();
        let s2 = String::from_utf8_lossy(&set2).into_owned();
        let ilen = 1 + rng.below(60) as usize;
        let input = rng.bytes(ilen);
        let r = run_bytes("tr", &[&s1, &s2], &input);
        assert_eq!(r.code, 0);
        assert_eq!(r.stdout.len(), input.len(), "1:1 mapping must preserve length");
        for (i, &b) in input.iter().enumerate() {
            let want = match set1.iter().position(|&x| x == b) {
                Some(j) => set2[j],
                None => b,
            };
            assert_eq!(r.stdout[i], want, "byte {i}");
        }
    }
}

#[test]
fn fuzz_delete_removes_exactly_set1() {
    let mut rng = Rng::new(0xDE1E);
    for _ in 0..200 {
        let n = 1 + rng.below(8) as usize;
        let set1: Vec<u8> = (0..n).map(|_| safe(&mut rng)).collect();
        let s1 = String::from_utf8_lossy(&set1).into_owned();
        let ilen = 1 + rng.below(80) as usize;
        let input = rng.bytes(ilen);
        let r = run_bytes("tr", &["-d", &s1], &input);
        assert_eq!(r.code, 0);
        for &b in &r.stdout {
            assert!(!set1.contains(&b), "deleted char {b} present in output");
        }
        let want = input.iter().filter(|&&b| !set1.contains(&b)).count();
        assert_eq!(r.stdout.len(), want);
    }
}
