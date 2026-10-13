//! stty(1) 測試（guest 內執行）。
//!
//! 一律用 `-F /dev/ttyS0`（guest 的序列主控台，devtmpfs 提供），不依賴 test binary 自己的
//! stdin 是否 tty。我方輸出格式決定（見 docs/assumptions.md「stty」節）：
//! - 無 settings：`speed <ospeed> baud;`
//! - -a：speed 行 + 旗標行（開=名、關=-名）+ cchars 行
//! - -g：單行 stty 可讀形式，且可被本實作讀回（round-trip 不變量）

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    let mut v: Vec<String> = vec!["-F".to_string(), "/dev/ttyS0".to_string()];
    v.extend(args.iter().map(|s| s.to_string()));
    t::run("stty", &v.iter().map(|s| s.as_str()).collect::<Vec<_>>(), "")
}

fn run_plain(args: &[&str]) -> t::Out {
    t::run("stty", args, "")
}

fn save_line() -> String {
    let r = run(&["-g"]);
    assert_eq!(r.code, 0, "stty -g failed: {}", r.err());
    let out = r.out();
    assert!(out.ends_with('\n'));
    out.trim_end().to_string()
}

#[test]
fn help_and_version() {
    let h = run_plain(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().starts_with("Print or change terminal characteristics."));
    assert!(h.out().contains("-a, --all"));
    assert!(h.out().contains("-F, --file DEVICE"));
    assert_eq!(run_plain(&["-h"]).out(), h.out());
    assert_eq!(run_plain(&["--version"]).out(), "stty (uutils coreutils) 0.10.0\n");
    assert_eq!(run_plain(&["-V"]).out(), "stty (uutils coreutils) 0.10.0\n");
}

#[test]
fn bare_output_is_speed_summary() {
    let g = save_line();
    let ospeed = g
        .split(' ')
        .skip_while(|tk| *tk != "ospeed")
        .nth(1)
        .expect("ospeed in -g");
    let r = run(&[]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), format!("speed {} baud;\n", ospeed));
}

#[test]
fn save_and_all_formats() {
    let g = save_line();
    assert!(g.starts_with("ispeed "), "{g}");
    assert!(g.contains(" ospeed "), "{g}");
    assert!(g.contains("VINTR=0x"), "{g}");
    // 旗標成對出現（開或關）
    for name in ["echo", "icanon", "onlcr"] {
        let present = g.split(' ').any(|tk| tk == name || tk == format!("-{}", name));
        assert!(present, "flag {name} in {g}");
    }
    let a = run(&["-a"]);
    assert_eq!(a.code, 0);
    let aout = a.out();
    let lines: Vec<&str> = aout.lines().collect();
    assert_eq!(lines.len(), 3, "{:?}", a.out());
    assert!(lines[0].starts_with("speed ") && lines[0].contains("baud;"), "{}", lines[0]);
    assert!(lines[1].contains("icanon") || lines[1].contains("-icanon"), "{}", lines[1]);
    assert!(lines[2].starts_with("cchars: "), "{}", lines[2]);
}

#[test]
fn toggle_echo_roundtrip() {
    let original = save_line();
    // 把 echo 切到另一邊
    let cur_on = original.split(' ').any(|tk| tk == "echo");
    let toggle = if cur_on { "-echo" } else { "echo" };
    let r = run(&[toggle]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), ""); // 套用 settings 不印東西
    let after = save_line();
    assert_ne!(after, original);
    let flipped = after.split(' ').any(|tk| tk == toggle);
    assert!(flipped, "after={after}");
    // 讀回 -g 的形式再套用 → 與 original 相同（round-trip 不變量）
    let toks: Vec<&str> = original.split(' ').collect();
    let r = run(&toks);
    assert_eq!(r.code, 0, "reapply failed: {}\ntokens {:?}", r.err(), toks);
    assert_eq!(save_line(), original, "round-trip");
}

#[test]
fn raw_and_sane_presets() {
    let original = save_line();
    let toks: Vec<&str> = original.split(' ').collect();
    run(&["raw"]);
    let raw_state = save_line();
    assert!(raw_state.split(' ').any(|tk| tk == "-icanon"), "raw: {raw_state}");
    assert!(raw_state.split(' ').any(|tk| tk == "-echo"), "raw: {raw_state}");
    assert!(raw_state.split(' ').any(|tk| tk == "-isig"), "raw: {raw_state}");
    run(&toks);
    assert_eq!(save_line(), original, "restore after raw");

    run(&["sane"]);
    let sane = save_line();
    assert!(sane.split(' ').any(|tk| tk == "icanon"), "sane: {sane}");
    assert!(sane.split(' ').any(|tk| tk == "echo"), "sane: {sane}");
    assert!(sane.split(' ').any(|tk| tk == "VINTR=0x03"), "sane: {sane}");
    run(&toks);
    assert_eq!(save_line(), original, "restore after sane");
}

