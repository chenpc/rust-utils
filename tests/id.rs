//! id(1) 的端到端測試（在 qemu guest 內執行，guest 預設為 uid 0 且沒有 /etc/passwd）。
//!
//! 斷言依據：docs/man/id.1.gz 的描述＋本機參考實測。
//! 兩條主路徑都測到：(a) 無 passwd/group db（名稱解析失敗）
//!                   (b) 自建 /etc/passwd＋/etc/group（名稱解析成功）。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("id", args, "")
}

fn clear_db() {
    let _ = std::fs::remove_file("/etc/passwd");
    let _ = std::fs::remove_file("/etc/group");
}

fn set_db(passwd: &str, group: &str) {
    clear_db();
    // guest 的 /etc 由 init 建立；測試自行確保目錄存在
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", passwd).unwrap();
    std::fs::write("/etc/group", group).unwrap();
}

/// 目前程序（guest 內為 root）的名稱解析失敗時，預設格式印數字並每筆印 warning。
#[test]
fn default_format_without_database() {
    clear_db();
    let r = run(&[]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "uid=0(0) gid=0(0) groups=0(0)\n");
    assert_eq!(
        r.err(),
        "id: cannot find name for user ID 0\nid: cannot find name for group ID 0\nid: cannot find name for group ID 0\n"
    );
}

