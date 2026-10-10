// rm — remove files or directories（man: GNU coreutils 9.7 版）
//
// 選項：-f/--force、-i、-I、--interactive[=WHEN]、--one-file-system、
// --no-preserve-root、--preserve-root[=all]、-r/-R/--recursive、-d/--dir、-v。
// 措辭與互動 rc 語意對齊本機參考 binary（man 未指定）：
//   -i 提示 "rm: remove file 'x'? "／"rm: remove directory 'x'? "，非 y/Y → 略過（rc 不影響）
//   -I：>3 operand 或 recursive → 提示一次（"remove N arguments? "／"remove all arguments recursively? "），否 → rc0 中止全部
//   '.'/'..' operand → "refusing to remove '.' or '..' directory: skipping '<arg>'" rc1
//   recursive 撞 '/'（preserve-root 預設）→ "it is dangerous..." 兩行 rc1

use crate::pathname::io_reason_nosuffix;
use std::fs;

use std::os::unix::fs::MetadataExt;

#[derive(PartialEq, Clone, Copy)]
enum When {
    Never,
    Once,
    Always,
}

struct Opts {
    force: bool,
    when: When,
    recursive: bool,
    dir: bool,
    verbose: bool,
    one_fs: bool,
    preserve_root: bool,
}

/// 錯誤種類：(是否為已自印訊息, 訊息)
type RmErr = (bool, String);

