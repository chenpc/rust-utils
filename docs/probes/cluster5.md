# cluster5 參考行為探測記錄（install / mkfifo / mknod / dircolors / chcon / mktemp / shred）

本機參考：`/usr/lib/cargo/bin/coreutils/<applet>`（uutils coreutils 0.10.0）。
探測環境：host（umask 002、非 root）；探測目錄 `/tmp/c5`。
依 development-log[48] 策略：man 有描述以 man 為準；man 未指定 → 以參考實測對齊；邊界 quirk 一律記 `docs/assumptions.md`。

## install

- `install SRC DST` rc0；同檔 → rc1 `install: 'SRC' and 'DST' are the same file`
- 無 operand → rc1 `install: missing file operand` + `Try '<path>/install --help' for more information.`
- 只有 SRC → rc1 `install: missing destination file operand after 'SRC'` + Try 行
- `-d` 無 operand → rc1 `install: install with -d requires at least one argument.`
- `-t DIR` 無檔案 → 同 missing file operand
- `-T` + 兩個 SRC → rc1 `install: extra operand 'X'` + 第二行 `install [OPTION]... [FILE]...`（注意：第二行沒有 `Try`）
- 兩 SRC 其中一個不存在 → rc1 `install: cannot stat 'X': No such file or directory`，整體 rc1（已處理的 SRC 仍完成）
- SRC 是目錄 → rc1 `install: omitting directory 'DIR'`
- `-m`：GNU mode 語法（octal / symbolic）。`-m a+x SRC DST` → mode 111（who 省略＝not-u 減 umask，與 GNU 一致）
  - `-m bogus` → rc1 `install: Invalid mode string: invalid operator (expected +, -, or =, but found b)`（**Invalid 大寫**，與 mkdir/mkfifo 措辭不同）
- 預設 mode：755（`-C` 案例 dst 600 → 755）
- `-C/--compare`：内容相同 → **不覆寫内容**（dst 保持原内容）但 **仍套用 mode/owner**；内容不同 → 照常複製（rc0）
- `-p`：atime/mtime 由 SRC 複製（實測雙方 `2001-01-01 08:00:00.000000000 +0800`）
- `-c`：被忽略（rc0）
- `-s`：執行外部 `strip`；失敗 → strip 自身 stderr（例 `strip: X: file format not recognized`）+ rc1 `install: strip program failed: strip process terminated abnormally - exit code: 1`
- `-o USER`：不存在的 user → rc1 `install: invalid user: 'nosuchuser'`；存在但無權 → rc1 `install: failed to chown 'PATH': changing ownership of 'PATH': Operation not permitted (os error 1)`；`-o "$(id -u -n)"` rc0
- `-g GROUP`：不存在 → rc1 `install: invalid group: 'nosuchgrp'`
- `-D`：建立 SRC→DST 所需上層目錄（rc0）
- `-t DIR`：DST 放入 DIR；`-t NONEXISTENT` **rc0**（uutils 會建立該目錄，待確認）
- DST 為目錄 → SRC 放進目錄內（rc0）
- `-d`：遞迴建立；預設 mode 664（umask 002 環境 → 0o777&~umask）；父目錄為普通檔 → rc1 `install: cannot create directory '/tmp/c5/n1/n2': Not a directory`
- `-v`：複製 → stdout `'SRC' -> 'DST'`；覆寫既有 DST → 先 `removed 'DST'` 再 `'SRC' -> 'DST'`；`-d` → `creating directory 'X'`
- `-b`：備份既有 DST，suffix `~`（或 `$SIMPLE_BACKUP_SUFFIX`）；`-S '.XX'` → suffix `.XX` 且**隱含開啟 backup**（無 -b 也產生 `g1.XX`）；`--backup=numbered` → `f3b.~1~`
- `-S '.XX'` 搭配既有檔 → 產生 `X.XX`
- `-h/--help` rc0：首兩行 `Copy SOURCE to DEST or multiple SOURCE(s) to the existing` / `DIRECTORY, while setting permission modes and owner/group`，`Usage: install [OPTION]... [FILE]...`，`Arguments:` / `  [files]...  `
- `--version` → `install (uutils coreutils) 0.10.0`
- `-Z`/`--context`：接受（無 SELinux 時不影響 rc）

## mkfifo

- 預設 mode：`0o666 & ~umask`（umask 002 → 664）
- 多 operand rc0
- `-m 644` rc0；`-m u=rw,g=r,o=` → 640
- `-m bogus` → rc1 `mkfifo: invalid mode: invalid operator (expected +, -, or =, but found b)`
- 已存在 → rc1 `mkfifo: cannot create fifo 'P': File exists`
- 父目錄不存在 → rc1 `mkfifo: cannot create fifo 'P': No such file or directory`
- `-Z` rc0（忽略）
- 無 operand → clap 風格：`error: the following required arguments were not provided:` + 第二行 `  <fifo>...` + `For more information, try '--help'.` rc1

