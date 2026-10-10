# 行為假設（man page 未定義之處）

規則 1 規定行為模型只能從 man page 推導。man page 沒寫到的行為視為未定義，
這裡記錄實作時所做的假設；每條都要有可重現的測試斷言。

| applet | 未定義之處 | 假設 | 測試 |
|---|---|---|---|
| `echo` | 未知選項（`-x`） | 視為一般 STRING；只有字元全為 `n`/`e`/`E` 的 `-xxx` 才當選項解析 | `tests/echo.rs::unknown_flag_is_treated_as_string` |
| `echo` | `\x` 後面无 hex digit | 保留字面 `x` | `tests/echo.rs::hex_escape_accepts_1_to_2_digits` |
| `echo` | `\` 後面不是已知 escape | 保留 `\` 與該字元 | `tests/echo.rs::named_escapes_are_expanded`（`\\`） |
| `yes` | 寫入失敗時的 exit status | 回傳 1 | `tests/yes.rs::default_string_is_y` |
| `true` / `false` | `--help` / `--version` 的文字內容 | 實作自訂最小文字，測試只斷言含 applet 名稱 | `tests/true.rs::help_and_version_print_text`、`tests/false.rs::help_still_returns_1` |
| 全部 | 寫入錯誤（EPIPE 等）的 exit status | 回傳 1 | `tests/cat.rs::write_error_on_stdin_returns_1` |

## sort (src/applets/sort.rs)

- man 同時把 `-h` 列為 `--help` 與 `--human-numeric-sort`。假設：`-h` 是 human-numeric
  排序模式，`--help` 才是印幫助；`-V` 是 version 排序，`--version` 印版本。
- `-R`（random）必須可重現才能測試：用 FNV-1a hash 每行決定順序（相同輸入 → 相同輸出）。
- `-m`（merge）假設輸入已排序；實作等價於對全部輸入做 stable sort。
- `-c` 預設行為取 man 的 `diagnose-first`：把第一個亂序行印到 stderr 並 exit 1；`-C` 靜默。
- `-u` 依排序鍵判斷相等；`-s` 時（無 last-resort 比較）依整行判斷。
- `--parallel`、`-S`、`-T`、`--compress-program`、`--batch-size`、`--files0-from`、`--debug`
  被接受但不改變記憶體中的排序演算法（`--files0-from` 需要额外讀檔，目前不支援）。
- `-k` 的 CHAR 為 1 起始；start CHAR 預設 1，end CHAR 預設 0（= 該欄結尾）；逗號後沒有 end
  表示到整行結尾。key options `MbdfhnRrV` 覆寫全域選項。
- 預設欄位分隔：非空白後的第一個空白；空白屬於欄位的一部分（man: "whitespace is appended at
  the beginning of each field"）；用 `-t` 時分隔字元不算入欄位。

## basenc / Z85

- The man page does not list the Z85 alphabet. Use the RFC 32 (ZeroMQ Base-85)
  alphabet: `0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?&<>()[]{}@%$#`
  (85 chars). Source: https://rfc.zeromq.org/spec/32/ (recorded 2026-10-09).
- Base58 alphabet (man page also silent): Bitcoin base58
  `123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz`.
- Base2 alphabets are `01`; base16 uses uppercase hex.
- Encoding selector may appear anywhere in the arguments; the last one wins.

## uniq -c 計數欄寬

- GNU `uniq -c` 每一行的計數都用 `%7d`（右對齊、寬 7）。本實作照 GNU。
- 觀察：uutils coreutils 0.10.0 只補齊**第一**行，後續行直接印計數（`printf 'a\nb\nc\n' | uniq -c`
  → `1 a` / `      1 b` / `      1 c`）。依「man page 衝突時以 GNU 為主」的規則，保留全部補齊（記錄 2026-10-09）。

## hashsum 家族：check mode（GNU 行為，man page 未定義）

實測參考 `/usr/bin/coreutils`（uutils 0.10.0）後採 GNU 行為：

- `cksum -c` 讀 tagged 清單時**逐行**由名稱推導演算法，不受 applet 預設影響：
  `MD5 (f) = hex` → 用 MD5 → `f: OK`。只有「明確指定」數值型演算法（`-a crc -c`）才報
  `--check is not supported with --algorithm={bsd,sysv,crc,crc32b}`（exit 1）。
- 缺失檔案：stderr `applet: file: No such file or directory`，同時 stdout `file: FAILED open or read`。
- 摘要警告（stderr）順序：`WARNING: N line is improperly formatted` →
  `WARNING: N computed checksum did NOT match` → `WARNING: N listed file could not be read`；
  單數用 `line is` / `listed file`，複數用 `lines are` / `listed files`。