#[test]
fn mode_flags_without_database() {
    clear_db();
    let r = run(&["-u"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));
    assert_eq!(r.err(), "");

    let r = run(&["-g"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));

    let r = run(&["-G"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));
    assert_eq!(r.err(), "");

    // -n：名稱找不到 → 印數字、rc 1、每筆一個 warning（man：「If any of the ID
    // numbers cannot be mapped into names, the number will be displayed as usual」）
    let r = run(&["-un"]);
    assert_eq!((r.code, r.out().as_str()), (1, "0\n"));
    assert_eq!(r.err(), "id: cannot find name for user ID 0\n");

    let r = run(&["-gn"]);
    assert_eq!((r.code, r.out().as_str()), (1, "0\n"));
    assert_eq!(r.err(), "id: cannot find name for group ID 0\n");

    let r = run(&["-Gn"]);
    assert_eq!((r.code, r.out().as_str()), (1, "0\n"));
    assert_eq!(r.err(), "id: cannot find name for group ID 0\n");

    // -z：NUL 結尾而非換行
    let r = run(&["-u", "-z"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out_bytes(), b"0\x00");
}

#[test]
fn names_and_numbers_with_database() {
    set_db(
        "root:x:0:0:rootgecos:/root:/bin/sh\nother:x:7:7:Other:/home/other:/bin/sh\n",
        "root:x:0:\nsupp:x:7:other\n",
    );
    let r = run(&["-un"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    assert_eq!(r.err(), "");

    let r = run(&["-gn"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));

    let r = run(&["-Gn"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));

    let r = run(&[]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "uid=0(root) gid=0(root) groups=0(root)\n");
    assert_eq!(r.err(), "");

    // 數字 operand 走 passwd 查找
    let r = run(&["other"]);
    assert_eq!((r.code, r.out().as_str()), (0, "uid=7(other) gid=7(supp) groups=7(supp)\n"));
    let r = run(&["7"]);
    assert_eq!(r.out(), "uid=7(other) gid=7(supp) groups=7(supp)\n");

    clear_db();
}

#[test]
fn supplementary_group_order_keeps_gid_first() {
    set_db(
        "u1:x:1000:100:U1:/home/u1:/bin/sh\n",
        "gb:x:200:u1\nga:x:100:u1\ngc:x:300:u1\n",
    );
    // 指定使用者：gid 先，其後依 /etc/group 檔案順序（實測）
    let r = run(&["-G", "u1"]);
    assert_eq!((r.code, r.out().as_str()), (0, "100 200 300\n"));
    let r = run(&["-Gn", "u1"]);
    assert_eq!((r.code, r.out().as_str()), (0, "ga gb gc\n"));
    clear_db();
}

#[test]
fn unknown_operand_is_reported() {
    clear_db();
    let r = run(&["nosuchuser"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: 'nosuchuser': no such user\n");
    assert_eq!(r.out(), "");

    let r = run(&["root", "nosuch"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("id: 'nosuch': no such user"));

    // 空 operand：參考用單引號包裝
    let r = run(&[""]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: '': no such user\n");

    // operand 內含單引號時改用雙引號（實測）
    let r = run(&["a'b"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: \"a'b\": no such user\n");

    // `--` 之後的 token 是 operand，不是選項
    let r = run(&["--", "-x"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: '-x': no such user\n");
}

#[test]
fn runtime_errors_for_z_n_r_z() {
    let r = run(&["-z"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: option --zero not permitted in default format\n");
    assert_eq!(r.out(), "");

    let r = run(&["-n"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: printing only names or real IDs requires -u, -g, or -G\n");

    let r = run(&["-r"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "id: printing only names or real IDs requires -u, -g, or -G\n");

    // -Z：本專案未接 SELinux/SMACK（man：「print only the security context」）
    let r = run(&["-Z"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "id: --context (-Z) works only on an SELinux/SMACK-enabled kernel\n"
    );

    // -Z 與 -u/-g/-G 是 clap 層衝突（先於運行期檢查，實測）
    let r = run(&["-u", "-Z"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '--user' cannot be used with '--context'\n\nFor more information, try '--help'.\n"
    );
    // -Z 與 -p/-P 不衝突 → 運行期錯誤
    let r = run(&["-p", "-Z"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "id: --context (-Z) works only on an SELinux/SMACK-enabled kernel\n"
    );
}

#[test]
fn clap_style_errors() {
    let r = run(&["-x"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\n  tip: to pass '-x' as a value, use '-- -x'\n\nUsage: id [OPTION]... [USER]...\n\nFor more information, try '--help'.\n"
    );

    // 选项衝突：措辭 `the argument '<A>' cannot be used with '<B>'`（A 為先出現者）
    let r = run(&["-u", "-g"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '--user' cannot be used with '--group'\n\nFor more information, try '--help'.\n"
    );

    let r = run(&["-gu"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '--group' cannot be used with '--user'\n\nFor more information, try '--help'.\n"
    );

    let r = run(&["-A", "-u"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '-A' cannot be used with '--user'\n\nFor more information, try '--help'.\n"
    );

    let r = run(&["-p", "-P"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '-p' cannot be used with '-P'\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn ignore_flags_compatibility() {
    // -a 為兼容性 no-op（man：「ignore, for compatibility with other versions」）
    clear_db();
    let r = run(&["-a"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "uid=0(0) gid=0(0) groups=0(0)\n");

    // -A 在 Linux 不可用（man 的 -A 說明）→ 完全無輸出，也不解析 operand（實測）
    let r = run(&["-A"]);
    assert_eq!((r.code, r.out().as_str()), (0, ""));
    assert_eq!(r.err(), "");
    let r = run(&["-A", "nosuchuser"]);
    assert_eq!((r.code, r.out().as_str(), r.err().as_str()), (0, "", ""));
}

#[test]
fn human_and_passwd_formats() {
    // -p：無 db 時沒有 login 列，uid/groups 印數字（實測）
    clear_db();
    let r = run(&["-p"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "uid\t0\ngroups\t0\n");
    assert_eq!(r.err(), "");

    set_db(
        "root:x:0:0:rootgecos:/root:/bin/sh\n",
        "root:x:0:\nextra:x:5:\n",
    );
    let r = run(&["-p"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "login\troot\nuid\troot\ngroups\troot\n");

    // -p 指定使用者：參考怪癖 —— `groups<TAB>` 列是空的，群組清單另起一列
    let r = run(&["-p", "root"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "uid\troot\ngroups\t\nroot\n");

    // -P：passwd 条目格式
    let r = run(&["-P"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root:x:0:0:rootgecos:/root:/bin/sh\n"));
    let r = run(&["-P", "root"]);
    assert_eq!(r.out(), "root:x:0:0:rootgecos:/root:/bin/sh\n");
    // -P 對多個 operand 只處理第一個（實測）
    let r = run(&["-P", "root", "root"]);
    assert_eq!(r.out(), "root:x:0:0:rootgecos:/root:/bin/sh\n");
    // -u 不蓋掉 -P
    let r = run(&["-P", "-u"]);
    assert_eq!(r.out(), "root:x:0:0:rootgecos:/root:/bin/sh\n");

    // 無 db 時 -P 的措辭
    clear_db();
    let r = run(&["-P"]);
    assert_eq!((r.code, r.err().as_str()), (1, "id: No such id: 0\n"));
    assert_eq!(r.out(), "");
}

#[test]
fn long_option_prefix_inference_and_help_version() {
    let r = run(&["--user"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));
    let r = run(&["--use"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));
    let r = run(&["--real", "--user"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));

    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Print user and group information for each specified USER,"));
    assert!(r.out().contains("  -a, --ignore   ignore, for compatibility with other versions"));
    assert!(r.out().contains("  -z, --zero     delimit entries with NUL characters, not whitespace;"));

    let r = run(&["--version"]);
    assert_eq!((r.code, r.out().as_str()), (0, "id (uutils coreutils) 0.10.0\n"));

    let r = run(&["-hV"]);
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Print user and group information"));
    assert_eq!(t::run("id", &["-V", "-h"], "").out(), "id (uutils coreutils) 0.10.0\n");
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("id", &["-u"], "");
    assert_eq!((r.code, r.out().as_str()), (0, "0\n"));
    assert_eq!(t::run_bin("id", &["--version"], "").out(), "id (uutils coreutils) 0.10.0\n");
}

#[test]
fn fuzz_flag_combinations_are_stable() {
    // 亂序旗標：只要不觸及 clap 衝突/運行期錯誤，輸出必須可預測且兩次一致
    clear_db();
    let mut rng = t::Rng::new(20261126);
    let flags = ["-u", "-g", "-G", "-n", "-z", "-a", "-p", "-P", "--", "-un", "-Gz"];
    for _ in 0..24 {
        let k = 1usize + rng.below(2) as usize;
        let mut args: Vec<String> = Vec::new();
        for _ in 0..k {
            args.push(flags[rng.below(flags.len() as u64) as usize].to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let a = run(&refs);
        let b = run(&refs);
        assert_eq!((a.code, a.out()), (b.code, b.out()), "args {:?}", refs);
        // 不崩潰：rc 必須在 0/1
        assert!(a.code == 0 || a.code == 1, "args {:?} rc {}", refs, a.code);
        // -u/-g/-G（不含 -n）的輸出必须是純數字或空
        if refs.iter().any(|s| s.contains('u') || s.contains('g') || s.contains('G'))
            && !refs.iter().any(|s| s.contains('n'))
            && !refs.iter().any(|s| s.contains('p') || s.contains('P'))
        {
            for line in a
                .out()
                .split(|c: char| c.is_whitespace() || c == '\0')
                .filter(|s| !s.is_empty())
            {
                assert!(line.chars().all(|c| c.is_ascii_digit()), "args {:?} out {:?}", refs, a.out());
            }
        }
    }
}