## mknod

- `<NAME> <TYPE> [major] [minor]`；TYPE ∈ b,c,u,p（GNU man）
- `mknod P p` rc0（fifo）；`-m 640 NAME p` → mode 640
- 預設 mode（b/c）：`a=rw - umask`（GNU man 措辭）
- 缺 TYPE → clap 風格 `error: the following required arguments were not provided:` + `  <TYPE>` rc1
- invalid TYPE → `error: invalid value 'x' for '<TYPE>': invalid device type 'x'` + `For more information, try '--help'.` rc1
- 只有 major（`mknod P c 1`）→ rc1 `mknod: Special files require major and minor device numbers.` + `Try '<path>/mknod --help' for more information.`
- major 為 `0x1` → clap `error: invalid value '0x1' for '[major]': invalid digit found in string` rc1（uutils 不接受 hex；GNU man 未寫明 → 記 assumptions，採參考）
- 非 root 建 b/c → `mknod: Operation not permitted (os error 1)`，**exit 255**（uutils quirk，記 assumptions）
- guest 以 root 執行 → 可成功建立 b/c（測試可用）

## dircolors

- 預設輸出 == 解析內嵌 db：`cmp <(dircolors) <(dircolors /tmp/db_default.txt)` 相同 → 實作只需內嵌 db + 解析器
- `dircolors -p` = 200 行內嵌 db（本機存檔 `/tmp/db_default.txt`，無 ESC 位元組；含 `# …` 註解、`COLORTERM ?*`、大量 `TERM <glob>` 行；key 與 value 以空白分隔）
- 預設 LS_COLORS 字串 1767 bytes（本機存檔 `/tmp/lsc_default.txt`）
- 輸出格式：sh（預設 / `-b` / `--sh`）→ `LS_COLORS='…';` + 下一行 `export LS_COLORS`；csh（`-c`）→ `setenv LS_COLORS '…'`（無 export 行）；結尾恒 `:';`（空 db → `LS_COLORS='';`）
- 行解析：跳過 `#` 開頭與空行；以**第一個空白/Tab** 切 key / rest
  - rest 空 → rc1 `dircolors: <file>:<lineno>: invalid line;  missing second token`（**兩個空格**）
  - rest 非空 → key 判定：`*` 或 `.` 開頭 → extension（`.tar`→`*.tar`、`*`→`*=`）；recognised keyword → 短碼；else rc1 `dircolors: unrecognized keyword <KEY>`
  - key 含 `=` 時一律走上述兩規則（uutils 0.10.0 **沒有**「變數忽略」語意）：`VAR=1 2`→`unrecognized keyword VAR=1`；`VAR=1`（無值）→ missing second token；`MYVAR=\tdir\tdi`→`unrecognized keyword MYVAR=`；`*.tar=01;31 extra`→`*.tar=01;31=extra:`；`*.i=vv z`→`*.i=vv=z:`；`LINK=ln\tv`→`unrecognized keyword LINK=ln`；`DI=v x`→`unrecognized keyword DI=v`
- recognised keyword（大小寫不拘）→ 短碼：RESET→rs、DIR→di、LINK/lnk→ln、MULTIHARDLINK→mh、FIFO→pi、SOCK→so、DOOR→do、BLK→bd、CHR→cd、ORPHAN→or、MISSING→mi、SETUID/suid→su、SETGID/sgid→sg、CAPABILITY→ca、STICKY_OTHER_WRITABLE→tw、OTHER_WRITABLE/owr→ow、STICKY→st、EXEC→ex
- 識別但忽略（不產出條目）：COLOR、OPTIONS、EIGHTBIT、TERM、COLORTERM
- **不**識別（→ unrecognized keyword）：directory、socket、other-writable、world-writable、exe、executable、multi、multi-ext、以及短碼本身（di/ln/pi/su/tw/ow/st/ex/mh/mi/ca/ln…）、i、ki、mI、MYVAR
- value 為 rest 原樣，可含空白（`dir\ta b c` → `di=a b c:`）
- TERM/COLORTERM 過濾：value 為 glob（`*`、`?`）比對 `$TERM` / `$COLORTERM`；匹配 → 其後條目納入；不匹配 → 其後條目略過；檔案內無 TERM/COLORTERM 行 → 全部納入（即使環境有 TERM）。條目輸出順序 = 檔案順序。
  - 例：`TERM\tdum*` + TERM=dumb → 其後全納入；TERM=xx → 其後全略過；`TERM\tzzz` → 其後全略過（`dir`（在 TERM 之前）仍納入）
