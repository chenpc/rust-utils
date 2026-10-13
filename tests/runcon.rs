//! runcon(1)：`runcon [-c] [-u USER] [-r ROLE] [-t TYPE] [-l RANGE] COMMAND [ARG...]`。
//!
//! 對齊基準：本機參考 `/usr/lib/cargo/bin/coreutils/runcon`（uutils coreutils 0.10.0）。
//! 本機與 guest 都**沒有** SELinux（`/sys/fs/selinux` 不存在、`/proc/self/attr/current`
//! 讀到 `unconfined`），因此「實際 transition」與「exec COMMAND」兩條路徑不可達，
//! 記錄在 docs/assumptions.md。clap rc = 125。

use coreutils::testutil as t;
use std::path::PathBuf;

fn run(args: &[&str]) -> t::Out {
    t::run("runcon", args, "")
}

const USAGE: &str = "runcon CONTEXT COMMAND [ARG...]\n       runcon [-c] [-u USER] [-r ROLE] [-t TYPE] [-l RANGE] COMMAND [ARG...]";

fn current_context() -> Option<String> {
    std::fs::read_to_string("/proc/self/attr/current")
        .ok()
        .map(|s| s.trim_end_matches('\n').trim_end_matches('\0').to_string())
}

#[test]
fn help_and_version() {
    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    let out = h.out();
    assert!(out.starts_with("Run command with"), "{:?}", out);
    assert!(out.contains("-c, --compute"));
    assert!(out.contains("-u, --user <USER>"));
    assert!(out.contains("-l, --range <RANGE>"));
    assert_eq!(run(&["-h"]).out(), out);

    let v = run(&["--version"]);
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "runcon (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["-V"]).out(), v.out());
}

#[test]
fn without_command_prints_current_context() {
    let expect = match current_context() {
        Some(ctx) => format!("{}\n", ctx),
        None => "\n".to_string(),
    };
    for case in [
        vec![],
        vec!["-c"],
        vec!["--compute"],
        vec!["-u", "unconfined_u"],
        vec!["--user=unconfined_u"],
        vec!["-r", "object_r"],
        vec!["-t", "process_t"],
        vec!["-l", "s0"],
        vec!["--range=s0-s15"],
        vec!["-c", "-u", "u", "-r", "r", "-t", "ty", "-l", "s0"],
        vec!["-crl"],
        vec!["-ur"],
    ] {
        let r = run(&case);
        assert_eq!(r.code, 0, "{:?}", case);
        assert_eq!(r.out(), expect, "{:?}", case);
        assert_eq!(r.err(), "", "{:?}", case);
    }
}

#[test]
fn missing_command_is_reported() {
    for case in [
        vec!["s0"],
        vec!["user:role:type:s0"],
        vec!["--", "ctx"],
    ] {
        let r = run(&case);
        assert_eq!(r.code, 1, "{:?}", case);
        assert_eq!(r.out(), "");
        assert_eq!(r.err(), "runcon: No command is specified\n");
    }
    // 有旗標時 operand[0] 是 COMMAND（不是 context）→ 走到 SELinux 拒绝路徑
    if !PathBuf::from("/sys/fs/selinux").exists() {
        for case in [vec!["-c", "ctx"], vec!["--user=u", "ctx"], vec!["--", "s0", "-c"]] {
            let r = run(&case);
            assert_eq!(r.code, 1, "{:?}", case);
            assert_eq!(r.err(), "runcon: runcon may be used only on a SELinux kernel\n", "{:?}", case);
        }
    }
}

#[test]
fn command_execution_without_selinux_is_refused() {
    if PathBuf::from("/sys/fs/selinux").exists() {
        return;
    }
    for case in [
        vec!["s0", "/bin/true"],
        vec!["user:role:type:s0", "true"],
        vec!["-c", "true"],
        vec!["--compute", "true", "1"],
        vec!["-u", "u", "true"],
        vec!["-r", "r", "-t", "t", "true"],
        vec!["-l", "s0", "-c", "true"],
        vec!["-c", "no-such-program-xyz"],
        vec!["-c", "--", "true"],
        vec!["s0", "no-such-program-xyz"],
    ] {
        let r = run(&case);
        assert_eq!(r.code, 1, "{:?}", case);
        assert_eq!(r.out(), "");
        assert_eq!(
            r.err(),
            "runcon: runcon may be used only on a SELinux kernel\n",
            "{:?}",
            case
        );
    }
}

