#!/usr/bin/env bash
# =============================================================================
# scripts/build_initramfs.sh — 組 guest 的 initramfs（cpio.gz）
#
#   /bin/busybox        multi-call
#   /init               mount proc/sys/dev → insmod 9p 模組 → mount 9p share → 跑 payload
#   /modules/*.ko       從 .guest-env/modules 複製（已解 zstd）
#
# payload 放在 9p share（guest 內的 /mnt），不在 initramfs 裡，這樣改測試內容不用重組 initramfs。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
source "$(dirname "$_self")/common.sh"

[ -f "$GUEST_ENV_ROOT/guest.env" ] && . "$GUEST_ENV_ROOT/guest.env"
BUSYBOX="$GUEST_ENV_ROOT/root/usr/bin/busybox"
[ -x "$BUSYBOX" ] || { rust_log "[initramfs] 找不到 busybox（先跑 scripts/install_guest.sh）"; exit 1; }

STAGE="$GUEST_ENV_ROOT/stage"
OUT="$GUEST_ENV_ROOT/initramfs.cpio.gz"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/modules" "$STAGE/proc" "$STAGE/sys" "$STAGE/dev" "$STAGE/tmp" "$STAGE/mnt"

cp "$BUSYBOX" "$STAGE/bin/busybox"
for a in sh mount insmod echo cat sed mkdir ls cp ln poweroff; do
  ln -sf busybox "$STAGE/bin/$a"
done

# busybox 是動態連結，initramfs 裡必須自帶 runtime（ld.so + libc + libresolv）
mkdir -p "$STAGE/lib64" "$STAGE/lib/x86_64-linux-gnu"
LD_LIBRARY_PATH="$GUEST_ENV_ROOT/root/usr/lib/x86_64-linux-gnu" ldd "$BUSYBOX" 2>/dev/null | awk '{ for (i=1;i<=NF;i++) if ($i ~ /^\//) { print $i; break } }' | while read -r p; do
  cp "$p" "$STAGE/lib/x86_64-linux-gnu/"
done
for f in "$GUEST_ENV_ROOT/root"/lib64/ld-linux-x86-64.so.2 "$GUEST_ENV_ROOT/root"/usr/lib64/ld-linux-x86-64.so.2; do
  [ -e "$f" ] && { mkdir -p "$STAGE/lib64"; cp "$f" "$STAGE/lib64/"; break; }
done

for f in "$GUEST_ENV_ROOT/modules"/*.ko; do
  [ -e "$f" ] || continue
  cp "$f" "$STAGE/modules/"
done

cat > "$STAGE/init" << 'INIT_EOF'
#!/bin/busybox sh

mount -t proc     proc     /proc
mount -t sysfs    sysfs    /sys
mount -t devtmpfs devtmpfs /dev

# 9p 在此 kernel 是模組不是內建，且 9pnet 依賴 netfs，所以順序固定。
for m in netfs 9pnet 9p 9pnet_virtio; do
  insmod "/modules/$m.ko" 2>/dev/null || echo "QEMU-INIT: insmod $m failed"
done

mkdir -p /mnt
if mount -t 9p -o trans=virtio,version=9p2000.L hostshare /mnt; then
  echo "QEMU-INIT: 9p mounted"
else
  echo "QEMU-INIT: 9p mount failed"
fi

# 可寫 share：guest 產出的檔案（profraw、測試結果）寫到 /out → host 的 guest-share/out
mkdir -p /out
if mount -t 9p -o trans=virtio,version=9p2000.L hostout /out; then
  echo "QEMU-INIT: out mounted"
else
  echo "QEMU-INIT: out mount failed"
fi

test_id="$(sed -n 's/.*rd\.test=\([^ ]*\).*/\1/p' /proc/cmdline)"
if [ -n "$test_id" ]; then
  rc=1
  if [ -f "/mnt/tests/$test_id.sh" ]; then
    /bin/busybox sh "/mnt/tests/$test_id.sh"; rc=$?
  elif [ -x "/mnt/tests/$test_id" ]; then
    "/mnt/tests/$test_id"; rc=$?
  else
    echo "QEMU-TEST: no payload for $test_id"
    poweroff -f
  fi
  if [ "$rc" = 0 ]; then echo "QEMU-TEST: PASS"; else echo "QEMU-TEST: FAIL (exit $rc)"; fi
  poweroff -f
fi

# rd.shell=1 → 互動 shell（run-shell.sh 用的路徑）
case "$(cat /proc/cmdline)" in
  *rd.shell=1*)
    if [ -x /mnt/bin/coreutils ]; then
      ln -sf /mnt/bin/coreutils /bin/coreutils
      if [ -f /mnt/applets.txt ]; then
        while read -r a; do
          [ -n "$a" ] && ln -sf /mnt/bin/coreutils "/bin/$a"
        done < /mnt/applets.txt
      fi
      echo "QEMU-SHELL: coreutils applets linked into /bin"
    fi
    echo "QEMU-SHELL: ready（exit 關機；互動模式可用 Ctrl-a c 進 monitor）"
    cmd_id="$(sed -n 's/.*rd\.cmd=\([^ ]*\).*/\1/p' /proc/cmdline)"
    if [ -n "$cmd_id" ]; then
      # -c 模式：stdin 給 /dev/null，避免 applet 讀 stdin 時卡在 console（要輸入就用管線或檔案）
      [ -f "/mnt/$cmd_id.sh" ] && /bin/busybox sh "/mnt/$cmd_id.sh" < /dev/null
    else
      /bin/busybox sh
    fi
    echo "QEMU-SHELL: shell exited"
    ;;
esac

poweroff -f
INIT_EOF
chmod +x "$STAGE/init"

( cd "$STAGE" && find . | sed 's|^\./||' | cpio -o -H newc 2>/dev/null | gzip -9 > "$OUT" )
rust_log "[initramfs] → $OUT ($(du -h "$OUT" | cut -f1))"
exit 0