- 兩 operand → rc1 `dircolors: extra operand 'X'` + `Try '<path>/dircolors --help' for more information.`
- 檔案不存在 → rc1 `dircolors: /nonexistent: No such file or directory (os error 2)`（此處帶 `(os error N)`）
- 未知選項 `-o` → clap `error: unexpected argument '-o' found` + `  tip: to pass '-o' as a value, use '-- -o'` + `Usage: dircolors [OPTION]... [FILE]` + `For more information, try '--help'.` rc1
- `--print-ls-colors` 存在（uutils man/EXAMPLES 提及）→ 需探測或實作（次要）
- man（uutils 0.10.0）：`-b, --sh  output Bourne shell code to set LS_COLORS`、`-c, --csh  output C shell code to set LS_COLORS`、`-p, --print-database  print the byte counts`（man 措辭古怪）、`--print-ls-colors  output fully escaped colors for display`

## mktemp

- 預設模板 `tmp.` + 10 個 X → 例 `/tmp/tmp.kitgswpZWl`；隨機字元 = [a-zA-Z0-9]
- 模板 X 少於 3 → rc1 `mktemp: too few X's in template 'XX'`（`mktemp 'plain'`、`mktemp X` 同）
- `mktemp 'XXX'` → rc0，印 3 字隨機
- `--suffix S`：接在隨機替換之後（`-u --suffix=.suf 'aa.XXXX'` → `aa.omk3.suf`）；模板未以 X 結尾 → rc1 `mktemp: with --suffix, template 'wu' must end in X`
- suffix 含路徑分隔 → rc1 `mktemp: invalid suffix '/x', contains directory separator`
- `-t 'sub/dir.XXXXXX'` → rc1 `mktemp: invalid template, 'sub/dir.XXXXXX', contains directory separator`；`-t ex.XXXXXX` → `/tmp/ex.mcKCBT`（TMPDIR 或 /tmp + 模板；deprecated 選項）
- 無 `-p/--tmpdir` 且無 `-t`：模板照原樣（**相對 cwd**）：`TMPDIR=/tmp/c5 mktemp 'tt.XXXXXX'` → `tt.eFQUxP`
- `-p DIR` / `--tmpdir=DIR`：DIR 作為前綴接在模板前面 → `/tmp/c5/qq.4xxIC3`；`mktemp -p /tmp/c5`（無模板）→ `/tmp/c5/tmp.fUFv8GUv3r`；`--tmpdir 'ss.XXXXXX'`（無值）→ TMPDIR 或 /tmp 當 DIR → `/tmp/ss.xRkPr5`
- `-u/--dry-run`：只印名字不建立（`-d -u 'ddir.XXXX'` → 印 `ddir.IUAi`）
- `-d/--directory`：建立目錄
- `-q/--quiet`：失敗時靜默（`-q -p /nonexistent2` rc1 無訊息）
- `--tmpdir` 時 TEMPLATE 不得為絕對路徑（uutils man），TEMPLATE 可含斜線但只建立最後一個元件
- 建立失敗（DIR 不存在）→ rc1（措辭待測；`-q` 靜默）

## shred

- 預設 3 passes；`-v` 每 pass 一行 `shred: F: pass N/M (random)...`
- `-z` → 最後追加 zero pass：`pass 2/2 (000000)...`（檔尾全 0）
- `-x/--exact`：不進位到完整 block（GNU man：非普通檔的預設）
- **size 語意（實測）**：無 `-s` 時 size 進位到 4096 的倍數（1→4096、400→4096、4096→4096、4100→8192、8192→8192）；`-s N`：檔案 < N 則成長到 N（3B→`-s 100`→100；5000B + `-s 100`→5000 不變；`-s 8193`→8193）
- `-n 0` → 完全略過（檔案 content 不變、無 verbose 行、rc0）
- `-n 1 -v -x F` → `pass 1/1 (random)...`；`-n 1 -v -s 5 F`（3B）→ `pass 1/1`；`-n 1 -v big(3003B)` → `pass 1/1`
- `-u/--remove`（或 `--remove=unlink|wipe|wipesync`）verbose 序列（實測）：
  `shred: F: pass 1/1 (random)...` → `shred: F: removing` → `shred: F: renamed to /tmp/c5/000` → `shred: F: renamed to /tmp/c5/00` → `shred: F: renamed to /tmp/c5/0` → `shred: F: removed`（檔名長度 3 的例子；名字長度 N → 先改名為 N 個 `0`，再逐次截短）；檔案確實消失
  `--remove=wipe`（無 -u）同樣執行 removing/renamed/removed
  `--remove=unlink` 只 unlink（實測 `-v --remove=unlink sz3` → 3 passes、無 removing 行）
