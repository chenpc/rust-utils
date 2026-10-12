//! chroot(1) — Run COMMAND with root directory set to NEWROOT.
//!
//! 依據 man page（docs/man/chroot.1.gz）與本機參考實測：
//! - NEWROOT 不存在 → `chroot: cannot change root directory to '<NEWROOT>': no such directory`
//! - `--skip-chdir` 只允許 NEWROOT 為 `/`
//! - `--userspec` / `--groups` 在 chroot 之前解析，解析失敗 → `chroot: invalid user`
//! - chroot(2) 失敗 → `chroot: cannot chroot to '<NEWROOT>': <reason (os error N)>`
//! 差異：chroot(2) 只在 child 的 pre_exec 中執行（父行程是 test binary，不能真的改 root）；
//! 失敗時用 socketpair 把 errno 回報給父行程再印出訊息（記錄於 docs/assumptions.md）。

use crate::exec::{lookup_group, lookup_user, status_code};
use crate::pathname::{clap_value_required, infer_long_opt, usage_hint};
use crate::{In, Out};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;

const CHROOT_HELP: &str = r#"Run COMMAND with root directory set to NEWROOT.

Usage: chroot [OPTION]... NEWROOT [COMMAND [ARG]...]

Options:
      --groups <GROUP1,GROUP2...>  Comma-separated list of groups to switch to
      --userspec <USER:GROUP>      Colon-separated user and group to switch to.
      --skip-chdir                 Use this option to not change the working directory to / after
                                   changing the root directory to newroot, i.e., inside the chroot.
  -h, --help                       Print help
  -V, --version                    Print version
"#;

const CHROOT_USAGE: &str = "chroot [OPTION]... NEWROOT [COMMAND [ARG]...]";
const CHROOT_LONGS: &[&str] = &["groups", "help", "skip-chdir", "userspec", "version"];