- `--status` 抑制摘要警告與 `OK`，但**不**抑制 `FAILED open or read`；`--quiet` 只抑制 `OK`。
- `--ignore-missing` 且完全沒有驗證任何檔 → `applet: <list>: no file was verified`（exit 1）。
- 整份清單都沒有可解析的行 → `applet: <list>: no properly formatted checksum lines found`（exit 1），
  此時不再印摘要警告。

## hashsum 家族：man page 與參考 binary 的落差

- Debian `rust-coreutils` man page mirror 對 `sha3sum`、`sha3-224sum`、`sha3-256sum`、`sha3-384sum`、
  `sha3-512sum`、`shake128sum`、`shake256sum` 回 HTTP 200 但 gzip 內容為空（37 bytes，原始 size 0）；
  `b3sum` 與 `sm3sum` 是 404。只有 `rust-hashsum.1.en.gz` 有內容 → 這些 applet 的行為模型由
  hashsum man page（uutils 0.0.30）+ 本地 md5sum/b2sum man page 推導。
- 本機安裝的 uutils 0.10.0 binary **沒有**註冊這些 applet（`coreutils hashsum` → `unknown program`），
  透過 `/usr/lib/cargo/bin/coreutils/<applet>` 符號連結跑時全部 behave like `sum`（BSD）。
  所以這些 applet 的 e2e test 斷言的是 man-page 推導行為，不是參考輸出。
- `hashsum` 的預設演算法 man page 沒寫 → 視為未定義，未指定 selector 時 `hashsum: no algorithm specified`（exit 1）。
- `--debug`：參考印硬體偵測行（`avx512 support not detected` 等），本實作印 `<applet>: no hardware acceleration detected`。
- `cksum` 預設 `crc` 是實作決定：本實作 poly `0x04c11db7`（MSB-first）、init 0、無 final xor；
  參考 `hello world\n` = 3733384285，本實作 = 3096396776，任何 init/xor/reflection 組合都重現不了。

## C 群路徑家族（pathchk/readlink/realpath/relpath）

- 本機參考實作為 uutils coreutils 0.10.0（`/usr/lib/cargo/bin/coreutils/<applet>`）。
- `relpath`：參考 binary **不提供**（`unknown program 'relpath'`），docs/man mirror 亦無
  `relpath.1.gz`。行為依 `docs/gnu_info/group_c_realpath_family.txt`（GNU coreutils 文件）實作：
  `relpath [-s] [-p] DEST [BASE]`，BASE 預設 `.`；預設純詞法（相對運算元先以 cwd 補絕對，
  不碰檔案系統）；`-s` 兩邊必須絕對路徑；`-p` 兩邊先解析 symlink（皆須存在）。
  錯誤訊息措辭為自訂（`relpath: both paths must be absolute: 'x', 'y'`、
  `relpath: extra operand 'c'`）。
- `realpath -q` quirk（參考實測）：quiet 不只抑制診斷，**exit code 也歸 0**
  （`realpath -q -e <dangling>` → rc 0）。本實跟隨。
- `pathchk` 訊息細節（參考實測）：預設模式總長 >4096 → `limit 4096 exceeded by length N of
  file name '<name>'`（有引號）；`-p` 總長 >256 → `limit 256 ... file name <name>`（**無**引號）；
  component >14 只報第一個；`-p` 先驗總長再驗 component；`-P` 的空名稱走 stat ENOENT 分支
  （`pathchk: '': No such file or directory`），只有 `-p` 才報 `empty file name`。
- 名稱檢查不因**尾斜線**報錯（`realpath /a/b/` 對 regular file 也照樣輸出，參考實測 rc 0）。
- `--relative-base` 語意（參考實測）：DEST 在 BASE 之下 → 輸出**相對**路徑（有 `--relative-to`
  時以其為基準）；不在 BASE 之下 → 輸出**絕對**路徑。基準不存在時依詞法解析。
- clap 專屬措辭（unknown option、重複選項、missing operand 的 `Try '...'` 行）不追隨參考，
  採 GNU 风格（`applet: invalid option -- 'x'`，exit 2）。

## F 群磁碟／系統（df/du/sync/dd）