#[test]
fn speed_and_rows_cols_size() {
    let original = save_line();
    let toks: Vec<&str> = original.split(' ').collect();

    let r = run(&["speed", "9600"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let g = save_line();
    assert!(g.contains("ispeed 9600"), "{g}");
    assert!(g.contains("ospeed 9600"), "{g}");

    let size = run(&["size"]);
    assert_eq!(size.code, 0, "{}", size.err());
    let sout = size.out();
    let wh: Vec<&str> = sout.split_whitespace().collect();
    assert_eq!(wh.len(), 2, "{}", size.out());

    let r = run(&["rows", "40", "cols", "132"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let size2 = run(&["size"]);
    assert_eq!(size2.out(), "40 132\n", "rows/cols set");

    run(&toks);
    assert_eq!(save_line(), original, "restore after speed/rows");
}

#[test]
fn errors_and_clap_style() {
    // 未知設定
    let r = run(&["nosuchsetting"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "stty: unknown setting 'nosuchsetting'\n");

    // 缺 operand
    for arg in ["rows", "cols", "speed"] {
        let r = run(&[arg]);
        assert_eq!(r.code, 1, "{arg}");
        assert_eq!(r.err(), format!("stty: missing operand after '{arg}'\n"));
    }
    let r = run(&["rows", "xx"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "stty: invalid argument 'xx'\n");

    // -F 檔案不存在
    let r = t::run("stty", &["-F", "/tmp/definitely-not-a-tty"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "stty: /tmp/definitely-not-a-tty: No such file or directory\n");

    // -F 不是終端機
    std::fs::write("/tmp/stty_notty", "x").unwrap();
    let r = t::run("stty", &["-F", "/tmp/stty_notty", "-a"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("stty: /tmp/stty_notty: "), "{}", r.err());

    // clap 風格
    let r = run_plain(&["--bogus"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '--bogus' found\n\n"), "{}", r.err());
    assert!(r.err().contains("Usage: stty [OPTION]... [SETTING]..."));

    let r = run_plain(&["-x"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '-x' found\n\n"));

    let r = run_plain(&["--file"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: a value is required for '--file <DEVICE>' but none was supplied"), "{}", r.err());

    // -Finline 值（-F/dev/ttyS0）可用
    let r = t::run("stty", &["-F/dev/ttyS0", "-g"], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().contains("ispeed "), "{}", r.out());
}

#[test]
fn fuzz_invariants() {
    let original = save_line();
    let toks: Vec<&str> = original.split(' ').collect();

    // 不變量 1：任意旗標設定一次後再全部讀回套用，狀態與 original 一致
    // 不變量 2：任何 settings 組合不 panic（exit code ∈ {0,1,2}）
    let pool = [
        "echo", "-echo", "icanon", "-icanon", "isig", "-isig", "ixon", "-ixon", "opost", "-opost",
        "crtscts", "-crtscts", "raw", "cooked", "sane", "VINTR=0x01", "VMIN=4", "VTIME=1", "-a", "-g",
    ];
    let mut st: u64 = 0xC0FFEE;
    for iter in 0..40u64 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st >> 40) % 6 + 1;
        let mut args: Vec<&str> = Vec::new();
        for j in 0..n {
            args.push(pool[((st >> (j * 5)) % pool.len() as u64) as usize]);
        }
        let r = run(&args);
        assert!(r.code == 0 || r.code == 1 || r.code == 2, "iter {} {:?} -> {} {}", iter, args, r.code, r.err());
        // 不確定狀態一律還原
        let rr = run(&toks);
        assert_eq!(rr.code, 0, "restore args {:?}", toks);
    }
    assert_eq!(save_line(), original);

    // 不變量 3：-g 輸出的每个 token 都能再被解析（self round-trip）
    let g = save_line();
    let gtoks: Vec<&str> = g.split(' ').collect();
    let r = run(&gtoks);
    assert_eq!(r.code, 0, "{:?}", r.err());
    assert_eq!(save_line(), g);
}
