// cp — copy files and directories（man: GNU coreutils 9.7 版）
//
// 措辭／部分 rc 語意對齊本機參考 binary（man 未指定），已記 docs/assumptions.md：
//   -v："'src' -> 'dst'"（stderr，無前綴）；-n 靜默略過 rc0；--update=none-fail → "not replacing" rc1；
//   未加 -R 而 operand 為目錄 → "-r not specified; omitting directory '<s>'"；
//   --preserve=context / -Z → "SELinux was not enabled during the compile time!"（本環境無 SELinux）。
// --sparse/--reflink 接受但為 no-op（不做 sparse 偵測／CoW）；FIFO 以 mkfifo 重建
// （GNU man 語意；參考 binary 對 FIFO 會卡死 → 刻意不追）。

use crate::pathname::io_reason_nosuffix;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[derive(PartialEq, Clone, Copy)]
pub enum Dereference {
    Never,
    Always,
    CommandLine,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Update {
    All,
    None,
    NoneFail,
    Older,
}

pub struct Cfg {
    pub deref: Dereference,
    pub recursive: bool,
    pub preserve_mode: bool,
    pub preserve_ownership: bool,
    pub preserve_timestamps: bool,
    pub preserve_links: bool,
    pub symlink_special: bool,
    pub hard_link: bool,
    pub update: Update,
    pub verbose: bool,
    pub interactive: bool,
    pub force: bool,
    pub remove_destination: bool,
    pub parents: bool,
    pub attributes_only: bool,
    pub one_fs: bool,
    pub backup: bool,
    pub suffix: String,
    pub debug: bool,
    pub preserve_explicit: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            deref: Dereference::CommandLine,
            recursive: false,
            preserve_mode: true,
            preserve_ownership: false,
            preserve_timestamps: false,
            preserve_links: false,
            symlink_special: false,
            hard_link: false,
            update: Update::All,
            verbose: false,
            interactive: false,
            force: false,
            remove_destination: false,
            parents: false,
            attributes_only: false,
            one_fs: false,
            backup: false,
            suffix: "~".to_string(),
            debug: false,
            preserve_explicit: false,
        }
    }
}

pub fn backup_suffix() -> String {
    std::env::var("SIMPLE_BACKUP_SUFFIX").unwrap_or_else(|_| "~".to_string())
}

fn apply_archive(c: &mut Cfg) {
    c.preserve_explicit = true;
    c.deref = Dereference::Never;
    c.preserve_links = true;
    c.recursive = true;
    c.preserve_mode = true;
    c.preserve_ownership = true;
    c.preserve_timestamps = true;
}

