//! init — initramfs 的 PID 1（取代 busybox /init 腳本）。
//!
//! 流程（對照舊 busybox init，dev log 有記錄）：
//! 1. mount proc/sysfs/devtmpfs
//! 2. finit_module 載入 9p 模組（順序：netfs → 9pnet → 9p → 9pnet_virtio）
//! 3. mount 9p：hostshare → /mnt、hostout → /out
//! 4. /proc/cmdline 解析：
//!    - rd.test=<id> → 跑 /mnt/tests/<id>.sh（本專案 sh）或 /mnt/tests/<id>（二進位），
//!      輸出 QEMU-TEST: PASS/FAIL 後 poweroff
//!    - rd.shell=1 → 把 /mnt/bin/coreutils 與 applets.txt 的 symlink 進 /bin，
//!      rd.cmd=<id> 則跑 /mnt/<id>.sh（stdin=empty），否則互動 sh
//!    - 其他 → 直接 poweroff
//! 標記字串 QEMU-INIT:/QEMU-TEST:/QEMU-SHELL: 是 scripts/*.sh 的判定依據，不可變動。

use std::io::{Read, Write};

use crate::applets::sh;
use crate::sys;

type In<'a> = &'a mut dyn Read;
type Out<'a> = &'a mut dyn Write;

const MODULES: &[&str] = &["netfs", "9pnet", "9p", "9pnet_virtio"];

/// 印一行並 flush（marker 必須即時出現在 serial log）。
macro_rules! say {
    ($w:expr, $s:expr) => {{
        let _ = writeln!($w, "{}", $s);
        let _ = $w.flush();
    }};
}

pub fn run(_args: &[String], stdin: In<'_>, stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    say!(stdout, "QEMU-INIT: coreutils init starting");

    for d in ["/proc", "/sys", "/dev", "/tmp", "/mnt", "/out"] {
        let _ = std::fs::create_dir_all(d);
    }
    mount_opt(stdout, "proc", "/proc", "proc", "");
    mount_opt(stdout, "sysfs", "/sys", "sysfs", "");
    mount_opt(stdout, "devtmpfs", "/dev", "devtmpfs", "");

    for m in MODULES {
        let path = format!("/modules/{m}.ko");
        match std::fs::read(&path) {
            Ok(img) => match sys::init_module(&img) {
                Ok(()) => say!(stdout, &format!("QEMU-INIT: insmod {m}")),
                Err(e) => say!(stdout, &format!("QEMU-INIT: insmod {m} failed: {e}")),
            },
            Err(_) => say!(stdout, &format!("QEMU-INIT: insmod {m} failed")),
        }
    }

    mount_9p(stdout, "hostshare", "/mnt");
    mount_9p(stdout, "hostout", "/out");

    let mut cmdline = String::new();
    let _ = std::fs::File::open("/proc/cmdline").and_then(|mut f| f.read_to_string(&mut cmdline));
    let opt = |key: &str| -> Option<String> {
        cmdline
            .split_whitespace()
            .find_map(|w| w.strip_prefix(key).map(str::to_string))
    };

    if let Some(id) = opt("rd.test=") {
        let rc = run_payload(&id);
        if rc == 0 {
            say!(stdout, "QEMU-TEST: PASS");
        } else {
            say!(stdout, &format!("QEMU-TEST: FAIL (exit {rc})"));
        }
        power_off_logged(stdout);
        return 0;
    }

    if opt("rd.shell=").is_some() {
        link_applets(stdout);
        say!(stdout, "QEMU-SHELL: ready（exit 關機；互動模式可用 Ctrl-a c 進 monitor）");
        if let Some(id) = opt("rd.cmd=") {
            let path = format!("/mnt/{id}.sh");
            if std::path::Path::new(&path).exists() {
                // -c 模式：stdin 給 empty，避免 applet 讀 stdin 時卡在 console
                let mut null = std::io::empty();
                let rc = sh::run_script(&path, &mut null, stdout, stderr);
                say!(stdout, &format!("QEMU-SHELL: cmd exited ({rc})"));
            } else {
                say!(stdout, &format!("QEMU-SHELL: no cmd payload for {id}"));
            }
        } else {
            let rc = sh::run(&[], stdin, stdout, stderr);
            say!(stdout, &format!("QEMU-SHELL: shell exited ({rc})"));
        }
    }

    power_off_logged(stdout);
    0
}

fn power_off_logged(out: &mut dyn Write) {
    if let Err(e) = sys::power_off() {
        say!(out, &format!("QEMU-INIT: power_off failed: {e}"));
    }
    // reboot(2) 在此 kernel 回 EINVAL → sysrq 備援
    sys::power_off_sysrq();
}

fn mount_opt(out: &mut dyn Write, src: &str, target: &str, fstype: &str, data: &str) {
    if let Err(e) = sys::mount_fs(src, target, fstype, data) {
        say!(out, &format!("QEMU-INIT: mount {target} failed: {e}"));
    }
}

fn mount_9p(out: &mut dyn Write, tag: &str, target: &str) {
    match sys::mount_fs(tag, target, "9p", "trans=virtio,version=9p2000.L") {
        Ok(()) => {
            let name = if target == "/out" { "out" } else { "9p" };
            say!(out, &format!("QEMU-INIT: {name} mounted"));
        }
        Err(e) => say!(out, &format!("QEMU-INIT: 9p mount failed: {e}")),
    }
}

/// rd.test payload：優先 .sh 腳本（本 sh 直譯），否則直接執行二進位。
fn run_payload(id: &str) -> i32 {
    let script = format!("/mnt/tests/{id}.sh");
    let bin = format!("/mnt/tests/{id}");
    if std::path::Path::new(&script).exists() {
        let mut null = std::io::empty();
        let mut so = std::io::stdout();
        let mut se = std::io::stderr();
        return sh::run_script(&script, &mut null, &mut so, &mut se);
    }
    if std::path::Path::new(&bin).exists() {
        let status = std::process::Command::new(&bin).arg("--test-threads=1").status();
        match status {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("QEMU-TEST: spawn failed: {e}");
                1
            }
        }
    } else {
        eprintln!("QEMU-TEST: no payload for {id}");
        1
    }
}

/// 把 9p share 上的 multi-call binary 與其 applet symlink 進 /bin。
fn link_applets(out: &mut dyn Write) {
    if std::path::Path::new("/mnt/bin/coreutils").exists() {
        relink("/mnt/bin/coreutils", "/bin/coreutils");
        if let Ok(list) = std::fs::read_to_string("/mnt/applets.txt") {
            for a in list.lines() {
                if !a.is_empty() {
                    relink("/mnt/bin/coreutils", &format!("/bin/{a}"));
                }
            }
        }
        say!(out, "QEMU-SHELL: coreutils applets linked into /bin");
    }
}

fn relink(target: &str, link: &str) {
    let _ = std::fs::remove_file(link);
    let _ = std::os::unix::fs::symlink(target, link);
}
