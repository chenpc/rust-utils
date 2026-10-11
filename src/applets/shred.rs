//! shred(1) — Overwrite files to hide contents.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - 預設 3 passes（全 `(random)...`）；`-z` 追加 `(000000)...`
//! - size：無 `-s` 時檔案大小無條件進位到 4096 的倍數；`-s N` 只在實際 size < N 時成長；
//!   `-x/--exact` 不進位
//! - `-n 0`：完全不寫（無 pass 行）；但 `-n 0 -u` 仍執行移除
//! - 移除：`-u` = `--remove=wipe`（rename 到 '0'×basename 長度，再逐次少一個 0）；
//!   `--remove=unlink` 直接 unlink（無 rename 行）；`--remove=wipesync` 另 fsync 目錄
//! - verbose 主語在 rename 之後換成暫存名，最後一行回到**原名**
//! - 非普通檔："shred: F: Not a file"；唯讀："shred: F: failed to open for writing: <reason>"（-f 可修）
//! - random-source 太小："shred: F: File write pass failed: failed to fill whole buffer"

use crate::{In, Out};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::os::unix::fs::PermissionsExt;

const BLOCK: u64 = 4096;

enum Remove {
    None,
    Unlink,
    Wipe,
    WipeSync,
}

struct Cfg {
    passes: u64,
    zero: bool,
    remove: Remove,
    force: bool,
    exact: bool,
    size: Option<u64>,
    random_source: String,
    verbose: bool,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    match run_inner(args, stdin, stdout, stderr) {
        Ok(code) => code,
        Err(code) => code,
    }
}