pub fn run(args: &[String], mut stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let _ = stdout;
    let mut c = Cfg { suffix: backup_suffix(), ..Default::default() };
    let mut tdir: Option<String> = None;
    let mut no_target_dir = false;
    let mut operands: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        match a {
            "--" => operands.extend(args[i..].iter().cloned()),
            "--help" => {
                writeln!(stdout, "usage: cp [OPTION]... [-T] SOURCE DEST").ok();
                return 0;
            }
            "--version" => {
                writeln!(stdout, "cp (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            "-a" | "--archive" => apply_archive(&mut c),
            "-d" => {
                c.deref = Dereference::Never;
                c.preserve_links = true;
            }
            "-R" | "-r" | "--recursive" => c.recursive = true,
            "-p" => {
                c.preserve_explicit = true;
                c.preserve_mode = true;
                c.preserve_ownership = true;
                c.preserve_timestamps = true;
            }
            "--preserve" => {
                // GNU：可选参数只經 --preserve=LIST；空白分離的下一個參數是 operand
                let list = "all".to_string();
                c.preserve_explicit = true;
                // GNU: --preserve=LIST 是「只保留 LIST」而非累加（--preserve=timestamps 不保 mode）
                c.preserve_mode = false;
                c.preserve_ownership = false;
                c.preserve_timestamps = false;
                c.preserve_links = false;
                for attr in list.split(',') {
                    match attr {
                        "mode" => c.preserve_mode = true,
                        "ownership" => c.preserve_ownership = true,
                        "timestamps" => c.preserve_timestamps = true,
                        "links" => c.preserve_links = true,
                        "xattr" | "no-xattr" => {}
                        "all" => apply_attrs_all(&mut c),
                        "context" | "security" => {
                            writeln!(
                                stderr,
                                "cp: SELinux was not enabled during the compile time!"
                            )
                            .ok();
                            return 1;
                        }
                        _ => {
                            writeln!(stderr, "cp: invalid attribute '{}'", attr).ok();
                            return 2;
                        }
                    }
                }
            }
            "--no-preserve" => {
                let list = match args.get(i) {
                    Some(v) if !v.starts_with("-") => {
                        i += 1;
                        v.clone()
                    }
                    _ => String::new(),
                };
                for attr in list.split(',') {
                    match attr {
                        "mode" => c.preserve_mode = false,
                        "ownership" => c.preserve_ownership = false,
                        "timestamps" => c.preserve_timestamps = false,
                        "links" => c.preserve_links = false,
                        "context" | "xattr" | "all" | "" => {}
                        _ => {
                            writeln!(stderr, "cp: invalid attribute '{}'", attr).ok();
                            return 2;
                        }
                    }
                }
            }
            "-s" | "--symbolic-link" => c.symlink_special = true,
            "-l" | "--link" => c.hard_link = true,
            "-P" | "--no-dereference" => c.deref = Dereference::Never,
            "-L" | "--dereference" => c.deref = Dereference::Always,
            "-H" => c.deref = Dereference::CommandLine,
            "-n" | "--no-clobber" => c.update = Update::None,
            "-i" | "--interactive" => {
                c.interactive = true;
                c.update = Update::All;
            }
            s if s.starts_with("--interactive=") => {
                match &s["--interactive=".len()..] {
                    "never" => c.interactive = false,
                    "always" | "prompt" => c.interactive = true,
                    other => {
                        writeln!(stderr, "cp: invalid argument '{}' for '--interactive'", other).ok();
                        return 2;
                    }
                }
            }
            "-f" | "--force" => c.force = true,
            "-u" => c.update = Update::Older,
            _ if a.starts_with("--update=") => {
                c.update = match &a["--update=".len()..] {
                    "none" => Update::None,
                    "none-fail" => Update::NoneFail,
                    "older" => Update::Older,
                    "all" => Update::All,
                    other => {
                        writeln!(stderr, "cp: invalid argument '{}' for '--update'", other).ok();
                        return 2;
                    }
                };
            }
            _ if a.starts_with("--sparse=") || a.starts_with("--reflink=") => {}
            _ if a.starts_with("--preserve=") => {
                let list = a["--preserve=".len()..].to_string();
                c.preserve_explicit = true;
                // GNU: --preserve=LIST 是「只保留 LIST」而非累加
                c.preserve_mode = false;
                c.preserve_ownership = false;
                c.preserve_timestamps = false;
                c.preserve_links = false;
                for attr in list.split(',') {
                    match attr {
                        "mode" => c.preserve_mode = true,
                        "ownership" => c.preserve_ownership = true,
                        "timestamps" => c.preserve_timestamps = true,
                        "links" => c.preserve_links = true,
                        "xattr" | "no-xattr" => {}
                        "all" => apply_attrs_all(&mut c),
                        "context" | "security" => {
                            writeln!(stderr, "cp: SELinux was not enabled during the compile time!").ok();
                            return 1;
                        }
                        _ => {
                            writeln!(stderr, "cp: invalid attribute '{}'", attr).ok();
                            return 2;
                        }
                    }
                }
            }
            _ if a.starts_with("--no-preserve=") => {
                let list = a["--no-preserve=".len()..].to_string();
                for attr in list.split(',') {
                    match attr {
                        "mode" => c.preserve_mode = false,
                        "ownership" => c.preserve_ownership = false,
                        "timestamps" => c.preserve_timestamps = false,
                        "links" => c.preserve_links = false,
                        "context" | "xattr" | "all" | "" => {}
                        _ => {
                            writeln!(stderr, "cp: invalid attribute '{}'", attr).ok();
                            return 2;
                        }
                    }
                }
            }
            _ if a.starts_with("--suffix=") => {
                c.suffix = a["--suffix=".len()..].to_string();
            }
            _ if a.starts_with("--target-directory=") => {
                tdir = Some(a["--target-directory=".len()..].to_string());
            }
            "--update" => {
                c.update = match args.get(i).map(|s| s.as_str()) {
                    Some("none") => {
                        i += 1;
                        Update::None
                    }
                    Some("none-fail") => {
                        i += 1;
                        Update::NoneFail
                    }
                    Some("older") => {
                        i += 1;
                        Update::Older
                    }
                    Some("all") => {
                        i += 1;
                        Update::All
                    }
                    _ => Update::Older,
                };
            }
            "--remove-destination" => c.remove_destination = true,
            "--parents" => c.parents = true,
            "--attributes-only" => c.attributes_only = true,
            "-x" | "--one-file-system" => c.one_fs = true,
            "-v" | "--verbose" => c.verbose = true,
            "--debug" => {
                c.debug = true;
                c.verbose = true;
            }
            "-b" => c.backup = true,
            "--backup" => {
                if let Some(v) = args.get(i).map(|s| s.as_str()) {
                    if v == "none" || v == "off" || v == "existing" || v == "numbered" || v == "nil" {
                        i += 1;
                        c.backup = !(v == "none" || v == "off");
                    } else {
                        c.backup = true;
                    }
                } else {
                    c.backup = true;
                }
            }
            "-S" | "--suffix" => {
                if i >= args.len() {
                    writeln!(stderr, "cp: option requires an argument -- 'S'").ok();
                    return 2;
                }
                c.suffix = args[i].clone();
                i += 1;
            }
            "-t" | "--target-directory" => {
                if i >= args.len() {
                    writeln!(stderr, "cp: option requires an argument -- 't'").ok();
                    return 2;
                }
                tdir = Some(args[i].clone());
                i += 1;
            }
            "-T" | "--no-target-directory" => no_target_dir = true,
            "--strip-trailing-slashes" => {}
            "--sparse" | "--reflink" => {
                if let Some(v) = args.get(i).map(|s| s.as_str()) {
                    if v == "never" || v == "auto" || v == "always" || v == "yes" || v == "no" {
                        i += 1;
                    }
                }
            }
            "-Z" | "--context" => {
                writeln!(stderr, "cp: SELinux was not enabled during the compile time!").ok();
                return 1;
            }
            "--copy-contents" | "--keep-directory-symlink" => {}
            s if s.starts_with("--") => {
                writeln!(stderr, "cp: unrecognized option '{}'", s).ok();
                return 2;
            }
            s if s.len() > 1 && s.starts_with('-') => {
                for ch in s[1..].chars() {
                    match ch {
                        'a' => apply_archive(&mut c),
                        'd' => {
                            c.deref = Dereference::Never;
                            c.preserve_links = true;
                        }
                        'R' | 'r' => c.recursive = true,
                        'p' => {
                            c.preserve_mode = true;
                            c.preserve_ownership = true;
                            c.preserve_timestamps = true;
                        }
                        's' => c.symlink_special = true,
                        'l' => c.hard_link = true,
                        'P' => c.deref = Dereference::Never,
                        'L' => c.deref = Dereference::Always,
                        'H' => c.deref = Dereference::CommandLine,
                        'n' => c.update = Update::None,
                        'i' => {
                            c.interactive = true;
                            c.update = Update::All;
                        }
                        'f' => c.force = true,
                        'u' => c.update = Update::Older,
                        'v' => c.verbose = true,
                        'x' => c.one_fs = true,
                        'b' => c.backup = true,
                        'T' => no_target_dir = true,
                        't' | 'S' => {
                            writeln!(stderr, "cp: option requires an argument -- '{}'", ch).ok();
                            return 2;
                        }
                        _ => {
                            writeln!(stderr, "cp: invalid option -- '{}'", ch).ok();
                            return 2;
                        }
                    }
                }
            }
            _ => operands.push(a.to_string()),
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "cp: missing file operand").ok();
        return 2;
    }
    if operands.len() == 1 && tdir.is_none() {
        writeln!(
            stderr,
            "cp: missing destination file operand after '{}'",
            operands[0]
        )
        .ok();
        return 2;
    }

    if c.parents {
        let dd = match &tdir {
            Some(d) => d.clone(),
            None => operands.last().unwrap().clone(),
        };
        if !is_dir_operand(&dd) {
            writeln!(stderr, "cp: with --parents, the destination must be a directory").ok();
            return 1;
        }
    }
    let mut rc = 0;
    if let Some(td) = tdir.clone() {
        let td = td.trim_end_matches('/').to_string();
        for s in &operands {
            if c.parents {
                if copy_with_parents(s, &td, &c, &mut stdin, stderr).is_err() {
                    rc = 1;
                }
            } else if copy_entry(s, &format!("{}/{}", td, base_name(s)), true, &c, &mut stdin, stderr)
                .is_err()
            {
                rc = 1;
            }
        }
        return rc;
    }
    let last = operands.last().unwrap().clone();
    let multi = operands.len() >= 2;
    if multi && !no_target_dir && is_dir_operand(&last) {
        let dest = last.trim_end_matches('/').to_string();
        for s in &operands[..operands.len() - 1] {
            if c.parents {
                if copy_with_parents(s, &dest, &c, &mut stdin, stderr).is_err() {
                    rc = 1;
                }
            } else if copy_entry(s, &format!("{}/{}", dest, base_name(s)), true, &c, &mut stdin, stderr)
                .is_err()
            {
                rc = 1;
            }
        }
        return rc;
    }
    if multi {
        if operands.len() > 2 && !is_dir_operand(&last) {
            writeln!(stderr, "cp: target '{}' is not a directory", last).ok();
            return 1;
        }
        return match copy_entry(&operands[0], &operands[1], true, &c, &mut stdin, stderr) {
            Ok(()) => 0,
            Err(()) => 1,
        };
    }
    0
}

fn apply_attrs_all(c: &mut Cfg) {
    c.preserve_mode = true;
    c.preserve_ownership = true;
    c.preserve_timestamps = true;
    c.preserve_links = true;
}

fn base_name(p: &str) -> String {
    let t = p.trim_end_matches('/');
    t.rsplit('/').next().unwrap_or(t).to_string()
}

fn is_dir_operand(p: &str) -> bool {
    fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

fn copy_with_parents(
    src: &str,
    dest_dir: &str,
    c: &Cfg,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<(), ()> {
    let rel = src.trim_start_matches('/').trim_end_matches('/');
    if rel.is_empty() {
        writeln!(stderr, "cp: cannot copy '/' with --parents").ok();
        return Err(());
    }
    // --parents 重建來源路徑的中間目錄層（GNU/ref：cp --parents a/b/c d → d/a/b/c）
    if let Some(pos) = rel.rfind('/') {
        let dir = format!("{}/{}", dest_dir, &rel[..pos]);
        if let Err(e) = fs::create_dir_all(&dir) {
            writeln!(stderr, "cp: cannot create directory '{}': {}", dir, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    }
    copy_entry(src, &format!("{}/{}", dest_dir, rel), true, c, stdin, stderr)
}

/// 複製單一項目（top：-H 是否適用）。Err 代表已印錯誤、rc 應為 1。
fn copy_entry(
    src: &str,
    dst: &str,
    top: bool,
    c: &Cfg,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<(), ()> {
    let lmd = match fs::symlink_metadata(src) {
        Ok(m) => m,
        Err(e) => {
            writeln!(stderr, "cp: cannot stat '{}': {}", src, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    };
    let follow = match c.deref {
        Dereference::Always => true,
        Dereference::Never => false,
        Dereference::CommandLine => top,
    };
    let md = if follow {
        match fs::metadata(src) {
            Ok(m) => m,
            Err(e) => {
                writeln!(stderr, "cp: cannot stat '{}': {}", src, io_reason_nosuffix(&e)).ok();
                return Err(());
            }
        }
    } else {
        lmd.clone()
    };

    if md.is_dir() {
        if !c.recursive {
            writeln!(stderr, "cp: -r not specified; omitting directory '{}'", src).ok();
            return Err(());
        }
        return copy_dir_tree(src, dst, &md, c, stdin, stderr);
    }

    // symlink 且未追隨（或 -s 要求做 symlink）→ 原樣複製 link 字串
    let is_link = follow == false && lmd.is_symlink();
    if is_link || c.symlink_special {
        if c.symlink_special && !is_link {
            let target = src.to_string();
            if prepare_dest(src, dst, &lmd, &md, c, stdin, stderr)? == Skip::Yes {
                return Ok(());
            }
            make_symlink(src, &target, dst, c, stderr)?;
            return Ok(());
        }
        if is_link {
            let tgt = match fs::read_link(src) {
                Ok(t) => t,
                Err(e) => {
                    writeln!(stderr, "cp: cannot read link '{}': {}", src, io_reason_nosuffix(&e)).ok();
                    return Err(());
                }
            };
            if prepare_dest(src, dst, &lmd, &md, c, stdin, stderr)? == Skip::Yes {
                return Ok(());
            }
            make_symlink(src, &tgt.to_string_lossy(), dst, c, stderr)?;
            return Ok(());
        }
    }

    if c.hard_link && !md.is_file() {
        writeln!(stderr, "cp: cannot hard link non-file '{}'", src).ok();
        return Err(());
    }
    if c.hard_link {
        if prepare_dest(src, dst, &lmd, &md, c, stdin, stderr)? == Skip::Yes {
            return Ok(());
        }
        if let Err(e) = fs::hard_link(src, dst) {
            writeln!(stderr, "cp: cannot create hard link '{}' to '{}': {}", dst, src, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
        log_copy(src, dst, c, stderr);
        return Ok(());
    }

    // 特殊檔：FIFO 以 mkfifo 重建（GNU man 語意）；socket/device 不支援
    let ft = md.mode() & 0o170000;
    if ft == 0o010000 {
        if prepare_dest(src, dst, &lmd, &md, c, stdin, stderr)? == Skip::Yes {
            return Ok(());
        }
        let mknod = 0o010000u32 | (md.mode() & 0o7777);
        let r = crate::sys::mknod(dst, mknod, 0);
        if r != 0 {
            writeln!(stderr, "cp: cannot create special file '{}': {}", dst, io_reason_nosuffix(&std::io::Error::last_os_error())).ok();
            return Err(());
        }
        log_copy(src, dst, c, stderr);
        return Ok(());
    }
    if ft == 0o020000 || ft == 0o060000 || ft == 0o140000 {
        writeln!(stderr, "cp: cannot copy special file '{}'", src).ok();
        return Err(());
    }

    copy_one_file(src, dst, &md, c, stdin, stderr)
}

#[derive(PartialEq)]
enum Skip {
    Yes,
    No,
}

/// 處理目的地存在時的 update/interactive/backup 決策；回傳 Skip::Yes 表示略過（非錯誤）。
fn prepare_dest(
    src: &str,
    dst: &str,
    src_lmd: &std::fs::Metadata,
    src_md: &std::fs::Metadata,
    c: &Cfg,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<Skip, ()> {
    let dmd = match fs::symlink_metadata(dst) {
        Ok(m) => m,
        Err(_) => return Ok(Skip::No),
    };
    // 同一檔（dev+ino）→ 錯誤
    if src_md.dev() == dmd.dev() && src_md.ino() == dmd.ino() && !c.symlink_special {
        writeln!(stderr, "cp: '{}' and '{}' are the same file", src, dst).ok();
        let _ = src_lmd;
        return Err(());
    }
    match c.update {
        Update::None => return Ok(Skip::Yes),
        Update::NoneFail => {
            writeln!(stderr, "cp: not replacing '{}'", dst).ok();
            return Err(());
        }
        Update::Older => {
            if dmd.mtime() >= src_md.mtime() {
                return Ok(Skip::Yes);
            }
        }
        Update::All => {}
    }
    if c.interactive {
        write!(stderr, "cp: overwrite '{}'? ", dst).ok();
        let line = read_line(stdin);
        if !matches!(line.trim().chars().next(), Some('y') | Some('Y')) {
            return Ok(Skip::Yes);
        }
    }
    if c.backup {
        let bk = format!("{}{}", dst, c.suffix);
        if let Err(e) = fs::rename(dst, &bk) {
            writeln!(stderr, "cp: cannot create backup '{}': {}", bk, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    } else if c.remove_destination {
        let _ = fs::remove_file(dst);
    }
    // 參考（uutils）覆寫既有目的地時不 unlink：原地開檔截斷、保留既有 mode
    Ok(Skip::No)
}

fn make_symlink(src: &str, target: &str, dst: &str, c: &Cfg, stderr: crate::Out) -> Result<(), ()> {
    if let Err(e) = std::os::unix::fs::symlink(target, dst) {
        writeln!(stderr, "cp: cannot create symlink '{}': {}", dst, io_reason_nosuffix(&e)).ok();
        return Err(());
    }
    let _ = src;
    log_copy(src, dst, c, stderr);
    Ok(())
}

fn log_copy(src: &str, dst: &str, c: &Cfg, stderr: crate::Out) {
    if c.debug {
        write!(
            stderr,
            "copy offload: yes, reflink: unsupported, sparse detection: no"
        )
        .ok();
    }
    if c.verbose {
        writeln!(stderr, "'{}' -> '{}'", src, dst).ok();
    }
}

fn copy_dir_tree(
    src: &str,
    dst: &str,
    src_md: &std::fs::Metadata,
    c: &Cfg,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<(), ()> {
    // 目的地目錄：不存在→建立；存在→直接入內
    if let Err(e) = fs::create_dir(dst) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            writeln!(stderr, "cp: cannot create directory '{}': {}", dst, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    }
    if c.verbose {
        writeln!(stderr, "created directory '{}'", dst).ok();
    }
    let entries = match fs::read_dir(src) {
        Ok(rd) => rd,
        Err(e) => {
            writeln!(stderr, "cp: cannot access '{}': {}", src, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    };
    let top_dev = if c.one_fs { Some(src_md.dev()) } else { None };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        let cs = format!("{}/{}", src.trim_end_matches('/'), name);
        let cd = format!("{}/{}", dst.trim_end_matches('/'), name);
        if let Some(dev) = top_dev {
            if let Ok(m) = fs::metadata(&cs) {
                if m.dev() != dev {
                    continue;
                }
            }
        }
        copy_entry(&cs, &cd, false, c, stdin, stderr)?;
    }
    // 目錄屬性
    if c.preserve_mode {
        let _ = fs::set_permissions(dst, fs::Permissions::from_mode(src_md.mode() & 0o7777));
    }
    if c.preserve_ownership {
        chown(dst, src_md.uid(), src_md.gid(), stderr);
    }
    if c.preserve_timestamps {
        set_times(dst, src_md.atime(), src_md.atime_nsec(), src_md.mtime(), src_md.mtime_nsec());
    }
    log_copy(src, dst, c, stderr);
    Ok(())
}

fn chown(path: &str, uid: u32, gid: u32, stderr: crate::Out) {
    use std::ffi::CString;
    extern "C" {
        fn chown(path: *const i8, uid: u32, gid: u32) -> i32;
    }
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let r = unsafe { chown(c.as_ptr(), uid, gid) };
    if r != 0 {
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(1) {
            writeln!(stderr, "cp: failed to preserve ownership for '{}': {}", path, io_reason_nosuffix(&e)).ok();
        }
    }
}

fn set_times(path: &str, atime: i64, atime_nsec: i64, mtime: i64, mtime_nsec: i64) {
    use std::ffi::CString;
    extern "C" {
        #[link_name = "utimensat"]
        fn utimensat_cp(dirfd: i32, path: *const i8, times: *const [i64; 4], flags: i32) -> i32;
    }
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let times = [atime, atime_nsec, mtime, mtime_nsec];
    unsafe {
        utimensat_cp(-100, c.as_ptr(), &times, 0);
    }
}

pub fn mtime_ns_of(m: &fs::Metadata) -> i128 {
    match m.modified() {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_nanos() as i128,
            Err(e) => -(e.duration().as_nanos() as i128),
        },
        Err(_) => 0,
    }
}

pub fn read_line(stdin: &mut crate::In) -> String {
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    loop {
        match stdin.read(&mut b) {
            Ok(0) => break,
            Ok(_) => {
                buf.push(b[0]);
                if b[0] == b'\n' {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn copy_one_file(
    src: &str,
    dst: &str,
    md: &std::fs::Metadata,
    c: &Cfg,
    stdin: &mut crate::In,
    stderr: crate::Out,
) -> Result<(), ()> {
    let dst_existed = fs::symlink_metadata(dst).is_ok();
    if prepare_dest(src, dst, md, md, c, stdin, stderr)? == Skip::Yes {
        return Ok(());
    }
    let mut fin = match fs::File::open(src) {
        Ok(f) => f,
        Err(e) => {
            writeln!(stderr, "cp: cannot open '{}' for reading: {}", src, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    };
    let mut fout = match fs::File::create(dst) {
        Ok(f) => f,
        Err(e) => {
            if c.force {
                let _ = fs::remove_file(dst);
                match fs::File::create(dst) {
                    Ok(f) => f,
                    Err(e2) => {
                        writeln!(stderr, "cp: cannot create '{}': {}", dst, io_reason_nosuffix(&e2)).ok();
                        return Err(());
                    }
                }
            } else {
                writeln!(stderr, "cp: cannot create '{}': {}", dst, io_reason_nosuffix(&e)).ok();
                return Err(());
            }
        }
    };
    if !c.attributes_only {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = match fin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    writeln!(stderr, "cp: error reading '{}': {}", src, io_reason_nosuffix(&e)).ok();
                    return Err(());
                }
            };
            if let Err(e) = fout.write_all(&buf[..n]) {
                writeln!(stderr, "cp: error writing '{}': {}", dst, io_reason_nosuffix(&e)).ok();
                return Err(());
            }
        }
    }
    drop(fout);
    if c.preserve_mode && (!dst_existed || c.preserve_explicit) {
        let _ = fs::set_permissions(dst, fs::Permissions::from_mode(md.mode() & 0o7777));
    }
    if c.preserve_ownership {
        chown(dst, md.uid(), md.gid(), stderr);
    }
    if c.preserve_timestamps {
        set_times(dst, md.atime(), md.atime_nsec(), md.mtime(), md.mtime_nsec());
    }
    log_copy(src, dst, c, stderr);
    Ok(())
}

// ---- 供 mv 跨裝置 fallback 使用 ----

pub fn mv_copy_fallback(
    src: &str,
    dst: &str,
    stderr: crate::Out,
) -> Result<(), ()> {
    let c = Cfg {
        deref: Dereference::Never,
        recursive: true,
        preserve_mode: true,
        preserve_ownership: true,
        preserve_timestamps: true,
        preserve_links: true,
        ..Default::default()
    };
    let md = fs::symlink_metadata(src).map_err(|_| ())?;
    let mut si = std::io::stdin();
    let mut stdin: crate::In = &mut si;
    if md.is_dir() {
        copy_dir_tree(src, dst, &md, &c, &mut stdin, stderr)?;
        let _ = fs::remove_dir_all(src);
    } else if md.is_symlink() {
        let tgt = fs::read_link(src).map_err(|_| ())?;
        std::os::unix::fs::symlink(tgt, dst).map_err(|_| ())?;
        let _ = fs::remove_file(src);
    } else {
        copy_one_file(src, dst, &md, &c, &mut stdin, stderr)?;
        let _ = fs::remove_file(src);
    }
    Ok(())
}