- **df 去重用 st_dev 而非 f_fsid**：guest 內 /mnt 與 /out 兩個 9p 掛載的 `f_fsid` 相同，若以 fsid 去重會漏掉一個掛載點；GNU 以 mount 點的 `st_dev` 為去重鍵，實作相同（`sys::stat_dev`）。
- **df 表頭單位縮寫**：非 `-P` 時表頭用 `label_from_size(unit)`：1024 鏈整除 → `1K`/`4K`/`1M`（不附 B）、1000 鏈 → `1kB`（附 B）、其他 → SI human（1536 → `1.6kB`）；`-P` 一律 "{unit}-blocks"。`-P` 會忽略 DF_BLOCK_SIZE/BLOCK_SIZE/BLOCKSIZE 環境變數。
- **df 的 autofs 行永遠略過**（如 systemd-1 的 binfmt_misc autofs 條目），`-a` 也不列；PSEUDO 清單只在非 `-a` 時略過。
- **df `-B bogus` rc2**（GNU 風格 "df: invalid block size 'bogus'"），參考 uutils 回 rc1 措辭不同；clap 措辭（`-v`、`--bogus` rc1）不追隨。
- **dd 不支援 `conv=ascii/ebcdic/ibm/sparse`**：回 "dd: conversion 'x' is not supported in this build" rc1。EBCDIC 表與 sparse 輸出超出本專案範圍假設。
- **dd 速率欄只斷言結構**：參考 uutils 0.10.0 的速率數值不穩定（4B → "4.0 kB/s"、3072B → "42.1 MB/s"，計時 bug），測試以 regex 斷言 "copied, <t> s, <rate>" 結構。
- **dd conv=sync 不補滿 count**：EOF 即停（uutils 行為），與 GNU（補 NUL 至 count 滿）不同；短讀補 NUL 後記錄計數仍屬 partial（0+1）。
- **dd conv=block/unblock 依 uutils 串流語意**：block 以 '\n' 切線、每線去 \n 後 len>cbs 截斷+truncated++、否則補空格至 cbs，EOF 殘線也 flush；unblock 以 cbs 切塊、去尾空格、加 \n。刻意不追 uutils 在 ibs 與 cbs 交錯時的記錄對齊細節。
- **dd conv=noerror 同時套用於讀與寫錯誤**（write_fail 在 noerror 時不設 rc=1）。
- **dd `conv=block` 且 ibs<cbs 時參考只輸出第一筆記錄**（uutils bug，"ab\ncd\n" ibs=3 cbs=4 → 參考僅 2 bytes）；我方依串流線模型完整輸出，此案例刻意不追隨。
- **du 目錄 own = lstat st_size**（`-b` apparent 下 tmpfs 目錄 st_size 非 0，為 60/140 等動態值）；測試用即時 stat 計算預期而非硬編 0。
- **du `--time=birth/creation` 以 ctime 近似**：無 statx 系統呼叫，birth time 無法取得（假設記錄於此）；`--time=UNKNOWN_WORD` 退回 mtime（uutils 行為）。
- **du 刻意不追的 uutils quirk**（host 探測 /tmp/dg）：(1) `-A` 對 tmpfs 空目錄 own 顯示 0（我方 GNU ceil → 1）；(2) `-S` 對 ext4 目錄顯示 subtree 而非 own（我方 GNU own）；(3) `--exclude='{f1,sparse}'` 參考只排除 f1（我方兩者皆排除）；(4) `-t` 與目錄互動在 uutils 有不明總值異常。
- **sync 對非普通檔/目錄跳過 fsync**：`sync /dev/null` 參考 rc=0（fsync 會 EINVAL），實作以 metadata is_file||is_dir 判斷後直接成功；`-f` 與 `-d` 併用自訂衝突訊息 rc1（參考 clap conflict 措辭不追隨）。

## D 群叢集 1（mkdir/rmdir/ln/link/unlink/truncate）
- 參考 uutils 0.10.0 錯誤措辭不一致：`mkdir` 僅 EEXIST 帶 operand 前綴（`mkdir: a: File exists`），其他錯誤不帶（`mkdir: No such file or directory`）。本實按 raw_os_error()==17 分流追隨。
- `mkdir -m` 不補 chmod：只靠 mkdir(2) mode 參數，最終 mode 受 umask 與檔案系統 nosuid 影響（host tmpfs `-m 7775` → 1775，與參考一致）。
- `truncate` 不支援 `/`（百分比取整）與 `%` 運算元（參考實測 `Invalid number: '20%'`）；支援 `0x10` hex 與 K/k/…/Y、KB/… 單位。
- `ln -rs` 以 cwd 詞法化後取 `pathname::relative_from`；`--relative` 未搭 `-s` → 自訂 rc2 措辭（參考 clap requires rc1，不追隨措辭）。
- 缺 operand／clap usage 措辭（ln/link/rmdir/truncate）一律自訂 rc2，測試只斷言非零與我方字串。

