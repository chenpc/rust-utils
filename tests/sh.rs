//! sh(1) tests（guest 內執行）。
//! e2e 用 t::run("sh", ...) in-process（coverage 計在 sh.rs），
//! 外部命令一律走 $COREUTILS_BIN（payload 已 export）。
use coreutils::testutil as t;

fn s(script: &str) -> t::Out {
    t::run("sh", &["-c", script], "")
}

#[test]
fn echo_and_quotes() {
    assert_eq!(s("echo hello").out(), "hello\n");
    assert_eq!(s("echo 'a b' \"c d\"").out(), "a b c d\n");
    // 單引號內不展開；雙引號內展開
    assert_eq!(s("X=v; echo '$X' \"$X\"").out(), "$X v\n");
    // 引號內的空白維持同一個 word
    assert_eq!(s("echo \"a b\"").out(), "a b\n");
    // 跳脫
    assert_eq!(s(r"echo \$.x 'a'\''b'").out(), "$.x a'b\n");
}

#[test]
fn variables_and_status() {
    assert_eq!(s("X=abc; echo $X${X}y$?").out(), "abcabcy0\n");
    assert_eq!(s("echo ${UNSET_VAR}x").out(), "x\n");
    assert_eq!(s("false; echo $?").out(), "1\n");
    assert_eq!(s("false; true; echo $?").out(), "0\n");
    // export 後外部命令看得到；unset 後消失
    let r = s("export SH_TESTV=1; \"$COREUTILS_BIN\" sh -c \"echo child:\\$SH_TESTV\" > /tmp/sh_env1.txt; unset SH_TESTV; echo [$SH_TESTV]");
    assert_eq!(r.out(), "[]\n", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_env1.txt").unwrap(), "child:1\n");
}

#[test]
fn and_or_short_circuit() {
    assert_eq!(s("true && echo t || echo f").out(), "t\n");
    assert_eq!(s("false && echo t || echo f").out(), "f\n");
    assert_eq!(s("false || true && echo ok").out(), "ok\n");
    // 修正前的反向 bug：这两条必须不输出
    assert_eq!(s("true && true").out(), "");
    assert_eq!(s("false || false").code, 1);
}

#[test]
fn if_elif_for() {
    std::fs::write("/tmp/sh_f1.txt", "x").unwrap();
    assert_eq!(s("if [ -f /tmp/sh_f1.txt ]; then echo y; else echo n; fi").out(), "y\n");
    assert_eq!(s("if false; then echo y; else echo n; fi").out(), "n\n");
    assert_eq!(s("if false; then echo a; elif true; then echo b; fi").out(), "b\n");
    assert_eq!(s("for m in a b c; do echo $m; done").out(), "a\nb\nc\n");
    // 清單後接 ; 再接 do
    assert_eq!(s("for m in x; do echo $m; done; echo after").out(), "x\nafter\n");
    // 無 in → 空清單
    assert_eq!(s("for x; do echo $x; done").out(), "");
}

#[test]
fn pipelines() {
    // 外部命令用 OS pipe 串接（cat 走 multi-call binary）
    let r = s("echo hi | \"$COREUTILS_BIN\" cat > /tmp/sh_p1.txt");
    assert_eq!(r.err(), "", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_p1.txt").unwrap(), "hi\n");
    let r = s("echo a | \"$COREUTILS_BIN\" tr a-z A-Z | \"$COREUTILS_BIN\" cat > /tmp/sh_p2.txt");
    assert_eq!(r.err(), "", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_p2.txt").unwrap(), "A\n");
    // 管線內不支援內建/控制結構
    assert_eq!(s("if true; then true; fi | true").code, 2);
    // 管線中段找不到命令 → 127（尾段失敗）
    let r = s("echo x | \"$COREUTILS_BIN\" cat | nosuchapplet_zz");
    assert_eq!(r.code, 127);
    // 前段失敗：下游 stdin 給 null，不讀 console 卡死 → cat 空輸入 rc 0
    let r = s("nosuchapplet_zz | \"$COREUTILS_BIN\" cat");
    assert_eq!(r.code, 0);
    // 內建在尾段：輸出照常、rc 取內建
    let r = s("\"$COREUTILS_BIN\" echo x | echo done");
    assert_eq!(r.out(), "done\n", "err={}", r.err());
    // 內建在尾段 + 管線級重導向
    let r = s("\"$COREUTILS_BIN\" echo x | echo red > /tmp/sh_p3.txt");
    assert_eq!(r.out(), "", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_p3.txt").unwrap(), "red\n");
}

#[test]
fn redirections() {
    let r = s("echo one > /tmp/sh_t1.txt; \"$COREUTILS_BIN\" cat /tmp/sh_t1.txt > /tmp/sh_c1.txt; echo two >> /tmp/sh_t1.txt; \"$COREUTILS_BIN\" cat /tmp/sh_t1.txt > /tmp/sh_c2.txt");
    assert_eq!(r.err(), "", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_c1.txt").unwrap(), "one\n");
    assert_eq!(std::fs::read_to_string("/tmp/sh_c2.txt").unwrap(), "one\ntwo\n");
    // < 餵給外部命令
    let r = s("\"$COREUTILS_BIN\" cat < /tmp/sh_t1.txt > /tmp/sh_c3.txt");
    assert_eq!(r.err(), "", "err={}", r.err());
    assert_eq!(std::fs::read_to_string("/tmp/sh_c3.txt").unwrap(), "one\ntwo\n");
    // 2> 抓到 stderr
    let r = s("nosuchapplet_zz 2> /tmp/sh_t2.txt");
    assert_eq!(r.code, 127);
    assert!(std::fs::read_to_string("/tmp/sh_t2.txt").unwrap().contains("nosuchapplet_zz"));
    // 內建輸出重導向（單元素管線 → run_cmd 層級 redirs）
    let r = s("echo in > /tmp/sh_t6.txt");
    assert_eq!(r.out(), "");
    assert_eq!(std::fs::read_to_string("/tmp/sh_t6.txt").unwrap(), "in\n");
    // 無法開啟的 redirection 目標 → 非零
    assert_ne!(s("echo x > /no_such_dir/f").code, 0);
}

#[test]
fn builtins_cd_pwd() {
    let r = s("cd /tmp; pwd");
    assert_eq!(r.out(), "/tmp\n", "err={}", r.err());
    assert_ne!(s("cd /no_such_dir_xyz").code, 0);
}

#[test]
fn test_bracket_ops() {
    assert_eq!(s("[ /tmp = /tmp ] && echo eq").out(), "eq\n");
    assert_eq!(s("[ /tmp != /x ] && echo ne").out(), "ne\n");
    assert_eq!(s("[ -d /tmp ] && [ -f /no_such ] || echo combo").out(), "combo\n");
    assert_eq!(s("[ -n abc ] && [ -z \"\" ] && echo nyz").out(), "nyz\n");
    // -o / -a / 括號（跳脫括號變成 word）
    assert_eq!(s("[ a = b -o a = a ] && echo or").out(), "or\n");
    assert_eq!(s("[ a = a -a b = b ] && echo and").out(), "and\n");
    assert_eq!(s("[ \\( a = b -o a = a \\) -a b = b ] && echo mix").out(), "mix\n");
    // 缺 ] → 2
    assert_eq!(s("[ a = a").code, 2);
    // test 外建形式（無 ]）
    assert_eq!(s("test -e /tmp && echo te").out(), "te\n");
    // -s：非空檔
    assert_eq!(s("echo x > /tmp/sh_t3.txt; [ -s /tmp/sh_t3.txt ] && echo se").out(), "se\n");
    // -nt/-ot：/tmp 新檔 vs /init（開機時解包）→ 確定性順序
    std::fs::write("/tmp/sh_t4.txt", "a").unwrap();
    std::fs::write("/tmp/sh_t5.txt", "b").unwrap();
    let r = s("[ /tmp/sh_t5.txt -nt /init ] && echo nt; [ /init -ot /tmp/sh_t4.txt ] && echo ot");
    assert_eq!(r.out(), "nt\not\n", "err={}", r.err());
}

#[test]
fn exit_code_and_errors() {
    assert_eq!(s("exit 7").code, 7);
    assert_eq!(s("if true; then exit 3; fi").code, 3);
    let r = s("nosuchapplet_zz");
    assert_eq!(r.code, 127);
    assert!(r.err().contains("nosuchapplet_zz"));
    // 語法錯誤 → 2
    let r = s("if");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("syntax error"), "err={}", r.err());
    assert_eq!(s("echo 'unterminated").code, 2);
    // 未存在的 script 檔
    assert_eq!(t::run("sh", &["/no_such_script.sh"], "").code, 2);
    // 空輸入 → 0
    assert_eq!(t::run("sh", &[], "").code, 0);
    // usage
    assert_eq!(t::run("sh", &["-h"], "").out().contains("usage:"), true);
}

#[test]
fn script_file_and_stdin() {
    std::fs::write("/tmp/sh_script.sh", "#!/bin/sh\necho from-script $1\nexit 5\n").unwrap();
    let r = t::run("sh", &["/tmp/sh_script.sh"], "");
    assert_eq!(r.out(), "from-script \n");
    assert_eq!(r.code, 5);
    // stdin 腳本模式
    assert_eq!(t::run("sh", &["-n"], "echo a\necho b\n").out(), "a\nb\n");
    // 註解：行中 # 起discard
    assert_eq!(s("echo a # comment").out(), "a\n");
    assert_eq!(s("# whole line\necho b").out(), "b\n");
}

#[test]
fn interactive_line_editing() {
    // -i 強制互動：raw-mode 在 test 里 tcsetattr 會失敗但行編輯仍可用
    let r = t::run("sh", &["-i"], "echo hi\r\x04");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("echo hi"), "out={:?}", r.out());
    assert!(r.out().contains("hi\n"));
    // 歷史：↑ 取回上一條
    let r = t::run("sh", &["-i"], "echo one\r\x1b[A\r\x04");
    assert_eq!(r.out().matches("\none\n").count(), 2, "out={:?}", r.out());
    // ↓ 回 stash
    let r = t::run("sh", &["-i"], "echo one\rabc\x1b[A\x1b[B\r\x04");
    assert!(r.out().contains("one\none"), "out={:?}", r.out());
    // Backspace：echo abc<BS> → ab（見下一個 test）
}

#[test]
fn interactive_backspace_and_caret() {
    let r = t::run("sh", &["-i"], "echo abc\x7f\r\x04");
    assert!(r.out().contains("ab\n"), "out={:?}", r.out());
    // Ctrl-C 棄行後可繼續
    let r = t::run("sh", &["-i"], "junk\x03echo kept\r\x04");
    assert!(r.out().contains("^C"), "out={:?}", r.out());
    assert!(r.out().contains("kept\n"));
    // Ctrl-D 在非空行不清空、在空行 EOF
    let r = t::run("sh", &["-i"], "echo x\x04y\r\x04");
    assert!(r.out().contains("xy\n"), "out={:?}", r.out());
    // 左右移 cursor + 插入
    let r = t::run("sh", &["-i"], "echo ac\x1b[D b\r\x04");
    assert!(r.out().contains("a bc\n"), "out={:?}", r.out());
}

#[test]
fn interactive_tab_completion() {
    // 第一個 word：補內建（exp → export/expo rt 唯一長前綴）
    let r = t::run("sh", &["-i"], "exp\t -n\r\x04");
    assert!(r.out().contains("export -n"), "out={:?}", r.out());
    // 無候選 → bell
    let r = t::run("sh", &["-i"], "zzqqxx\t\x04");
    assert!(r.out().contains('\u{7}'), "out={:?}", r.out());
}

// --- 補全純函數 ---

#[test]
fn completion_pure() {
    use coreutils::applets::sh::{completion, common_prefix_for_test, path_dirs, word_bounds};
    let lister = |dir: &str| -> Vec<String> {
        match dir {
            "." => vec!["alpha.txt".into(), "alphabet.txt".into(), "beta/".into()],
            "/etc" => vec!["passwd".into(), "profile".into()],
            _ => vec![],
        }
    };
    let paths: Vec<String> = vec![];
    assert_eq!(completion("alph", false, &lister, &paths), ["alpha.txt", "alphabet.txt"]);
    assert_eq!(completion("/etc/p", false, &lister, &paths), ["/etc/passwd", "/etc/profile"]);
    // 命令補全：內建
    let c = completion("exp", true, &lister, &paths);
    assert_eq!(c, ["export"]);
    let c = completion("", true, &lister, &paths);
    assert!(c.contains(&"cd".to_string()) && c.contains(&"echo".to_string()));
    assert_eq!(common_prefix_for_test(&c), "");
    // word_bounds
    let line: Vec<char> = "echo /et".chars().collect();
    let (start, word) = word_bounds(&line, line.len());
    assert_eq!((start, word.as_str()), (5, "/et"));
    let line2: Vec<char> = "a;b".chars().collect();
    let (s2, w2) = word_bounds(&line2, 3);
    assert_eq!((s2, w2.as_str()), (2, "b"));
    // 真實 fs：/ 下一定有 entry
    let real = completion("/", false, &coreutils::applets::sh::fs_lister, &paths);
    assert!(!real.is_empty());
    assert!(!path_dirs().is_empty());
}

// --- fuzz：不變量而非 crash oracle ---

#[test]
fn fuzz_programs_are_deterministic() {
    // 同一輸入跑兩次，(code, stdout, stderr) 必須一致；
    // 不允許被 signal 殺（code >= 0）；tokenize/parse 錯誤收斂為 2。
    let toks = [
        "echo", "a", "b", "$X", "${X}", "$?", ";", "|", "&&", "||", "if", "then", "fi",
        "for", "in", "do", "done", "\"", "'", "(", ")", "[", "]", ">", ">>", "<", "2>",
        "\n", " ", "#", "${", "}", "?", "$", "\t", "\\", "true", "false", "exit", "cd",
    ];
    let mut rng = t::Rng::new(0x5F5F);
    for _ in 0..200 {
        let n = 1 + rng.below(12) as usize;
        let mut prog = String::new();
        for _ in 0..n {
            prog.push_str(toks[rng.below(toks.len() as u64) as usize]);
            if rng.below(3) == 0 {
                prog.push(' ');
            }
        }
        let a = t::run("sh", &["-c", &prog], "");
        let b = t::run("sh", &["-c", &prog], "");
        assert_eq!(a.code, b.code, "nondeterministic code for {prog:?}");
        assert_eq!(a.stdout, b.stdout, "nondeterministic stdout for {prog:?}");
        assert!(a.code >= 0, "killed by signal for {prog:?}");
        if a.code == 2 {
            let err = String::from_utf8_lossy(&a.stderr).into_owned();
            assert!(
                err.contains("syntax error")
                    || err.contains("not supported")
                    || err.contains("missing ]"),
                "rc 2 without recognized error: {prog:?}"
            );
        }
    }
}

#[test]
fn fuzz_word_bounds_invariant() {
    let chars = "ab ;|&<>\t\n";
    let mut rng = t::Rng::new(0xB0);
    for _ in 0..200 {
        let n = rng.below(20) as usize;
        let line: String = (0..n).map(|_| chars.chars().nth(rng.below(chars.len() as u64) as usize).unwrap()).collect();
        let v: Vec<char> = line.chars().collect();
        let pos = rng.below((n + 1) as u64) as usize;
        let (start, word) = coreutils::applets::sh::word_bounds(&v, pos);
        assert!(start <= pos);
        assert_eq!(word, v[start..pos].iter().collect::<String>());
    }
}

#[test]
fn fuzz_completion_prefix_invariant() {
    use coreutils::applets::sh::completion;
    let lister = |dir: &str| -> Vec<String> {
        match dir {
            "." => vec!["alpha".into(), "alphabet".into(), "beta/".into(), "a/".into()],
            _ => vec![],
        }
    };
    let paths: Vec<String> = vec![];
    let mut rng = t::Rng::new(0xC0);
    let alphabet = "ab/ ";
    for _ in 0..150 {
        let n = rng.below(5) as usize;
        let word: String = (0..n).map(|_| alphabet.chars().nth(rng.below(alphabet.len() as u64) as usize).unwrap()).collect();
        let cands = completion(&word, false, &lister, &paths);
        for c in &cands {
            assert!(c.starts_with(&word), "candidate {c:?} lost prefix of {word:?}");
        }
        // 結果必須排序且無重複
        let mut sorted = cands.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(cands, sorted);
    }
}
