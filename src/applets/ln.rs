//! ln(1) — Make links between files.
//!
//! 措辭依參考（uutils 0.10.0）：
//! - hard 失敗："ln: failed to create hard link 'src1' => 'l2': Already exists"（'source' => 'linkpath'）
//! - symbolic 失敗："ln: failed to create symbolic link 'l3': Already exists"
//! - 單 operand → 'X' => './X'；`-t DIR` 每個 operand 鏈入 DIR
//! - `-f` 覆蓋；`-b`/-S 備份（預設後綴 `~`）；`-n` 不追隨既有 symlink 目錄；`-T` 不視末 operand 為目錄
//! - `-L/-P`：hard link 是否追隨 symlink source（預設 -P）
//! clap 缺 operand 的 usage 措辭不追隨（自訂 rc2）。

use crate::{In, Out};
use std::path::PathBuf;

struct Opts {
    symbolic: bool,
    force: bool,
    no_deref: bool,
    no_target_dir: bool,
    logical: bool,
    backup: bool,
    suffix: String,
    target_dir: Option<String>,
    verbose: bool,
    relative: bool,
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut o = Opts {
        symbolic: false,
        force: false,
        no_deref: false,
        no_target_dir: false,
        logical: false,
        backup: false,
        suffix: "~".to_string(),
        target_dir: None,
        verbose: false,
        relative: false,
    };
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("");
            match name {
                "symbolic" => o.symbolic = true,
                "force" => o.force = true,
                "interactive" => {}
                "no-dereference" => o.no_deref = true,
                "no-target-directory" => o.no_target_dir = true,
                "logical" => o.logical = true,
                "physical" => o.logical = false,
                "relative" => o.relative = true,
                "verbose" => o.verbose = true,
                "backup" => o.backup = true,
                "suffix" => {
                    if let Some(pos) = a.find('=') {
                        o.suffix = a[pos + 1..].to_string();
                    } else if i < args.len() {
                        o.suffix = args[i].clone();
                        i += 1;
                    } else {
                        writeln!(stderr, "ln: option requires an argument -- 'suffix'").ok();
                        return 2;
                    }
                }
                "target-directory" => {
                    if let Some(pos) = a.find('=') {
                        o.target_dir = Some(a[pos + 1..].to_string());
                    } else if i < args.len() {
                        o.target_dir = Some(args[i].clone());
                        i += 1;
                    } else {
                        writeln!(stderr, "ln: option requires an argument -- 't'").ok();
                        return 2;
                    }
                }
                "help" => {
                    writeln!(stdout, "Usage: ln [OPTION]... [-T] TARGET LINK_NAME\n  or:  ln [OPTION]... TARGET\n  or:  ln [OPTION]... TARGET... DIRECTORY").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "ln (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "ln: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0;
        while k < chars.len() {
            match chars[k] {
                's' => o.symbolic = true,
                'f' => o.force = true,
                'i' => {}
                'n' => o.no_deref = true,
                'L' => o.logical = true,
                'P' => o.logical = false,
                'r' => o.relative = true,
                'v' => o.verbose = true,
                'b' => o.backup = true,
                'T' => o.no_target_dir = true,
                't' | 'S' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let v = if !rest.is_empty() {
                        k = chars.len();
                        rest
                    } else if i < args.len() {
                        let v = args[i].clone();
                        i += 1;
                        v
                    } else {
                        writeln!(stderr, "ln: option requires an argument -- '{}'", chars[k]).ok();
                        return 2;
                    };
                    if chars[k] == 't' {
                        o.target_dir = Some(v);
                    } else {
                        o.suffix = v;
                    }
                    break;
                }
                'h' => {
                    writeln!(stdout, "Usage: ln [OPTION]... [-T] TARGET LINK_NAME").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "ln (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "ln: invalid option -- '{}'", c).ok();
                    return 2;
                }
            }
            k += 1;
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "ln: missing operand").ok();
        return 2;
    }
    if o.relative && !o.symbolic {
        // 參考 clap 要求 -r 需搭配 -s（措辭不追隨）
        writeln!(stderr, "ln: --relative requires --symbolic").ok();
        return 2;
    }