## D 群叢集 2（stat/chmod/chown/chgrp/touch）
- `stat` 預設**不追隨** symlink（uutils 行徑；GNU 預設追隨）。以 man page 無法區分，追隨參考 binary 實測。
- `stat` 實作用 statx(2)（syscall 332）；错误措辭 `cannot statx '…'`。`%t`/`%T`（檔案系統 magic/type）在檔案模式下一律印 `0`（參考未做 statfs）；`%B` 固定 512；`%H` 印 `?`；`-f --format` 的 `%p` 印 `?`（參考未實作）。
- `stat -f` 為布林（不吃參數）；FS 型別名稱查 magic 表，ext4(0xEF53) 顯示 `ext2/ext3`（參考命名）。Device 欄為十進位 `major,minor`（GNU 為 hex）。
- `chmod` 省略 who 時 `perms & ~umask`（umask 於行程內即時探測）；裸 `t`/`s` 被參考拒絕 → 本實亦拒絕（GNU 接受）。`-f` 靜默但 exit code 仍 1（GNU 會歸 0）→ 不追 GNU。
- `chown`/`chgrp`：`--reference` 存在時所有 positional 均視為 FILE（clap 語意，wrapper 層模擬）；參考取得 reference 失敗 → `failed to get attributes of '<f>': <reason>`。
- `chown -R` 時 top-level operand 錯誤也不帶 `(os error N)` 後綴（參考 quirk，非 -R 才帶）。
- `touch -t` 為**本地時區**語意驗證通過（TZ=+0800 下 `-t` 與 `-d ISO` 同刻）；不支援自然語言日期（`next friday` 參考可解析）→ 記為限制。
- `touch -r <missing>` → `touch: failed to get attributes of '<rf>': …` rc1（不逐 operand 報）。

## D 群叢集 3（rm/mv/cp）
- **cp FIFO/特殊檔重建改用 libc `mknod()`**：本環境（host qemu 兩側）`syscall(253 /*mknodat*/)` 回傳 3 且 errno=0（getpid via `syscall(39)` 正常），不可靠；改 `extern "C" mknod`（src/sys.rs）。
- `cp --preserve=LIST` 為**取代**而非累加（GNU）：`--preserve=timestamps` 不保 mode；裸 `--preserve` 語意為 all，且可選參數只經 `=` 表單（不吃下一參數）。
- `--interactive=never|always|prompt` 三支 applet 皆實作（GNU man 有描述 → 依策略以 man 為準；參考 uutils clap 拒絕帶值形式，措辭不追隨）。非法值 → `invalid argument 'X' for '--interactive'` rc2。
- `cp` 覆寫既有目的地不 unlink：原地截斷寫入、**保留既有 mode**（參考 uutils 實測）。
- `cp -l` 對非普通檔（字元裝置）rc1；一般 hard link 失敗措辭與參考同格式：`cannot create hard link '<dst>' to '<src>': <reason>`。
- `cp --parents`：destination 必須是目錄，否則 `cp: with --parents, the destination must be a directory` rc1；絕對路徑 operand 在 dst 下重建完整絕對路徑（含 `-t` 合併時）。
- `cp` 對目錄 operand 而無 `-R/-a`：`cp: -r not specified; omitting directory` rc1。
- `cp` cluster 短選項缺附值（`-vt X`）→ `cp: option requires an argument -- 't'` rc2（不吞下一參數，GNU 吞；測試斷言我方措辭）。cluster 內 `-T` 已補（修 `invalid option -- 'T'` bug）。
- `mv` cluster 的 `-S`/`-t` 在 cluster 內**吞下一參數**（`mv -fS f1 b` → f1 被吃為 suffix → 缺 destination rc1，GNU 同）。
- `mv -T` 目的地為既存非空目錄 → rc1；`-fT` 空目錄 → 先 rmdir 再 rename 成功。
- `mv -u` 以 mtime_ns 比較（相等即略過）；同 clock tick 寫入的兩檔 `-u` 會略過 — 測試需顯式 `set_times` 分離 mtime。
- 修兩個真 bug：cp `make_symlink` 失敗原不回傳 Err（rc 誤 0）；cp cluster 缺 'T' arm。
- 假設記錄：`docs/assumptions.md` 本節；checklist #71-73 ☑。
