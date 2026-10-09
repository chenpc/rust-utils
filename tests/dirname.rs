//! dirname(1) e2e tests（guest 內執行）。
use coreutils::testutil as t;

fn d(args: &[&str]) -> t::Out {
    t::run("dirname", args, "")
}

#[test]
fn man_page_examples() {
    assert_eq!(d(&["/usr/bin/sort"]).out(), "/usr/bin\n");
    assert_eq!(d(&["usr/bin/sort/"]).out(), "usr/bin\n");
    // GNU 與 uutils 實測皆為 `/usr/bin//.`：先去尾斜線，截掉最後一個 component，
    // 內部重複斜線不collapse（GNU info 例：`///usr//bin//` → `///usr/bin`）。
    assert_eq!(d(&["/usr/bin//././/"]).out(), "/usr/bin//.\n");
    assert_eq!(d(&["stdio.h"]).out(), ".\n");
    assert_eq!(d(&["./"]).out(), ".\n");
    assert_eq!(d(&["./."]).out(), ".\n");
    assert_eq!(d(&[".."]).out(), ".\n");
}

#[test]
fn slash_only_and_edge() {
    assert_eq!(d(&["//"]).out(), "/\n");
    assert_eq!(d(&["///"]).out(), "/\n");
    assert_eq!(d(&["/"]).out(), "/\n");
    assert_eq!(d(&[""]).out(), ".\n");
    assert_eq!(d(&["/a"]).out(), "/\n");
    assert_eq!(d(&["///a"]).out(), "/\n");
    assert_eq!(d(&["a//b"]).out(), "a\n");
    assert_eq!(d(&["/a/b///"]).out(), "/a\n");
    assert_eq!(d(&["a/b/"]).out(), "a\n");
}

#[test]
fn multiple_operands_and_zero() {
    assert_eq!(d(&["dir1/str", "dir2/str"]).out(), "dir1\ndir2\n");
    assert_eq!(d(&["-z", "a/b", "c/d"]).out_bytes(), b"a\0c\0");
    assert_eq!(d(&["--zero", "a/b"]).out_bytes(), b"a\0");
}

#[test]
fn errors_and_terminator() {
    let o = d(&[]);
    assert_eq!(o.code, 1);
    assert!(o.err().contains("missing operand"), "{}", o.err());
    assert_eq!(d(&["--", "-a"]).out(), ".\n");
    let o = d(&["-x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("invalid option"), "{}", o.err());
    let o = d(&["--nope"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("unrecognized option"), "{}", o.err());
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("dirname", &["/usr/bin/sort"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.out(), "/usr/bin\n");
    let o = t::run_bin("dirname", &["-z", "a/b"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.out_bytes(), b"a\0");
    let o = t::run_bin("dirname", &["--version"], "");
    assert_eq!(o.code, 0);
    assert!(o.out().contains("dirname"), "{}", o.out());
    let o = t::run_bin("dirname", &["--help"], "");
    assert_eq!(o.code, 0);
    assert!(o.out().contains("Usage"), "{}", o.out());
    let o = t::run_bin("dirname", &["-h"], "");
    assert_eq!(o.code, 0);
    let o = t::run_bin("dirname", &["-V"], "");
    assert_eq!(o.code, 0);
}

/// fuzz 不變量：dirname 的輸出不含尾斜線（除非就是 "/"），
/// 且 dirname(X) 與 basename(X) 拼回去掉多餘斜線後的 X。
#[test]
fn fuzz_dirname_basename_reconstructs() {
    let mut rng = t::Rng::new(0x6469_726E_616D_65);
    for _ in 0..200 {
        let ncomp = 1 + rng.below(4);
        let mut comps: Vec<String> = Vec::new();
        for _ in 0..ncomp {
            let len = 1 + rng.below(5);
            let mut s = String::new();
            for _ in 0..len {
                s.push((b'a' + rng.below(26) as u8) as char);
            }
            comps.push(s);
        }
        let absolute = rng.below(2) == 0;
        let mut name = if absolute { String::from("/") } else { String::new() };
        name.push_str(&comps.join("/"));
        // 隨機插入多餘斜線與尾斜線
        if rng.below(3) == 0 {
            name = name.replace('/', "//");
        }
        if rng.below(3) == 0 {
            name.push('/');
        }
        let dir = t::run("dirname", &[name.as_str()], "").out();
        let dir = dir.trim_end_matches('\n').to_string();
        let base = t::run("basename", &[name.as_str()], "").out();
        let base = base.trim_end_matches('\n').to_string();
        assert!(!dir.ends_with('/') || dir == "/", "name={name} dir={dir}");
        // 重組：把 dirname 與 basename 用一個斜線接起來，正規化後應等於原路徑的正規化
        let joined = if dir == "/" {
            format!("/{base}")
        } else if dir == "." && !name.starts_with('/') {
            base.clone()
        } else {
            format!("{dir}/{base}")
        };
        let norm = |s: &str| -> String {
            let mut out = String::new();
            for part in s.split('/') {
                if part.is_empty() {
                    if out.is_empty() {
                        out.push('/');
                    }
                    continue;
                }
                if !out.is_empty() && !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(part);
            }
            if out.is_empty() {
                out.push('/');
            }
            out
        };
        assert_eq!(norm(&joined), norm(&name), "name={name} dir={dir} base={base}");
    }
}