- 非普通檔 → rc1 `shred: F: Not a file`
- 唯讀檔 → rc1 `shred: F: failed to open for writing: Permission denied`；`-f` 可修（改 mode 後重試）
- 檔案不存在 → rc1 `shred: F: No such file or directory`
- `--random-source=MISSING` → rc1 `shred: MISSING: No such file or directory`
- random-source 太小（4B 供應 4096）→ rc1 `shred: FILE: File write pass failed: failed to fill whole buffer`
- `--random-source=/dev/zero`（4096B）→ 全 0 寫入 rc0
- `-n bogus` → rc1 `shred: invalid number of passes: 'bogus'`；`-s bogus` → rc1 `shred: invalid file size: 'bogus'`
- `--remove=bogus` → clap invalid value rc1（possible values unlink/wipe/wipesync）

## chcon

- 無 SELinux（guest tmpfs / host）→ 一律失敗；測試只斷言 rc 與前綴
- `chcon CTX FILE` → rc1 `chcon: Setting security context failed. setfilecon() failed on path '/proc/self/fd/5'. Operation not permitted (os error 1).`（參考用內部 fd 路徑，**不追**；我方措辭自訂，斷言前綴 `chcon: Setting security context failed`）
- partial context（`-t TYPE` / `-u USER` 等而無完整 CTX positional）→ rc1 `chcon: Applying partial security context to unlabeled file failed on 'FILE'. invalid input parameter.`（`-R` 對每個檔案一行，遞迴包含目錄本身）
- `--reference=/nonexistent` → rc1 `chcon: Getting security context failed. getfilecon() failed on path '/nonexistent'. No such file or directory (os error 2).`
- 無 FILE operand → rc1 `chcon: No files are specified.` + 空行 + `Try '<path>/chcon --help' for more information.`
- `--help` rc0：`Change the SELinux security context of each FILE to CONTEXT.` / `With --reference, change the security context of each FILE to that of RFILE.` / `Usage: chcon [OPTION]... CONTEXT FILE...` / `chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...` / `chcon [OPTION]... --reference=RFILE FILE...`
- `ls -Z` 在無 SELinux 環境顯示 `?`

## 補測（workers 發出後的追加實測，2026-09 收尾輪）

- shred `-n 0 -u FILE` → **仍然執行移除**（無 pass 行，但有 `removing` / `renamed …` / `removed`）；`-n 0` 单独（無 -u）才是完全略過。
- shred 完整 rename 序列（`-v`）：`shred: NAME: removing` → `shred: NAME: renamed to DIR/0…0`（0 的個數 = basename 長度）→ 之後每一行的**主語變成目前的暫存名**、目標逐次少一個 0（`DIR/00000 → DIR/0000 → … → DIR/0`）→ 最後一行主語回到**原名**：`shred: NAME: removed`。例（NAME=abcde）：
  ```
  shred: /tmp/c5/abcde: removing
  shred: /tmp/c5/abcde: renamed to /tmp/c5/00000
  shred: /tmp/c5/00000: renamed to /tmp/c5/0000
  shred: /tmp/c5/0000: renamed to /tmp/c5/000
  shred: /tmp/c5/000: renamed to /tmp/c5/00
  shred: /tmp/c5/00: renamed to /tmp/c5/0
  shred: /tmp/c5/abcde: removed
  ```
- `--remove=wipe`（無 -u）與 `-u` 輸出相同。
- install `-v -t DIR` 覆寫既有檔 → `removed 'DIR/NAME'` 然後 `'SRC' -> 'DIR/NAME'`（DST 為目錄時 removed 行帶完整路徑）。
- install `-t NONEXISTENT SRC` → rc0 且**建立該目錄**（uutils 行為）。
- dircolors 未知選項完整 stderr：`error: unexpected argument '-o' found` + 空行 + `  tip: to pass '-o' as a value, use '-- -o'` +（_usage_ 空行 + `Usage: dircolors [OPTION]... [FILE]` + 空行 + `For more information, try '--help'.`）。
- dircolors `--print-ls-colors` 輸出格式：每條 `<ESC>[<value>m<KEY><TAB><value><ESC>[0m`，例如 `[0mrs\t0[0m`、`[01;34mdi\t01;34[0m`（ESC 為 0x1b 原位元組）。