pub fn run(args: &[String], mut stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let _ = stdout;
    let mut o = Opts {
        force: false,
        when: When::Never,
        recursive: false,
        dir: false,
        verbose: false,
        one_fs: false,
        preserve_root: true,
    };
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        match a {
            "--" => files.extend(args[i..].iter().cloned()),
            "--help" => {
                writeln!(stdout, "usage: rm [-dfiIrRv] FILE...").ok();
                return 0;
            }
            "--version" => {
                writeln!(stdout, "rm (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            "--force" => o.force = true,
            s if s.starts_with("--interactive=") => {
                o.when = match &s["--interactive=".len()..] {
                    "never" => When::Never,
                    "once" => When::Once,
                    _ => When::Always,
                };
            }
            s if s.starts_with("--preserve-root=") => {
                o.preserve_root = &s["--preserve-root=".len()..] != "none";
            }
            "--recursive" | "--recurse" => o.recursive = true,
            "--dir" => o.dir = true,
            "--verbose" => o.verbose = true,
            "--one-file-system" => o.one_fs = true,
            "--no-preserve-root" => o.preserve_root = false,
            "--preserve-root" => {
                o.preserve_root = true;
                if args.get(i).map(|s| s.as_str()) == Some("all") {
                    i += 1;
                }
            }
            "--interactive" => {
                o.when = match args.get(i).map(|s| s.as_str()) {
                    Some("never") => {
                        i += 1;
                        When::Never
                    }
                    Some("once") => {
                        i += 1;
                        When::Once
                    }
                    Some("always") => {
                        i += 1;
                        When::Always
                    }
                    _ => When::Always,
                };
            }
            "-f" => o.force = true,
            "-i" => o.when = When::Always,
            "-I" => o.when = When::Once,
            "-r" | "-R" => o.recursive = true,
            "-d" => o.dir = true,
            "-v" => o.verbose = true,
            s if s.starts_with("--") => {
                writeln!(stderr, "rm: unrecognized option '{}'", s).ok();
                return 2;
            }
            s if s.len() > 1 && s.starts_with('-') => {
                for c in s[1..].chars() {
                    match c {
                        'f' => o.force = true,
                        'i' => o.when = When::Always,
                        'I' => o.when = When::Once,
                        'r' | 'R' => o.recursive = true,
                        'd' => o.dir = true,
                        'v' => o.verbose = true,
                        _ => {
                            writeln!(stderr, "rm: invalid option -- '{}'", c).ok();
                            return 2;
                        }
                    }
                }
            }
            _ => files.push(a.to_string()),
        }
    }
    if files.is_empty() {
        writeln!(stderr, "rm: missing operand").ok();
        return 2;
    }

    let mut rc = 0;
    if o.when == When::Once && (files.len() > 3 || o.recursive) {
        let prompt = if files.len() > 3 {
            format!("rm: remove {} arguments? ", files.len())
        } else {
            "rm: remove all arguments recursively? ".to_string()
        };
        if !confirm(&prompt, &mut stdin, stderr) {
            return 0;
        }
    }
    for f in &files {
        match remove_one(f, &o, &mut stdin, stderr) {
            Ok(()) => {}
            Err((self_printed, msg)) => {
                if self_printed {
                    rc = 1;
                } else if !(o.force && msg == "No such file or directory") {
                    writeln!(stderr, "rm: cannot remove '{}': {}", f, msg).ok();
                    rc = 1;
                }
            }
        }
    }
    rc
}

fn io_err(e: std::io::Error) -> RmErr {
    (false, io_reason_nosuffix(&e))
}

fn is_root_operand(p: &str) -> bool {
    p == "/" || p.trim_end_matches('/') == ""
}

fn basename_is_dotdot(p: &str) -> bool {
    let t = p.trim_end_matches('/');
    let b = t.rsplit('/').next().unwrap_or(t);
    b == "." || b == ".."
}

fn remove_one(
    path: &str,
    o: &Opts,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<(), RmErr> {
    if basename_is_dotdot(path) {
        writeln!(
            stderr,
            "rm: refusing to remove '.' or '..' directory: skipping '{}'",
            path
        )
        .ok();
        return Err((true, String::new()));
    }
    let md = match fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(e) => return Err(io_err(e)),
    };
    if md.is_dir() {
        if is_root_operand(path) {
            if o.recursive {
                if o.preserve_root {
                    writeln!(stderr, "rm: it is dangerous to operate recursively on '/'").ok();
                    writeln!(
                        stderr,
                        "rm: use --no-preserve-root to override this failsafe"
                    )
                    .ok();
                    return Err((true, String::new()));
                }
            } else {
                return Err((false, "Is a directory".to_string()));
            }
        }
        if !o.recursive && !o.dir {
            return Err((false, "Is a directory".to_string()));
        }
        if o.dir && !o.recursive {
            if fs::read_dir(path).map(|mut d| d.next().is_some()).unwrap_or(false) {
                return Err((false, "Directory not empty".to_string()));
            }
        }
        if o.when == When::Always
            && !confirm(&format!("rm: remove directory '{}'? ", path), stdin, stderr)
        {
            return Ok(());
        }
        let top_dev = if o.one_fs { Some(md.dev()) } else { None };
        if o.recursive {
            walk_remove(path, o, stdin, stderr, top_dev)?;
        }
        if let Err(e) = fs::remove_dir(path) {
            if e.raw_os_error() == Some(39) {
                return Err((false, "Directory not empty".to_string()));
            }
            return Err(io_err(e));
        }
        if o.verbose {
            writeln!(stderr, "removed directory '{}'", path).ok();
        }
        return Ok(());
    }
    // 檔、symlink、FIFO、device 皆走 unlink
    if o.when == When::Always
        && !confirm(&format!("rm: remove file '{}'? ", path), stdin, stderr)
    {
        return Ok(());
    }
    if let Err(e) = fs::remove_file(path) {
        return Err(io_err(e));
    }
    if o.verbose {
        writeln!(stderr, "removed '{}'", path).ok();
    }
    Ok(())
}

fn walk_remove(
    dir: &str,
    o: &Opts,
    stdin: &mut crate::In,
    stderr: crate::Out,
    top_dev: Option<u64>,
) -> Result<(), RmErr> {
    let entries: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(e) => return Err(io_err(e)),
    };
    for p in entries {
        let ps = p.to_string_lossy().to_string();
        let md = match fs::symlink_metadata(&p) {
            Ok(md) => md,
            Err(e) => return Err(io_err(e)),
        };
        if md.is_dir() {
            if let Some(dev) = top_dev {
                if md.dev() != dev {
                    continue; // --one-file-system：跨裝置目錄略過
                }
            }
            walk_remove(&ps, o, stdin, stderr, top_dev)?;
            if let Err(e) = fs::remove_dir(&p) {
                return Err(io_err(e));
            }
            if o.verbose {
                writeln!(stderr, "removed directory '{}'", ps).ok();
            }
        } else {
            if o.when == When::Always
                && !confirm(&format!("rm: remove file '{}'? ", ps), stdin, stderr)
            {
                continue;
            }
            if let Err(e) = fs::remove_file(&p) {
                return Err(io_err(e));
            }
            if o.verbose {
                writeln!(stderr, "removed '{}'", ps).ok();
            }
        }
    }
    Ok(())
}

fn confirm(prompt: &str, stdin: &mut crate::In, stderr: crate::Out) -> bool {
    write!(stderr, "{}", prompt).ok();
    let line = crate::applets::cp::read_line(stdin);
    matches!(line.trim().chars().next(), Some('y') | Some('Y'))
}