fn run_inner(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> Result<i32, i32> {
    let mut c = Cfg {
        passes: 3,
        zero: false,
        remove: Remove::None,
        force: false,
        exact: false,
        size: None,
        random_source: "/dev/urandom".to_string(),
        verbose: false,
    };
    let mut operands: Vec<String> = Vec::new();
    let mut passes_raw: Option<String> = None;
    let mut size_raw: Option<String> = None;
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let name = crate::pathname::infer_long_opt(
                &name,
                &[
                    "exact",
                    "force",
                    "help",
                    "iterations",
                    "random-source",
                    "remove",
                    "size",
                    "verbose",
                    "version",
                    "zero",
                ],
            )
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "remove" => {
                    let how = if let Some(pos) = a.find('=') {
                        a[pos + 1..].to_string()
                    } else {
                        // --remove 無值 = wipe（與 -u 相同）
                        "wipe".to_string()
                    };
                    match how.as_str() {
                        "unlink" => c.remove = Remove::Unlink,
                        "wipe" => c.remove = Remove::Wipe,
                        "wipesync" => c.remove = Remove::WipeSync,
                        _ => {
                            writeln!(
                                stderr,
                                "error: invalid value '{}' for '--remove[=<HOW>]'",
                                how
                            )
                            .ok();
                            writeln!(stderr).ok();
                            writeln!(
                                stderr,
                                "  [possible values: unlink, wipe, wipesync]"
                            )
                            .ok();
                            writeln!(stderr).ok();
                            writeln!(stderr, "For more information, try '--help'.").ok();
                            return Err(1);
                        }
                    }
                }
                "iterations" => {
                    passes_raw = Some(long_str(&a, args, &mut i, "iterations", stderr)?);
                }
                "size" => {
                    size_raw = Some(long_str(&a, args, &mut i, "size", stderr)?);
                }
                "random-source" => {
                    c.random_source = long_str(&a, args, &mut i, "random-source", stderr)?;
                }
                "zero" => c.zero = true,
                "force" => c.force = true,
                "exact" => c.exact = true,
                "verbose" => c.verbose = true,
                "help" => {
                    print_help(stdout);
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "shred (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                _ => return Err(unexpected(&a, stderr)),
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            let ch = chars[k];
            let rest: String = chars[k + 1..].iter().collect();
            match ch {
                'n' => passes_raw = Some(short_num(&rest, args, &mut i, 'n', &mut k, stderr)?),
                's' => size_raw = Some(short_num(&rest, args, &mut i, 's', &mut k, stderr)?),
                'u' => c.remove = Remove::Wipe,
                'z' => c.zero = true,
                'f' => c.force = true,
                'x' => c.exact = true,
                'v' => c.verbose = true,
                'h' => {
                    print_help(stdout);
                    return Err(0);
                }
                'V' => {
                    writeln!(stdout, "shred (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                c2 => return Err(unexpected(&format!("-{}", c2), stderr)),
            }
            k += 1;
        }
    }

    if operands.is_empty() {
        writeln!(stderr, "shred: missing file operand").ok();
        writeln!(stderr, "Try 'shred --help' for more information.").ok();
        return Err(1);
    }

    // 數值驗證在 operand 檢查之後（參考：`shred -n f` 先報 missing file operand）
    if let Some(raw) = &passes_raw {
        match raw.parse::<u64>() {
            Ok(v) => c.passes = v,
            Err(_) => {
                writeln!(stderr, "shred: invalid number of passes: '{}'", raw).ok();
                return Err(1);
            }
        }
    }
    if let Some(raw) = &size_raw {
        match parse_size(raw) {
            Some(v) => c.size = Some(v),
            None => {
                writeln!(stderr, "shred: invalid file size: '{}'", raw).ok();
                return Err(1);
            }
        }
    }

    let mut rc = 0;
    for path in &operands {
        if shred_one(path, &c, stderr) {
            rc = 1;
        }
    }
    Ok(rc)}

/// 回 true 表示失敗。
fn shred_one(path: &str, c: &Cfg, stderr: Out) -> bool {
    let md = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            writeln!(
                stderr,
                "shred: {}: {}",
                path,
                crate::pathname::io_reason_nosuffix(&e)
            )
            .ok();
            return true;
        }
    };
    if !md.is_file() {
        writeln!(stderr, "shred: {}: Not a file", path).ok();
        return true;
    }

    // 參考實測（docs/probes/cluster5.md）：
    //   預設 size = ceil(actual/4096)*4096；-x → 實際長度；
    //   -s N：非 -x 時 size = max(N, block-rounded)（永不縮小）；
    //          -x 時 size = max(N, actual)
    //   passes == 0 且無 -z → 完全不寫、不 truncate（檔案不變）
    let actual = md.len();
    let block = (actual + BLOCK - 1) / BLOCK * BLOCK;
    let noop = c.passes == 0 && !c.zero;
    let size: u64 = if noop {
        0
    } else if let Some(v) = c.size {
        v.max(actual)
    } else if c.exact {
        actual
    } else {
        block
    };
    let perms = md.permissions().mode();
    let writable_bit = perms & 0o020 != 0;
    let mut forced = false;

    // 隨機來源先準備完整 buffer：來源不足時檔案不被改寫（實測）
    let total_bytes = c.passes.saturating_mul(size);
    let mut rand: Vec<u8> = Vec::new();
    if total_bytes > 0 {
        match read_random_source(&c.random_source, total_bytes) {
            Ok(v) => rand = v,
            Err(kind) => match kind {
                RandErr::Open(e) => {
                    writeln!(stderr, "shred: {}: {}", c.random_source, e).ok();
                    return true;
                }
                RandErr::Read(e) => {
                    writeln!(stderr, "shred: {}: File write pass failed: {}", path, e).ok();
                    return true;
                }
            },
        }
        if (rand.len() as u64) < total_bytes {
            writeln!(
                stderr,
                "shred: {}: File write pass failed: failed to fill whole buffer",
                path
            )
            .ok();
            return true;
        }
    }

    let mut opened = open_for_write(path);
    if opened.is_err() && !writable_bit && c.force {
        // 參考：-f 先把 mode 設為 0o600（chmod 後才可寫）
        if std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).is_ok() {
            forced = true;
            opened = open_for_write(path);
        }
    }
    if opened.is_err() && (writable_bit || forced) {
        writeln!(
            stderr,
            "shred: {}: File write pass failed: failed to fill whole buffer",
            path
        )
        .ok();
        return true;
    }
    let mut f = match opened {
        Ok(f) => f,
        Err(e) => {
            writeln!(
                stderr,
                "shred: {}: failed to open for writing: {}",
                path,
                crate::pathname::io_reason_nosuffix(&e)
            )
            .ok();
            return true;
        }
    };

    let mut pass_no = 0u64;
    for _ in 0..c.passes {
        pass_no += 1;
        if c.verbose {
            writeln!(
                stderr,
                "shred: {}: pass {}/{} (random)...",
                path,
                pass_no,
                c.passes + if c.zero { 1 } else { 0 }
            )
            .ok();
        }
        if size == 0 {
            continue;
        }
        let chunk = &rand[((pass_no - 1) as usize * size as usize)..(pass_no as usize * size as usize)];
        // 每一輪都從 offset 0 重寫（參考實測：`-z` 後整個檔案都是 0，
        // 而非最後一段落在截斷範圍外）。
        let _ = f.rewind();
        if let Err(e) = f.write_all(chunk) {
            writeln!(
                stderr,
                "shred: {}: File write pass failed: failed to fill whole buffer: {}",
                path, e
            )
            .ok();
            return true;
        }
    }
    if c.zero && (c.passes > 0 || size > 0) {
        if c.verbose {
            writeln!(
                stderr,
                "shred: {}: pass {}/{} (000000)...",
                path,
                pass_no + 1,
                pass_no + 1
            )
            .ok();
        }
        if size > 0 {
            let zeros = vec![0u8; size as usize];
            let _ = f.rewind();
            if let Err(e) = f.write_all(&zeros) {
                writeln!(
                    stderr,
                    "shred: {}: File write pass failed: {}",
                    path,
                    crate::pathname::io_reason_nosuffix(&e)
                )
                .ok();
                return true;
            }
        }
    }
    if size > 0 {
        let _ = f.flush();
        let _ = f.set_len(size);
        let _ = f.sync_all();
    }
    drop(f);
    if forced {
        // 參考實測：-f 完成後 mode 恒為 0o200
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o200));
    }

    match c.remove {
        Remove::None => false,
        Remove::Unlink => {
            if c.verbose {
                writeln!(stderr, "shred: {}: removing", path).ok();
            }
            match std::fs::remove_file(path) {
                Ok(()) => {
                    if c.verbose {
                        writeln!(stderr, "shred: {}: removed", path).ok();
                    }
                    false
                }
                Err(e) => {
                    writeln!(stderr, "shred: {}: cannot remove: {}", path, e).ok();
                    true
                }
            }
        }
        Remove::Wipe | Remove::WipeSync => {
            wipe_remove(path, c, stderr)
        }
    }
}

