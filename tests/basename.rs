//! basename(1) e2e tests（guest 內執行）。
use coreutils::testutil as t;

fn b(args: &[&str]) -> t::Out {
    t::run("basename", args, "")
}

#[test]
fn man_page_examples() {
    // man page / GNU info 的範例
    assert_eq!(b(&["/usr/bin/sort"]).out(), "sort\n");
    assert_eq!(b(&["include/stdio.h", ".h"]).out(), "stdio\n");
    // 尾斜線先去掉，再做 SUFFIX 比對
    assert_eq!(b(&["/a/b/", ".b"]).out(), "b\n");
    assert_eq!(b(&["/a/b.c", ".x"]).out(), "b.c\n");
    assert_eq!(b(&["/a/b/"]).out(), "b\n");
    assert_eq!(b(&["/a//b//"]).out(), "b\n");
}

#[test]
fn edge_names() {
    assert_eq!(b(&["//"]).out(), "/\n");
    assert_eq!(b(&["/"]).out(), "/\n");
    assert_eq!(b(&[""]).out(), "\n");
    // SUFFIX 與 NAME 相同時不鋸
    assert_eq!(b(&[".h", ".h"]).out(), ".h\n");
    // SUFFIX 含斜線 → 永遠不比對
    assert_eq!(b(&["a/b.h", "/b.h"]).out(), "b.h\n");
}

#[test]
fn multiple_and_suffix_options() {
    assert_eq!(b(&["-a", "/a/b", "/c/d"]).out(), "b\nd\n");
    // -s 代稱 --multiple（GNU）
    assert_eq!(b(&["-s", ".h", "a.h", "b.h"]).out(), "a\nb\n");
    assert_eq!(b(&["--suffix", ".h", "a.h", "b.h/c.h"]).out(), "a\nc\n");
    assert_eq!(b(&["-as", ".h", "x.h"]).out(), "x\n");
    // -z 用 NUL 分隔；沒有 -a 時仍是 legacy 兩 operand 形式（第二個是 SUFFIX，
    // 含 `/` 不比對）→ 只輸出一個名字。GNU 與 uutils 實測皆 `b\0`。
    assert_eq!(b(&["-z", "/a/b", "/c/d"]).out_bytes(), b"b\0");
    // legacy 兩 operand 形式：第二個是 SUFFIX
    assert_eq!(b(&["/a/b", "/c/d"]).out(), "b\n");
}

#[test]
fn option_terminator_and_errors() {
    assert_eq!(b(&["--", "-a"]).out(), "-a\n");
    assert_eq!(b(&["-a", "--", "-a"]).out(), "-a\n");
    let o = b(&[]);
    assert_eq!(o.code, 1);
    assert!(o.err().contains("missing operand"), "{}", o.err());
    let o = b(&["/a/b", "/c/d", ".c"]);
    assert_eq!(o.code, 1);
    assert!(o.err().contains("extra operand '.c'"), "{}", o.err());
    // GNU getopt 預設會 permutation：operand 後的 `-a` 仍被當成選項，
    // 只輸出一個名字。GNU 與 uutils 實測皆 `b`。
    assert_eq!(b(&["/a/b", "-a"]).out(), "b\n");
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("basename", &["/usr/bin/sort"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.out(), "sort\n");
    let o = t::run_bin("basename", &["-s", ".h", "a.h"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.out(), "a\n");
    let o = t::run_bin("basename", &["--version"], "");
    assert_eq!(o.code, 0);
    assert!(o.out().contains("basename"), "{}", o.out());
    let o = t::run_bin("basename", &["--help"], "");
    assert_eq!(o.code, 0);
    assert!(o.out().contains("Usage"), "{}", o.out());
}

/// fuzz 不變量：輸出絕不含 `/`，且是「最後一個 component 去掉 SUFFIX」的結果。
#[test]
fn fuzz_output_is_slashfree_suffix_of_last_component() {
    let mut rng = t::Rng::new(0x62_6173_65);
    for round in 0..200 {
        let ncomp = 1 + rng.below(4);
        let mut comps: Vec<String> = Vec::new();
        for _ in 0..ncomp {
            let len = 1 + rng.below(6);
            let mut s = String::new();
            for _ in 0..len {
                s.push((b'a' + rng.below(26) as u8) as char);
            }
            comps.push(s);
        }
        let absolute = rng.below(2) == 0;
        let mut name = if absolute { String::from("/") } else { String::new() };
        name.push_str(&comps.join("/"));
        if rng.below(4) == 0 {
            name.push('/'); // 尾斜線
        }
        let suffix = if rng.below(2) == 0 {
            comps[comps.len() - 1][1..].to_string()
        } else {
            String::new()
        };
        let args: Vec<String> = if suffix.is_empty() {
            vec![name.clone()]
        } else {
            vec![String::from("-s"), suffix.clone(), name.clone()]
        };
        let argsr: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let o = t::run("basename", &argsr, "");
        assert_eq!(o.code, 0, "round {round} name={name}");
        let out = o.out().trim_end_matches('\n').to_string();
        assert!(!out.contains('/'), "round {round} name={name} out={out}");
        let last = comps[comps.len() - 1].clone();
        assert!(last.starts_with(&out), "round {round} last={last} out={out}");
        if !suffix.is_empty() {
            // 去掉 SUFFIX 後剩下的長度
            assert_eq!(
                out.len() + suffix.len(),
                if last.ends_with(&suffix) && last != suffix {
                    last.len()
                } else {
                    last.len() + suffix.len()
                },
                "round {round} last={last} suffix={suffix} out={out}"
            );
        }
    }
}