fn clap_tail(stderr: Out) {
    write!(
        stderr,
        "\nUsage: {}\n\nFor more information, try '--help'.\n",
        CHROOT_USAGE
    )
    .ok();
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    clap_tail(stderr);
    125
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut newroot: Option<String> = None;
    let mut command: Option<String> = None;
    let mut cmd_args: Vec<String> = Vec::new();
    let mut skip_chdir = false;
    let mut userspec: Option<String> = None;
    let mut groups: Option<String> = None;
    let mut end_of_options = false;
    let mut i = 0usize;

    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if end_of_options || command.is_some() {
            cmd_args.push(a);
            continue;
        }
        if a == "--" {
            end_of_options = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let inline = a[1..].find('=').map(|p| a[p + 2..].to_string());
            let long = match name.as_str() {
                "groups" => "groups",
                "userspec" => "userspec",
                "skip-chdir" => "skip-chdir",
                "help" => "help",
                "version" => "version",
                _ => {
                    if infer_long_opt(&name, CHROOT_LONGS).is_none() {
                        return unexpected(&a, stderr);
                    }
                    return unexpected(&a, stderr);
                }
            };
            match long {
                "help" => {
                    if let Some(v) = inline {
                        crate::pathname::clap_unexpected_value("help", &v, CHROOT_USAGE, stderr);
                        return 125;
                    }
                    write!(stdout, "{}", CHROOT_HELP).ok();
                    return 0;
                }
                "version" => {
                    if let Some(v) = inline {
                        crate::pathname::clap_unexpected_value("version", &v, CHROOT_USAGE, stderr);
                        return 125;
                    }
                    write!(stdout, "chroot (uutils coreutils) 0.10.0\n").ok();
                    return 0;
                }
                "skip-chdir" => skip_chdir = true,
                "userspec" => {
                    userspec = Some(match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                clap_value_required("userspec", "USER:GROUP", stderr);
                                return 125;
                            }
                        }
                    })
                }
                "groups" => {
                    groups = Some(match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                clap_value_required("groups", "GROUP1,GROUP2...", stderr);
                                return 125;
                            }
                        }
                    })
                }
                _ => {}
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            for c in a[1..].chars() {
                match c {
                    'h' => {
                        write!(stdout, "{}", CHROOT_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        write!(stdout, "chroot (uutils coreutils) 0.10.0\n").ok();
                        return 0;
                    }
                    c => return unexpected(&format!("-{}", c), stderr),
                }
            }
            continue;
        }
        if newroot.is_none() {
            newroot = Some(a);
        } else if command.is_none() {
            command = Some(a);
        } else {
            cmd_args.push(a);
        }
    }

    let root = match newroot {
        Some(r) => r,
        None => {
            write!(
                stderr,
                "error: the following required arguments were not provided:\n  <newroot>\n"
            )
            .ok();
            clap_tail(stderr);
            return 125;
        }
    };
    if skip_chdir && root != "/" {
        write!(
            stderr,
            "chroot: option --skip-chdir only permitted if NEWROOT is old '/'\n"
        )
        .ok();
        write!(stderr, "{}\n", usage_hint("chroot")).ok();
        return 125;
    }
    match std::fs::metadata(&root) {
        Ok(md) if md.is_dir() => {}
        _ => {
            write!(
                stderr,
                "chroot: cannot change root directory to '{}': no such directory\n",
                root
            )
            .ok();
            return 125;
        }
    }

    let mut uid: Option<u32> = None;
    let mut user_name: Option<String> = None;
    let mut gid: Option<u32> = None;
    if let Some(spec) = userspec.as_deref() {
        let mut parts = spec.splitn(2, ':');
        let user = parts.next().unwrap_or("");
        let group = parts.next().unwrap_or("");
        if !user.is_empty() {
            match lookup_user(user) {
                Some(v) => {
                    uid = Some(v);
                    user_name = Some(user.to_string());
                }
                None => {
                    write!(stderr, "chroot: invalid user\n").ok();
                    return 125;
                }
            }
        }
        if !group.is_empty() {
            match lookup_group(group) {
                Some(v) => gid = Some(v),
                None => {
                    write!(stderr, "chroot: invalid group\n").ok();
                    return 125;
                }
            }
        }
    }
    let mut grouplist: Option<Vec<u32>> = None;
    if let Some(spec) = groups.as_deref() {
        let mut list = Vec::new();
        for g in spec.split(',') {
            if g.is_empty() {
                continue;
            }
            match lookup_group(g) {
                Some(v) => list.push(v),
                None => {
                    write!(stderr, "chroot: invalid group\n").ok();
                    return 125;
                }
            }
        }
        grouplist = Some(list);
    }

    let cmd = match command {
        Some(c) => c,
        None => std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
    };
    // 指令路徑由子行程在 chroot 之後解析（與 GNU 相同：新根內的路徑與 PATH）。

    let (mut sock_read, sock_write) = match UnixStream::pair() {
        Ok(p) => p,
        Err(_) => return 125,
    };
    let wfd = sock_write.as_raw_fd();
    let root_c = match std::ffi::CString::new(root.clone()) {
        Ok(c) => c,
        Err(_) => return 125,
    };
    let user_c = user_name.as_ref().and_then(|n| std::ffi::CString::new(n.clone()).ok());
    let groups_v = grouplist.clone();
    let skip = skip_chdir;

    // pre_exec 於 fork 之後、exec 之前執行；失敗時把 (步驟, errno) 寫回父行程。
    let mut builder = std::process::Command::new(&cmd);
    builder.args(&cmd_args);
    unsafe {
        builder.pre_exec(move || {
            let report = |step: u8| -> std::io::Error {
                let err = std::io::Error::last_os_error();
                let code = err.raw_os_error().unwrap_or(1);
                let mut payload = [0u8; 5];
                payload[0] = step;
                payload[1..].copy_from_slice(&code.to_ne_bytes());
                let _ = crate::exec::c_write(wfd, payload.as_ptr(), payload.len());
                err
            };
            if let Some(list) = &groups_v {
                if crate::exec::c_setgroups(list.len(), list.as_ptr()) == -1 {
                    return Err(report(1));
                }
            } else if let Some(name) = &user_c {
                if crate::exec::c_initgroups(name.as_ptr(), uid.unwrap_or(0)) == -1 {
                    return Err(report(1));
                }
            }
            if let Some(g) = gid {
                if crate::exec::c_setgid(g) == -1 {
                    return Err(report(2));
                }
            }
            if let Some(u) = uid {
                if crate::exec::c_setuid(u) == -1 {
                    return Err(report(3));
                }
            }
            if crate::exec::c_chroot(root_c.as_ptr()) == -1 {
                return Err(report(4));
            }
            if !skip && crate::exec::c_chdir(c"/".as_ptr()) == -1 {
                return Err(report(5));
            }
            Ok(())
        });
    }
    let _ = &sock_write;

    let mut child = match builder.spawn() {
        Ok(c) => c,
        Err(spawn_err) => {
            use std::io::Read;
            let _ = sock_read.set_nonblocking(true);
            let mut payload = [0u8; 5];
            let n = sock_read.read(&mut payload).unwrap_or(0);
            if n != 5 {
                // pre_exec 未回報步驟，代表失敗發生在 exec 本身。
                let (reason, rc) = match spawn_err.kind() {
                    std::io::ErrorKind::NotFound => ("No such file or directory".to_string(), 127),
                    std::io::ErrorKind::PermissionDenied => ("Permission denied".to_string(), 126),
                    _ => (spawn_err.to_string(), 127),
                };
                write!(stderr, "chroot: failed to run command '{}': {}\n", cmd, reason).ok();
                return rc;
            }
            let (step, code) = if n == 5 {
                (payload[0], i32::from_ne_bytes([payload[1], payload[2], payload[3], payload[4]]))
            } else {
                (4, 1)
            };
            let reason = std::io::Error::from_raw_os_error(code).to_string();
            if step == 1 || step == 2 {
                write!(stderr, "chroot: cannot set groups: {}\n", reason).ok();
            } else if step == 3 {
                write!(stderr, "chroot: cannot set user id: {}\n", reason).ok();
            } else if step == 5 {
                write!(stderr, "chroot: cannot change working directory: {}\n", reason).ok();
            } else {
                write!(stderr, "chroot: cannot chroot to '{}': {}\n", root, reason).ok();
            }
            return 125;
        }
    };
    match child.wait() {
        Ok(status) => status_code(status),
        Err(_) => 125,
    }
}