fn wipe_remove(path: &str, c: &Cfg, stderr: Out) -> bool {
    if c.verbose {
        writeln!(stderr, "shred: {}: removing", path).ok();
    }
    let p = std::path::Path::new(path);
    let parent = p.parent().map(|x| x.to_string_lossy().to_string());
    let name = p
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_default();
    // Path::new("f").parent() == Some("")：空字串要當成「目前目錄」，
    // 否則 prefix 會變成 "/" 而 rename 到 /0 失敗（實測踩到）。
    let base = parent.clone().filter(|x| !x.is_empty());
    let dir = base.clone().unwrap_or_else(|| ".".to_string());
    let prefix = match base {
        Some(d) => format!("{}/", d.trim_end_matches('/')),
        None => String::new(),
    };
    let n0 = name.chars().count();
    let mut cur = path.to_string();
    // rename 到 0×n0，之後逐次少一個 0；verbose 主語 = 當時的暫存名
    for n in (1..=n0).rev() {
        let target = format!("{}{}", prefix, "0".repeat(n));
        if std::fs::rename(&cur, &target).is_err() {
            break;
        }
        if c.verbose {
            writeln!(stderr, "shred: {}: renamed to {}", cur, target).ok();
        }
        cur = target;
    }
    if let Remove::WipeSync = c.remove {
        let _ = std::fs::File::open(&dir).map(|d| {
            let _ = d.sync_all();
        });
    }
    match std::fs::remove_file(&cur) {
        Ok(()) => {
            if c.verbose {
                writeln!(stderr, "shred: {}: removed", path).ok();
            }
            false
        }
        Err(e) => {
            writeln!(stderr, "shred: {}: cannot remove: {}", path, e).ok();
            true
        }
    }
}

fn open_for_write(path: &str) -> std::io::Result<File> {
    OpenOptions::new().write(true).open(path)
}

enum RandErr {
    /// 開檔失敗：訊息報來源路徑（`shred: <source>: <reason>`）
    Open(String),
    /// 讀取失敗／來源是目錄：訊息報目標檔（`shred: <target>: File write pass failed: <reason>`）
    Read(String),
}

fn read_random_source(path: &str, need: u64) -> Result<Vec<u8>, RandErr> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) => return Err(RandErr::Open(crate::pathname::io_reason_nosuffix(&e))),
    };
    // 來源比 need 短時回傳部分資料，由呼叫端報 "failed to fill whole buffer"
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = vec![0u8; 8192];
    loop {
        match f.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => return Err(RandErr::Read(crate::pathname::io_reason_nosuffix(&e))),
        }
        if buf.len() as u64 >= need {
            break;
        }
    }
    Ok(buf)
}

fn parse_size(s: &str) -> Option<u64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    // 後綴表依本機參考實測：KB/kB=1000、K/k=1024、MB=1000^2、M/m=1024^2、
    // GB=1000^3、G/g=1024^3、TB=1000^4、T/t=1024^4、P/p=1024^5、E/e=1024^6、b=512；
    // 大寫 B 單獨不是後綴（`1B` 被拒）、`1Ki`/`1mb`/`1BB` 等亦被拒。
    const TABLE: &[(&str, u64)] = &[
        ("KB", 1_000u64),
        ("kB", 1_000u64),
        ("MB", 1_000_000u64),
        ("GB", 1_000_000_000u64),
        ("TB", 1_000_000_000_000u64),
        ("K", 1024u64),
        ("k", 1024u64),
        ("M", 1024u64 * 1024),
        ("m", 1024u64 * 1024),
        ("G", 1024u64 * 1024 * 1024),
        ("g", 1024u64 * 1024 * 1024),
        ("T", 1024u64 * 1024 * 1024 * 1024),
        ("t", 1024u64 * 1024 * 1024 * 1024),
        ("P", 1024u64 * 1024 * 1024 * 1024 * 1024),
        ("p", 1024u64 * 1024 * 1024 * 1024 * 1024),
        ("E", 1024u64 * 1024 * 1024 * 1024 * 1024 * 1024),
        ("e", 1024u64 * 1024 * 1024 * 1024 * 1024 * 1024),
        ("b", 512u64),
    ];
    for (suf, mult) in TABLE {
        if let Some(num) = t.strip_suffix(suf) {
            return num.parse::<u64>().ok().and_then(|v| v.checked_mul(*mult));
        }
    }
    t.parse::<u64>().ok()
}