#[test]
fn option_parsing_follows_clap() {
    // 需要值的選項缺值
    for (short, long, metavar) in [
        ("-u", "user", "USER"),
        ("-r", "role", "ROLE"),
        ("-t", "type", "TYPE"),
        ("-l", "range", "RANGE"),
    ] {
        for args in [vec![short], vec![short, "-c"]] {
            let r = run(&args);
            assert_eq!(r.code, 125, "{:?}", args);
            assert_eq!(
                r.err(),
                format!(
                    "error: a value is required for '--{} <{}>' but none was supplied\n\nFor more information, try '--help'.\n",
                    long, metavar
                ),
                "{:?}",
                args
            );
        }
    }

    // 重複使用
    for (args, expect) in [
        (
            vec!["-c", "-c"],
            "error: the argument '--compute' cannot be used multiple times",
        ),
        (
            vec!["-c", "--compute"],
            "error: the argument '--compute' cannot be used multiple times",
        ),
        (
            vec!["-u", "a", "-u", "b"],
            "error: the argument '--user <USER>' cannot be used multiple times",
        ),
        (
            vec!["-u", "a", "--user=b"],
            "error: the argument '--user <USER>' cannot be used multiple times",
        ),
        (
            vec!["--range=a", "--range=b"],
            "error: the argument '--range <RANGE>' cannot be used multiple times",
        ),
    ] {
        let r = run(&args);
        assert_eq!(r.code, 125, "{:?}", args);
        assert!(r.err().starts_with(expect), "{:?} {}", args, r.err());
        assert!(r.err().contains("\n\nFor more information, try '--help'.\n"));
    }

    // 未知旗標：tip 用「顯示用的那個 token」
    for (args, shown, tip) in [
        (vec!["--bogus"], "--bogus", "--bogus"),
        (vec!["-Z"], "-Z", "-Z"),
        (vec!["-cfoo"], "-f", "-f"),
        (vec!["-c0"], "0", "-0"),
        (vec!["-c=1"], "=", "="),
        (vec!["-x"], "-x", "-x"),
    ] {
        let r = run(&args);
        assert_eq!(r.code, 125, "{:?}", args);
        assert!(
            r.err()
                .starts_with(&format!("error: unexpected argument '{}' found", shown)),
            "{:?} {}",
            args,
            r.err()
        );
        assert!(
            r.err()
                .contains(&format!("  tip: to pass '{}' as a value, use '-- {}'", tip, tip)),
            "{:?} {}",
            args,
            r.err()
        );
        assert!(r.err().contains(&format!("Usage: {}\n", USAGE)), "{:?}", args);
        assert!(r.err().contains("For more information, try '--help'."));
    }

    // 前綴推斷
    assert_eq!(run(&["--comp"]).code, 0);
    assert_eq!(run(&["--us", "u"]).code, 0);
    assert_eq!(run(&["--ver"]).out(), "runcon (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["--h"]).code, 0);
    // 不存在的長旗標（--exponents 之類）→ unexpected
    let r = run(&["--user=x", "--bog"]);
    assert_eq!(r.code, 125);
    assert!(r.err().contains("error: unexpected argument '--bog' found"));
}

#[test]
fn attached_values_and_clusters_are_accepted() {
    // 附值與簇：這些都只影響 context（無 SELinux 時不生效），但必须被正確解析
    for case in [
        vec!["--user=u", "s0", "cmd"],
        vec!["--role=r", "s0", "cmd"],
        vec!["--type=t", "s0", "cmd"],
        vec!["--range=s0", "s0", "cmd"],
        vec!["-uuser", "s0", "cmd"],
        vec!["-rl", "s0", "cmd"],
        vec!["-cul", "s0", "cmd"],
        vec!["-ur", "s0", "cmd"],
        vec!["-l", "s0", "-u", "u", "-r", "r", "-t", "ty", "-c", "cmd"],
    ] {
        let r = run(&case);
        if PathBuf::from("/sys/fs/selinux").exists() {
            continue;
        }
        assert_eq!(r.code, 1, "{:?}", case);
        assert_eq!(
            r.err(),
            "runcon: runcon may be used only on a SELinux kernel\n",
            "{:?}",
            case
        );
    }
}

#[test]
fn flag_with_attached_value_on_boolean_flags_is_rejected() {
    for args in [vec!["--compute=1"], vec!["--help=1"], vec!["--version=1"]] {
        let r = run(&args);
        assert_eq!(r.code, 125, "{:?}", args);
        assert_eq!(r.out(), "");
    }
    let r = run(&["--compute=1"]);
    assert!(r.err().starts_with("error: unexpected value '1' for '--compute' found"), "{}", r.err());
}

#[test]
fn via_binary_matches_in_process() {
    let r = t::run_bin("runcon", &["--version"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "runcon (uutils coreutils) 0.10.0\n");

    let b = t::run_bin("runcon", &["s0"], "");
    assert_eq!(b.code, 1);
    assert_eq!(b.err(), "runcon: No command is specified\n");

    let c = t::run_bin("runcon", &["s0", "true"], "");
    if !PathBuf::from("/sys/fs/selinux").exists() {
        assert_eq!(c.code, 1);
        assert_eq!(c.err(), "runcon: runcon may be used only on a SELinux kernel\n");
    }

    let h = t::run_bin("runcon", &["-c", "-c"], "");
    assert_eq!(h.code, 125);
    assert!(h.err().starts_with("error: the argument '--compute' cannot be used multiple times"));
}

#[test]
fn long_prefix_resolution_rules() {
    // 唯一前綴可解析（值接在後面即被該選項吃掉）
    for args in [
        vec!["--use=", "u"], // 空值 + operand：有 COMMAND → 一律 SELinux 錯誤（rc 1，指令存在與否不影響）
        vec!["--rol", "r"],
        vec!["--typ", "t"],
        vec!["--rang", "s0"],
        vec!["--comp"],
        vec!["--ver"],
        vec!["--hel"],
    ] {
        let r = run(&args);
        if args[0] == "--use=" {
            assert_eq!(r.code, 1, "{:?}", args); // 無 SELinux → 一律 1
            continue;
        }
        assert_eq!(r.code, 0, "{:?}", args);
        assert_eq!(r.err(), "", "{:?}", args);
    }
    // 唯一前綴但需要值：裸 --use → clap 缺值 125
    let r = run(&["--use"]);
    assert_eq!(r.code, 125);

    // `--r` 同時是 range/role 的前缀 → similar tip；`--l` 沒有候選 → unexpected argument
    // 無候選 → unexpected argument tip
    for raw in ["--l", "--b", "--usr", "--verion"] {
        let r = run(&[raw]);
        assert_eq!(r.code, 125, "{raw}");
        assert_eq!(r.out(), "", "{raw}");
        assert!(
            r.err()
                .starts_with(&format!("error: unexpected argument '{raw}' found\n\n")),
            "{raw}: {:?}",
            r.err()
        );
        assert!(r.err().contains(&format!("'-- {raw}'")), "{raw}: {:?}", r.err());
        assert!(r.err().contains("Usage: runcon CONTEXT COMMAND"), "{raw}");
    }
    // 唯一前綴（--ro→role、--typ→type）：解析成功但缺值 → clap 缺值 125
    for (raw, opt) in [("--ro", "'--role <ROLE>'"), ("--typ", "'--type <TYPE>'")] {
        let r = run(&[raw]);
        assert_eq!(r.code, 125, "{raw}: {:?}", r.err());
        assert!(
            r.err()
                .starts_with(&format!("error: a value is required for {opt}")),
            "{raw}: {:?}",
            r.err()
        );
    }
    // 歧義前綴：我方 resolve_long 取第一個候選當 similar tip
    let r = run(&["--r"]);
    assert_eq!(r.code, 125);
    assert!(
        r.err().contains("tip: a similar argument exists: '--range'"),
        "{:?}",
        r.err()
    );
}

#[test]
fn boolean_flags_reject_attached_values() {
    let cases: [(&str, &str, &str); 4] = [
        ("--c=x", "compute", "x"),
        ("--compute=", "compute", ""),
        ("--help=1", "help", "1"),
        ("--version=1", "version", "1"),
    ];
    for (arg, long, value) in cases {
        let r = run(&[arg]);
        assert_eq!(r.code, 125, "{arg}");
        assert_eq!(
            r.err(),
            format!(
                "error: unexpected value '{value}' for '--{long}' found; no more were expected\n\nUsage: {USAGE}\n\nFor more information, try '--help'.\n"
            ),
            "{arg}"
        );
    }
}

#[test]
fn help_and_version_ignore_trailing_operands() {
    for args in [
        vec!["--help", "s0"],
        vec!["--help", "-c"],
        vec!["--version", "s0"],
        vec!["--version", "s0", "true"],
        vec!["--ver", "x"],
        vec!["--v", "x"],
        vec!["--h", "x"],
    ] {
        let r = run(&args);
        assert_eq!(r.code, 0, "{:?}", args);
        assert_eq!(r.err(), "", "{:?}", args);
    }
    assert_eq!(run(&["--version", "x"]).out(), "runcon (uutils coreutils) 0.10.0\n");
    assert!(run(&["--help", "x"]).out().starts_with("Run command with"));
}

#[test]
fn empty_attached_values_are_accepted() {
    for args in [
        vec!["--user="],
        vec!["--range="],
        vec!["--role="],
        vec!["--type="],
        vec!["-u", ""],
    ] {
        let r = run(&args);
        assert_eq!(r.code, 0, "{:?}", args);
        assert_eq!(r.err(), "", "{:?}", args);
    }
    // 空附值 + operand：operand 是 COMMAND → SELinux 拒绝路徑
    if !PathBuf::from("/sys/fs/selinux").exists() {
        for args in [vec!["--use=", "u"], vec!["--user=", "s0"], vec!["--range=", "s0"]] {
            let r = run(&args);
            assert_eq!(r.code, 1, "{:?}", args);
            assert_eq!(r.err(), "runcon: runcon may be used only on a SELinux kernel\n", "{:?}", args);
        }
    }
}

#[test]
fn fuzz_invariants() {
    // 不變量：任意組合 rc ∈ {0,1,125,2}（無 SELinux 環境下）；
    // rc==0 → err 空、stdout 恰一行（目前 context）；
    // rc==1 → 必然是 SELinux 拒絕訊息；clap 錯誤 stderr 以 "error: " 起頭。
    let pool = [
        "--user=u", "--role=", "--type", "t", "--range=s0", "-c", "ctx", "ls", "--",
        "--hel", "--ver", "-h", "-V", "-u", "u2", "-l", "s0-s0",
    ];
    let selinux_present = PathBuf::from("/sys/fs/selinux").exists();
    let mut st: u64 = 0xBEEF01;
    for _ in 0..90 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st >> 50) % 4 + 1;
        let mut args: Vec<&str> = Vec::new();
        for k in 0..n {
            args.push(pool[((st >> (k * 5)) % pool.len() as u64) as usize]);
        }
        let r = run(&args);
        let ok_codes: &[i32] = if selinux_present { &[0, 1, 2, 125, 126, 127] } else { &[0, 1, 2, 125] };
        assert!(ok_codes.contains(&r.code), "{:?} -> {}", args, r.code);
        // 解析出「旗標數」與「operand 數」：無旗標時第一 operand 是 CONTEXT、
        // 要有第二個才是 COMMAND；有旗標時第一 operand 就是 COMMAND。
        let (flag_count, operands) = {
            let mut flags = 0usize;
            let mut ops: Vec<&str> = Vec::new();
            let mut after_dd = false;
            let mut pending = 0usize;
            for a in &args {
                if after_dd { ops.push(a); continue; }
                if *a == "--" { after_dd = true; continue; }
                if a.starts_with("--") {
                    if a.contains('=') { flags += 1; } else { flags += 1; pending += 1; }
                } else if a.starts_with('-') && a.len() > 1 {
                    flags += 1; pending += 1;
                } else if pending > 0 {
                    pending -= 1;
                } else {
                    ops.push(a);
                }
            }
            (flags, ops)
        };
        let has_command = if flag_count > 0 { !operands.is_empty() } else { operands.len() >= 2 };
        let context_only = flag_count == 0 && operands.len() == 1;
        match r.code {
            0 => {
                assert_eq!(r.err(), "", "{:?}", args);
                assert!(!r.out().is_empty(), "{:?} out={:?}", args, r.out());
            }
            1 => {
                if !selinux_present {
                    if context_only {
                        assert_eq!(r.err(), "runcon: No command is specified\n", "{:?}", args);
                    } else if has_command {
                        assert_eq!(r.err(), "runcon: runcon may be used only on a SELinux kernel\n", "{:?}", args);
                    }
                }
            }
            2 | 125 => {
                assert!(r.err().starts_with("error: "), "{:?} err={:?}", args, r.err());
                assert_eq!(r.out(), "", "{:?}", args);
            }
            _ => {}
        }
    }
}