    // 決定 target directory 與 sources
    let sources: Vec<String>;
    let tdir: Option<String>;
    if let Some(td) = o.target_dir.clone() {
        tdir = Some(td);
        sources = operands.clone();
    } else if operands.len() == 1 {
        tdir = Some(".".to_string());
        sources = operands.clone();
    } else if !o.no_target_dir && is_dir_last(&operands[operands.len() - 1], o.logical, o.no_deref) {
        tdir = Some(operands[operands.len() - 1].clone());
        sources = operands[..operands.len() - 1].to_vec();
    } else if operands.len() == 2 {
        tdir = None;
        sources = vec![operands[0].clone()];
    } else {
        writeln!(stderr, "ln: target '{}': Not a directory", operands[operands.len() - 1]).ok();
        return 1;
    }

    let mut rc = 0;
    for src in &sources {
        let linkpath: PathBuf = if let Some(td) = &tdir {
            let base = base_name(src);
            PathBuf::from(td).join(base)
        } else {
            PathBuf::from(&operands[1])
        };
        if let Err(msg) = make_link(src, &linkpath, &o, stdout) {
            if o.symbolic {
                writeln!(stderr, "ln: failed to create symbolic link '{}': {}", linkpath.display(), msg).ok();
            } else {
                writeln!(stderr, "ln: failed to create hard link '{}' => '{}': {}", src, linkpath.display(), msg).ok();
            }
            rc = 1;
        }
    }
    rc
}

fn is_dir_last(p: &str, logical: bool, no_deref: bool) -> bool {
    // -n（no-dereference）時不追隨 symlink 判斷目錄
    let follow = logical && !no_deref;
    if follow {
        std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
    } else {
        std::fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false)
    }
}

fn base_name(p: &str) -> String {
    let t = p.trim_end_matches('/');
    if t.is_empty() {
        return "/".to_string();
    }
    match t.rsplit('/').next() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => t.to_string(),
    }
}

fn make_link(src: &str, linkpath: &std::path::Path, o: &Opts, stdout: Out) -> Result<(), String> {
    let target = if o.relative {
        // 詞法相對：target = relpath(src, linkpath.parent)
        let cwd = std::env::current_dir().unwrap_or_default();
        let cwd = cwd.to_string_lossy().to_string();
        let abs = |s: &str| -> String {
            let joined = if s.starts_with('/') { s.to_string() } else { format!("{cwd}/{s}") };
            crate::pathname::build_lexical_absolute(&crate::pathname::components(&joined))
        };
        let dir = linkpath.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_else(|| ".".to_string());
        crate::pathname::relative_from(&abs(&dir), &abs(src))
    } else {
        src.to_string()
    };
    // 既有 linkpath 處理
    if let Ok(md) = std::fs::symlink_metadata(linkpath) {
        let is_symlink_dir_dest = md.file_type().is_symlink() && !o.no_deref && !o.no_target_dir && std::fs::metadata(linkpath).map(|m| m.is_dir()).unwrap_or(false);
        if is_symlink_dir_dest {
            // ln -s src existing-symlink-to-dir → 鏈進目錄內
            let np = PathBuf::from(linkpath).join(base_name(src));
            return make_link_named(src, &np, &target, o, stdout);
        }
        if o.backup {
            let mut bk = linkpath.as_os_str().to_os_string();
            bk.push(&o.suffix);
            let _ = std::fs::rename(linkpath, &bk);
        }
        if o.force {
            if md.is_dir() {
                let _ = std::fs::remove_dir(linkpath);
            } else {
                let _ = std::fs::remove_file(linkpath);
            }
        }
    }
    make_link_named(src, linkpath, &target, o, stdout)
}

fn make_link_named(src: &str, linkpath: &std::path::Path, target: &str, o: &Opts, stdout: Out) -> Result<(), String> {
    // hard link：-L 時 source 若為 symlink 則追隨（用 realpath 後 hard link 解析目標）
    let r = if o.symbolic {
        std::os::unix::fs::symlink(target, linkpath)
    } else {
        let real_src = if o.logical {
            match std::fs::canonicalize(src) {
                Ok(r) => r,
                Err(e) => return Err(crate::pathname::io_reason_nosuffix(&e)),
            }
        } else {
            std::path::PathBuf::from(src)
        };
        std::fs::hard_link(&real_src, linkpath)
    };
    match r {
        Ok(()) => {
            if o.verbose {
                writeln!(stdout, "ln: linked '{}' => '{}'", src, linkpath.display()).ok();
            }
            Ok(())
        }
        Err(e) => Err(if e.raw_os_error() == Some(17) { "Already exists".to_string() } else { crate::pathname::io_reason_nosuffix(&e) }),
    }
}