fn long_str(
    a: &str,
    args: &[String],
    i: &mut usize,
    long: &str,
    stderr: Out,
) -> Result<String, i32> {
    if let Some(pos) = a.find('=') {
        return Ok(a[pos + 1..].to_string());
    }
    if *i >= args.len() {
        crate::pathname::clap_value_required(long, value_metavar_long(long), stderr);
        return Err(1);
    }
    let v = args[*i].clone();
    *i += 1;
    Ok(v)
}

fn short_num(
    rest: &str,
    args: &[String],
    i: &mut usize,
    ch: char,
    k: &mut usize,
    stderr: Out,
) -> Result<String, i32> {
    if !rest.is_empty() {
        *k = usize::MAX - 1;
        // clap：`-X=value` 的 '=' 不屬於 value
        return Ok(rest.strip_prefix('=').unwrap_or(rest).to_string());
    }
    if *i >= args.len() {
        let (long, metavar) = match ch {
            'n' => ("iterations", "NUMBER"),
            's' => ("size", "N"),
            _ => ("value", "VALUE"),
        };
        crate::pathname::clap_value_required(long, metavar, stderr);
        return Err(1);
    }
    let v = args[*i].clone();
    *i += 1;
    *k = usize::MAX - 1;
    Ok(v)
}

const SHRED_HELP: &str = r#"Overwrite the specified FILE(s) repeatedly, in order to make it harder for even
very expensive hardware probing to recover the data.

Usage: shred [OPTION]... FILE...

Arguments:
  [file]...  

Options:
  -f, --force                          change permissions to allow writing if necessary
  -n, --iterations <NUMBER>            overwrite N times instead of the default (3) [default: 3]
  -s, --size <N>                       shred this many bytes (suffixes like K, M, G accepted)
  -u                                   deallocate and remove file after overwriting
      --remove[=<HOW>]                 like -u but give control on HOW to delete;  See below
                                       [possible values: unlink, wipe, wipesync]
  -v, --verbose                        show progress
  -x, --exact                          do not round file sizes up to the next full block;
                                       this is the default for non-regular files
  -z, --zero                           add a final overwrite with zeros to hide shredding
      --random-source <random-source>  take random bytes from FILE
  -h, --help                           Print help
  -V, --version                        Print version

Delete FILE(s) if --remove (-u) is specified. The default is not to remove
the files because it is common to operate on device files like /dev/hda, and
those files usually should not be removed.

CAUTION: Note that shred relies on a very important assumption: that the file
system overwrites data in place. This is the traditional way to do things, but
many modern file system designs do not satisfy this assumption. The following
are examples of file systems on which shred is not effective, or is not
guaranteed to be effective in all file system modes:

 - log-structured or journal file systems, such as those supplied with
   AIX and Solaris (and JFS, ReiserFS, XFS, Ext3, etc.)

 - file systems that write redundant data and carry on even if some writes
   fail, such as RAID-based file systems

 - file systems that make snapshots, such as Network Appliance's NFS server

 - file systems that cache in temporary locations, such as NFS
   version 3 clients

 - compressed file systems

In the case of ext3 file systems, the above disclaimer applies (and shred is
thus of limited effectiveness) only in data=journal mode, which journals file
data in addition to just metadata. In both the data=ordered (default) and
data=writeback modes, shred works as usual. Ext3 journal modes can be changed
by adding the data=something option to the mount options for a particular
file system in the /etc/fstab file, as documented in the mount man page (`man
mount`).

In addition, file system backups and remote mirrors may contain copies of
the file that cannot be removed, and that will allow a shredded file to be
recovered later.
"#;

fn print_help(stdout: Out) {
    write!(stdout, "{}", SHRED_HELP).ok();
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: shred [OPTION]... FILE...").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}
fn value_metavar_long(long: &str) -> &'static str {
    match long {
        "iterations" => "NUMBER",
        "size" => "N",
        "random-source" => "random-source",
        "remove" => "HOW",
        _ => "VALUE",
    }
}
