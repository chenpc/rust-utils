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

## D 群叢集 4（ls/dir/vdir）
- 策略依 log[48]：man 有描述以 man 為準；man 未指定處以本機參考 binary（uutils coreutils 0.10.0，`/usr/lib/cargo/bin/coreutils/ls`）實測對齊。pair-diff 殘留項列於此，e2e 測試改抓 man-derivable 不變量。
- **多欄（-C/-x/--format=horizontal）欄數選擇**：以「最小列數 r0 使 k=ceil(n/r0) 欄可容納（欄寬=欄內 max＋2、末欄不補）」實作，多數 sweep 吻合（lw 14 檔 w7..30、un11 w20..40、lg w20..120）；但參考在部分組合（lg `-x`/`--format=horizontal` w80、`-w 12 -C`、`-T 4 -C`）選出更多欄或不同 gap，且含 tab 檔名時補白 tab-stop 位置差一個空格 → 無法由單一公式重現，不追 byte-exact。行內最後一個實際項目之後不補空白（已修）。
- **sort ties**：參考對同 mtime／同 size 的檔案不依名稱排序（readdir/實作內部順序，不可重現）。本實採 `sort_by`（unstable）＋大小/時間主鍵；e2e 只斷言主鍵單調與集合相同。`-t`/`-S`/`--sort=time`/`--sort=size` 的 tie 順序差異入此假設。
- **tmpfs 的 `..` 行欄位**：`/tmp` 為 tmpfs 時 `ls -la` 的 `..` 行 st_size（如 3300）大於列表最大 size，參考將 owner 欄 pad（`root   `）但 group 後單空格、大小欄寬取 `max(sw, 自身寬)`；其餘 tmpfs 行是否 pad 依 st_size 而變（lg/emptyd 案例不一致）→ 以「`..` 行 owner pad gw+1、大小欄 per-row max」近似，`-la` 其餘 gap 差異不追。`..` 的 st_size 不參與 sw 計算。
- **-m（commas）斷行**：名稱區段與分隔詞 `", "` 的數寬規則自相矛盾（lg -w80 斷於 `n13,`@78、-w40 斷於 `f1,`@34、-w60 斷於 `n1,`@50；brk fixture 斷於 cur+next==width）。採 `cur+2 >= width` 近似，斷行位置差異入此假設。
- **humanize（-h/--si）**：定案模型為 v<10 → 2 位有效 ceil（整數也保留 `.0`：1024→1.0K、2048→2.0K）；10 ≤ v < base → 3 位有效 ceil 整數（10.09K→11K、976.6K→977K）；ceil ≥ base → 進一位（999999 --si → 1000k）。34 sizes sweep 中 bin 33/34、si 33/34 吻合；邊界 quirk（16385→17K、1048575→1024K、1049600→1.1M、999999 --si→1000k vs 1.0M）顯示參考在 base² 附近切換 1024/1000 基準，無法統一 → 不追 byte-exact。
- **total 行**：僅「目錄內容列表」或 `-s` 時出現；數值 = Σ st_blocks / 2。`-h/--si` 只 humanize total（參考以 blocks*512 為基），個別檔案大小欄仍是 st_size 原值。
- **-ls blocks 欄**：自然寬＋單空格（`4 -rw-rw-r--`），非固定 3 欄右對齊（參考實測）。
- **-D（dired）**：名稱後 ASCII `//DIRED// a b …` 與 `//DIRED-OPTIONS// --quoting-style=<style>`；offset 為名稱於整個輸出 byte 流的 0-based 起點與 exclusive 終點；`-b/-Q` → `C`、`-q/-N` → `literal`、`--quoting-style=shell` → `shell-show-control`、`shell-escape` → `shell-escape-show-control`。
- **--hyperlink**：`--hyperlink=always` 以 OSC8 `file://<hostname><path>` 包裹名稱（`-l` 的 symlink target 不包裹、URI 用 resolved 路徑）；never/auto（非 tty）不輸出。參考 `-i` 模式 URI 路徑截断為 uutils quirk，不追隨。
- **色彩**：`--color` 只實作「有/無色彩」層級與預設 LS_COLORS 表；參考的 `orphan`/`missing` 等細部 palette 差異不追（guest 無色彩終端，測試斷言無 escape 序列）。
- **--time-style=off** 參考 rc0 正常列表（先前誤記為 uutils 錯誤措辭，已移除自創錯誤）；`--time-style=iso` 在 long 格式留下尾空格（參考 quirk，追隨）。
- 錯誤措辭：`ls: cannot access 'X': No such file or directory`（無 `(os error N)` 後綴）、`ls: cannot open directory 'X': Permission denied`，rc2。
- `-k` 為 no-op（參考實測）；`--absolute-paths` 參考不支援（clap unexpected argument）。
- **`-g`/`-o` 語意採 GNU man**（`-g`＝long 省略 owner、`-o`＝long 省略 group）；本機參考 uutils 0.10.0 兩者都是「省略 group」，屬政策 (1) 覆盖 (2)。
- **`--hyperlink` 為可選值旗標**：裸 `--hyperlink`＝always、不消耗下一個 operand（GNU man SYNOPSIS 無值）；`--hyperlink=always|auto|never` 亦可。`-l`/`-D` 的名稱欄在 always 時也包 OSC8（參考 `-l` 實測）。
- **auto（`--color`/`--hyperlink`）在本專案一律視為非 tty＝never**：applet 的輸出是 `dyn Write`（guest 測試寫入記憶體緩衝、無法以 fd 判定終端）；`--color=always`/`--hyperlink=always` 仍強制啟用。
- **`--time` 的合法值只有 `atime,ctime,mtime,birth`**（參考 clap possible values；非法值 rc1）。
- **數值選項的 equals 形式**（`--width=50`、`--tabsize=4`、`--block-size=1K`）追隨參考 rc0；非法值措辭 `ls: invalid line width: 'X'`／`invalid tab size: 'X'`／`invalid --block-size argument 'X'` rc2。
- **選項缺值措辭採參考（clap）**：`error: a value is required for '--format <format>' but none was supplied`＋possible values＋`For more information, try '--help'.`，rc1；未知短旗標則採 GNU man 風格 `ls: invalid option -- '~'`（rc2）。
- **`--format` 亦接受 time-style 名稱** `iso/long-iso/full-iso/verbatim`（GNU man `--format=LONG` 段落含這些值；參考不支援 → 政策 (1) 覆盖 (2)）。
- **`--quoting-style` 合法值**：literal, locale, shell, shell-always, shell-escape, shell-escape-always, clocale, c, escape（GNU man 無 `backslash`）。
- **`--classify` 長形式**依 GNU man 對齊 `-F`；`--file-type` 為不加 `*` 的 classify。
- **`-R` 遞迴时每層目錄都有 `name:` 標題**（單 operand 也加，參考實測）。
- **`-a`/`-A`**：`-a` 含 `.`/`..`（以 `.hidden` 慣例與 `.`/`..` 存在性為不變量）。

## seq（src/applets/seq.rs）

依據：`seq(1)` man 描述為準；man 未指定處以本機參考（uutils coreutils 0.10.0，`/usr/lib/cargo/bin/coreutils/seq`）實測對齊。黑箱 pair-diff 電池 295 例全綠（`~/tools/seqalign.py`）。

- **計數採字面十進位精確算**：count = floor((LAST−FIRST)/INCREMENT)，以三者共 scale 的整數計算（非 f64 累加）。實測佐證：`0 0.1 0.3` → 0.0 0.1 0.2 0.3；`1 1 3.9999999999999996` → 1 2 3；`1 1 3.999999999999999` → 1 2 3（純 f64 累加或 ULP 容差都無法同時成立）。
- **小數位 precision = max(decimals(FIRST), decimals(INCREMENT))**，LAST 不貢獻（`9.1 9.14` → precision 0 → `9`）；decimals 取自字面量的最小十進位展開（`1e-5`→5、`1.5e-2`→3、`1e100`→0）。
- **`-w` 寬度 = max(int_field(渲染後 FIRST), int_field(渲染後 LAST), int_field(渲染後 INCREMENT))}`**，三個 operand 即使不被印出也計入（`-w 3 100 5` → `003`、`-w 9.999 10.001` → `09.999`）；補零位在符號之後、整數位之前；hex operand 不貢獻寬度（`-w 0x10` → 1..16 不補零，參考實測）。
- **hex operand 探 C `strtod` 語意**：`0x10`→16、`0x1.8`→1.5、`0x1.8p2`→6（`0b`/`0o` 前綴拒絕）。含小數部分的 hex 其值以 C `%g` 風格渲染（去尾零、必要時科學記號）：`1.5 0.25 0x2.8` → `1.5 1.75 2 2.25 2.5`。
- **clap positional 語意**：出現第一個 positional 之後，後續 token（含 `--help`、`-V`、`--`、旗標本身）一律落地成 positional 值；累積超過 3 個時為 `error: unexpected value '<最後值>' for '[numbers]...' found; no more were expected`＋空行＋三行 Usage＋空行＋`For more information, try '--help'.`（佐證：`1 2 3 -s x`→'x'、`1 2 3 -w`→'-w'、`1 2 3 --`→'--'、`1 3 -s,`→','）。help/version 只在尚未出現 positional 時立即中斷解析（`-h 1 2 3 4` → help、`-Vh` → version）。
- **選項值不得是「已知選項」**：`-s -w 3` → `error: a value is required for '--separator <separator>' but none was supplied`；但接受未知 hyphen token（`-s -x 1 3` → separator `-x`）。
- **`-f` 格式驗證**：只接受 `e E f F g G`（長度修飾符 `hh h ll l L j z t` 可前置）。非浮點轉換（`d i o u x X c s b q *`）→ `seq: wrong % directive type was given`；其他（`n p z t j h l L C D U O`、`%10`、`%-`）→ `seq: <前兩字元>: invalid conversion specification`（`%10` 報 `%1`）。`%%` 是跳脫、不计入 directive 數；無 directive → `format '<spec>' has no % directive`；結尾孤立 `%` → `format '<spec>' ends in %`（已有一個 directive 時改報 `has too many % directives`）。所有 `-f` 格式錯誤一律不附 `Try '<path> --help' for more information.` 行。
- **錯誤措辭採參考**：`seq: invalid floating point argument: 'a'`、`seq: invalid 'not-a-number' argument: 'nan'`、`seq: invalid Zero increment value: '0'`（大寫 Z）、`seq: missing operand`（皆附 Try 行，唯 `-f` 格式錯誤不附）。
- **負零**：FIRST 為 `-0` 時保留負號（`seq -0 3` → `-0 1 2 3`）。
- **已知偏差 (1)：`%a`/`%A` 不支援**。參考接受 C99 hex float（`-f %a 1 0.5 3` → `0x8p-3 0xcp-3`；`-f %a 1e100 1e1200 1e100` 正規化為單一 hex 整數位且 p ≡ 1 (mod 4)）；man 僅列 e/E/f/F/g/G，本實作回報 `invalid conversion specification`。
- **已知偏差 (2)：`.5` 的舍入方向**。`-f %.6g 100000.5 1 100002` → 參考 `100001 100002`（half-away），本實作 `100000 100002`（half-to-even，與先前 C/glibc `%.1g`、`%+.3g` 實測一致）。
- **已知偏差 (3)：極端量位的多 operand run**。當字面量超出 f64（如 `seq 1e1200 1e1200 1e1201`），參考改用科學記號渲染（`1e+1200 2e+1200 …`），而單 operand（`seq 1e1200 1e1200`）又輸出完整展開——參考自身不一致；本實作一律完整十進位展開（數值相同）。`-f` 指定格式時兩者一致（`-f %g 1e1200 …` → `1e+1200`）。
- **無限 operand**：`seq 1 1 inf` 兩邊都是無限輸出（測試不涵蓋）；`seq inf 1 2` 兩邊皆空輸出 rc0。

## D 群叢集 5：install / mkfifo / mknod / dircolors / chcon / mktemp / shred

依據：`docs/man/<applet>.1.gz`（GNU）為準；man 未載的措辭與邊界以本機參考（uutils coreutils 0.10.0，`/usr/lib/cargo/bin/coreutils/<applet>`，須以完整路徑執行——該目錄本身不可執行）實測對齊。探測記錄見 `docs/probes/cluster5.md`。

### install（src/applets/install.rs）
- verbose 措辭（man 未載，採參考逐字）：`-d -v` 每個 operand 只印一筆 `creating directory '<operand 原樣>'`（不逐層；目錄已存在則不印）；`-D -v` 印 `install: creating directory '<DEST 的父目錄>'`（帶 `install: ` 前綴，且只一筆），其後才是 `'SRC' -> 'DEST'`。
- ownership 失敗（非 root）：`install: failed to chown '<dst>': changing ownership of '<dst>': <reason>`；只給 `-g` 時中段為 `changing group of`；皆無尾號句點。失敗時 chmod 不套用（dst 留在 copy 後的 0600）。`-U/--unprivileged` 時 chown 失敗不報錯、chmod 仍套用（參考 `-U -o root` → mode 755）。
- `--context CTX`（無 `=`）在 clap 屬可选值但仍消耗下一個非旗標 token → `install --context s0 src` 只剩一個 operand → `install: missing destination file operand after 'src'`。`-Z`/`-P`/`--preserve-context` 接受但不作用。
- `-s/--strip`：host 逐字 `strip: <dst>: file format not recognized` ＋ `install: strip program failed: strip process terminated abnormally - exit code: 1`；guest 無 `strip` 程式 → 測試只斷言 rc1 且 stderr 含 `strip`。
- guest 無 `/etc/passwd`：`install -o root` → `install: invalid user: 'root'`（與 host 名稱查找失敗同措辭）→ 測試改用數字 id。
- **已知偏差：`--strip-program <PROGRAM>` 未實作**（參考 help 列有此選項）。

### mkfifo / mknod
- `-m` 缺值一律用長名稱措辭 `error: a value is required for '--mode <MODE>' but none was supplied`（參考對短旗標也用長名稱）。
- mode 解析錯誤延後到 positional 檢查之後（clap 先報缺少 `<fifo>...`/`<NAME>...`）。
- `-Z`/`--context` 接受但不作用；`-Zfoo` 的附值被吞（不再把 `foo` 當短旗標簇）。
- symbolic mode 基底 0666（`mkfifo`/`mknod` 共用 `chmod::apply_spec(0o666, spec)`）。
- `mknod NAME c|b`（major/minor 全缺）→ `mknod: Special files require major and minor device numbers.`；major/minor 只接受十進位 plain（`010`→10、`0x10` 被拒為 `error: invalid value '0x10' for '[major]': invalid digit found in string`）；`-m` 給精確 mode 時在 mknod 之後 set_permissions。
- `unexpected` 的 `tip:` 行只在 arg 以 `-` 開頭時印出。

### dircolors
- `TERM`/`COLORTERM` filter 行只影響其後條目；filter 在檔尾則無影響。內建 db 全為 filter 行 → 未設定 TERM 的 guest 預設輸出 `LS_COLORS='';`（兩邊一致）。
- 別名：NORMAL/FILE→`fi`、PIPE→`pi`、LEFT→`lc`、RIGHT→`rc`；`OPTIONS`/`COLOR`/`EIGHTBIT` 識別但忽略。
- 未識別 keyword → `dircolors: unrecognized keyword <KEY>`（KEY 含 `=` 時整串出現，例 `DI=v x` → `unrecognized keyword DI=v`）。
- 一行僅一個 token → `dircolors: <name>:<lineno>: invalid line;  missing second token`（`line;` 後兩個空格）。
- **政策 (1)（GNU man 勝參考）**：`-p, --print-database` 描述採 man 的 `output the contents of the internal colors database`；本機參考寫 `print the byte counts`（uutils 錯字）→ 不跟隨。

### mktemp
- `--suffix` 要求 X run 位於結尾，否则 `mktemp: with --suffix, template '<t>' must end in X`（含 `--suffix=` 空值、X run 不在結尾如 `aXXXXb`）。
- `-t` 走 TMPDIR 前綴，且 X 檢查先於目錄分隔符檢查：`-t 'sub/dir.X'` → `mktemp: too few X's in template 'sub/dir.X'`；`-t 'a/bXXXXXX'` → `mktemp: invalid template, 'X', contains directory separator`。分支順序：`-p/--tmpdir` → `-t` → 無 operand → 路徑含目錄分隔符。
- 建立失敗措辭 `mktemp: failed to create file via template '<template>': <reason>`（`-d` 為 `... via template` 的 directory 變體）。
- **已知外觀偏差**：部分錯誤參考多一層引號（`template "'aXXXXb'"`），我方不加引號。

### shred
- 數值驗證排在 missing-operand 檢查之後：`shred -n <file>` → `shred: missing file operand`（檔案不變）。
- 缺值措辭採長選項名與參考 metavar：`'--iterations <NUMBER>'`、`'--size <N>'`、`'--random-source <random-source>'`（help 的 metavar 同）。
- size 後綴表採參考實測：`b`=512、`K`/`k`=1024、`KB`/`kB`=1000、`M`/`m`=1024²、`MB`=1000²、`G`/`g`=1024³、`GB`=1000³、`T`/`t`=1024⁴、`TB`=1000⁴、`P`/`p`=1024⁵、`E`/`e`=1024⁶；大寫 `B` 單獨不是後綴（`1B` 被拒）、`1Ki`/`1mb`/`1gb`/`1kb`/`1BB`/`1KK`/`1bB`/`1.5K` 皆被拒。
- `-u`／`--remove[=wipe|wipesync|unlink]` 的 rename 序列採參考逐字：`renamed to 00000`→`0000`→`000`→`00`→`0`，最後一行主語回到原檔名（verbose 用完整路徑）；1 字元檔名只有 `renamed to 0`。`--remove`（無 HOW）等同 `-u`。
- `--random-source`：只有開檔失敗才報來源路徑（`shred: <source>: No such file or directory`）；來源是目錄/讀取不足時報 `shred: <target>: File write pass failed: Is a directory|failed to fill whole buffer`。
- `--help` 與參考 byte-identical（含 `  [file]...  ` 的尾隨兩空格）。

### chcon
- `-H`/`-L`/`-P` 在 clap 解析完成後才檢查衝突（`-L -R ctx d` 與 `-R -L ctx d` 皆接受）；`-H` 缺 operand 時為 `error: the following required arguments were not provided:` ＋ `  --recursive`。
- 遞迴採 post-order（先子後自、entries 排序、`depth < 64` 防環）；`-L` 跟進所有連字號目錄、`-H` 只跟進命令列 operand、`-P`（預設）都不跟進。
- `--preserve-root` 對 `/` 的兩段措辭：`chcon: warning: It is dangerous to operate recursively on '/'. Use --no-preserve-root to override this failsafe.` 再 `chcon: Modifying root path failed on '/'. permission denied.`
- 無 context → `chcon: No files are specified.`（空行＋Try 行）；access 失敗 → `chcon: Accessing failed on '<p>'. No such file or directory (os error 2).`
- guest 無 SELinux policy，但 tmpfs 可存 `security.selinux` xattr → 標 label／`--reference`／verbose 路徑可在 guest 實測；`-Z`/`--context` 不是 chcon 選項（GNU man 亦無）→ `error: unexpected argument '-Z' found`。

### 本叢集 guest 結果
`tests/install.rs`(13)、`tests/mkfifo.rs`(12)、`tests/mknod.rs`(16)、`tests/mktemp.rs`(14)、`tests/dircolors.rs`(11)、`tests/chcon.rs`(15)、`tests/shred.rs`(16) 全綠；coverage（門檻 80）install 80.02%、mkfifo 93.31%、mknod 91.18%、mktemp 82.09%、dircolors 91.73%、chcon 85.51%、shred 86.82%。全量迴歸 guest test PASS=92 FAIL=0。
備註：`cargo test --test vdir|ls_extra|ls_coverage` 不產生自己的 profraw（ls 家族由 `tests/ls.rs`、`tests/dir.rs` 涵蓋，coverage.sh 報 83.15%）。

## E1：env / printenv / pwd / sleep

基準依据（m11455 之後的政策）：man 有描述以 man 為準；man 未指定處以本機參考
（`/usr/lib/cargo/bin/coreutils/<applet>`）實測對齊；邊界 quirk 全數記錄於此。

### env
- **不重現 GNU 的 exec 語意**：COMMAND 由 `std::process::Command`（posix_spawn）另開行程執行，
  而非 `execve` 取代當前行程。理由：本專案測試用 `testutil::run()` 是 in-process 呼叫 `dispatch`，
  真的 exec 會取代 test binary。差異記錄在案；exit code 傳承（被訊號殺 → 128+sig）。
  訊號處置在 child 的 `pre_exec` 內套用（`signal()`／`sigprocmask(SIG_BLOCK,…)`）。
- 同一信號只保留**第一次**出現的 action（實測 `--ignore-signal=INT --default-signal=INT`
  → 只剩 `INT        (2): IGNORE`；`--ignore-signal=TERM,INT --block-signal=1 --default-signal=15`
  → `HUP (1): BLOCK`、`INT (2): IGNORE`、`TERM (15): IGNORE`）。
- `--list-signal-handling` 的清單依信號編號遞升排序、格式 `{:<11}({}): {}`、寫到 stderr。
- 信號名稱：全數字（1..=64）、可選 `SIG` 前綴、大小寫不拘；`sig9`、`all`、`0` →
  `env: '<spec>': invalid signal`（rc125）。SIGKILL(9)/SIGSTOP(19) 的 ignore/default →
  `env: failed to set signal action for signal <N>: Invalid argument`（rc125）；block 允許。
- exit code 分級：選項類錯誤 125（重複選項、缺值、`-0` 與 command 共存、`-C` 缺 command、
  invalid signal、chdir 失敗）；`-f` 讀檔失敗 1；command 找不到 127；找到但不可執行（如目錄）126。
- `env: use -[v]S to pass options in shebang lines` 只附加在（a）clap 式 argument 錯誤（含重複、缺值）
  與（b）exec ENOENT 之後。
- 第一個 `NAME=VALUE` 之後，任何「不是 assignment」的 token 一律是 COMMAND（連 `-u` 也算），
  故 `env -i -v A=1 -u B true` → `executing: -u` ＋ `env: '-u': No such file or directory`（127）。
- `-v` 印 `executing:` 段（用**原始**命令字串，不是 PATH 解析後的絕對路徑）；
  `-v -v`（或 `-vv`）额外印 `input args:` 段，即使沒有 command 也印。
- `chdir` 的驗證排在 `executing:` 段之前（`env -v -i -C /tmp/nope /bin/true` 只印 chdir 錯誤）；
  措辭 `env: cannot change directory to '<dir>': <reason> (os error <n>)`。
- **dotenv 文法（man 只寫「read and set variables from a ".env"-style configuration file」）**
  採自訂乾淨文法：略過空白行與 `#` 註解、以第一個 `=` 切分、兩側 trim、去掉成對引號
  （`"q w"`→`q w`、`"a"b`→`ab`、找不到成對結尾則保留字面引號）、不做 `$` 展開、
  沒有 `=` 的行略過。本機參考用 dotenvy 的怪癖**不重現**：無 `=` 的行會與下一「以 `=` 開頭」的行
  合併（`M` + `=nomatch` → `M=nomatch`）、末行無 `=` 會報
  `env: <file>: 8:1 expecting "[Some('='), Some(':')]" but found EOF.`。
  檔讀不到 → `env: <path>: <reason> (os error <n>)`、rc1。
- 檔內設定一律先於任何 unset 套用（呼應 man 的 "prior to any unset and/or set"）。
- PATH 查找用「修改後」的環境；PATH 未設或空 → 內建預設 `/bin:/usr/bin`
  （佐證：guest 內 `env -i echo` 仍找到 echo）。
- `env --help` 與參考 byte-identical（28 行）；首行 `Set each NAME to VALUE in the environment and run COMMAND`
  **沒有句號**。

### pwd
- 預設為 physical（`-P` 語意）；`-L` 僅在 `PWD` 為絕對路徑且其 dev/ino 與實際 cwd 相符時才採用，
  否則退回 physical（佐證 `PWD=/bogus pwd -L` → 印 physical）。後出現的 `-L`/`-P` 贏。
- 非選項 operand → rc0 且 `pwd: ignoring non-option arguments`。
- 未知選項是 clap 結構、rc **1**（不同於 printenv 的 rc2）。

### sleep
- 尾綴大小寫敏感：只接受結尾 `s`/`m`/`h`/`d`（`1M`、`1S`、`1D`、`1m30` 皆 invalid）；
  倍率 1/60/3600/86400；多個 operand 求和。
- **已知偏差**：不接受 hex float（參考依 C `strtod` 接受 `sleep 0x10` = 16 秒）。
- `-1` 不是「invalid time interval」而是 clap 的 `error: unexpected argument '-1' found`（rc1）；
  真正的負數 interval 要寫 `sleep -- -1`。

### printenv
- 無 operand → 依 environ 原始順序印 `NAME=VALUE`（不排序）；有 operand → 印存在者，
  缺任一 → rc1 且 stderr 空；`printenv ""` → rc1 無輸出。
- 未知選項 rc **2**。

### clap 措辭的統一決定
- 重複選項錯誤統一由 `crate::pathname::clap_dup(long, metavar, code, stderr)` 產生
  （`error: the argument '--<long>[ <metavar>]' cannot be used multiple times` ＋ 空行 ＋
  `For more information, try '--help'.`；無 tip、無 Usage）；code 由呼叫端給
  （pwd 1、printenv 2、env 125）。
- **已知外觀偏差**：參考（clap）對某些短旗標的顯示會吃掉 dash —— `sleep -0x1` 報 `'0'`、
  `env -0x1` 報 `'-x'`；我方一律報 `'-<第一個字元>'`。

### 本叢集 guest 結果
`tests/pwd.rs`(10)、`tests/sleep.rs`(10)、`tests/printenv.rs`(12)、`tests/env.rs`(19) 全綠；
coverage（門檻 80）pwd 96.28%、sleep 99.39%、printenv 97.26%、env 87.03%。
測試內把 multi-call binary 以 **symlink** 提供成 `<dir>/<name>`（argv[0] basename 分派照樣成立）；
用 `fs::copy` 會把 instrumented binary 塞爆 guest 的 tmpfs（`No space left on device`）。

## E2 叢集：id / whoami / groups / logname / hostid（使用者與群組資訊）

### 名稱解析不走 NSS（重要實作決定）
本專案的 binary 以 `-C target-feature=+crt-static` 靜態連結；靜態 glibc 的
`getpwnam`/`getgrgid` 需要 dlopen `libnss_files.so.2`，在 static+initramus 環境不可靠。
因此 `src/passwd.rs` 自行解析 `/etc/passwd` 與 `/etc/group`。
佐證：把本機參考放進掛了自訂 `/etc/passwd`＋`/etc/group` 的沙箱，它即依該兩檔解析
（`id t2` → `uid=1001(t2) gid=200(grpB) groups=200(grpB),100(grpA)`）。

### 群組清單不去重（實測兩邊一致）
`/etc/group` 有多個 gid 同名時，`id -Gn`/`groups` 照檔案順序印出重複名稱
（實測 `id -Gn` → `g1000 g65534 g65534 g65534 g65534 g65534 g65534 g65534 g65534`）；
指定使用者時 gid 一律排第一，其後依 `/etc/group` 檔案順序、且略與 gid 重複者。

### 名稱解析失敗的行為（無 /etc/passwd 時，guest 即此情境）
- 預設格式：印 `<id>(<id>)`，並每筆失敗印一行 `id: cannot find name for user ID <n>` /
  `id: cannot find name for group ID <n>`；**rc 仍為 0**。
- `-n` 模式：印數字且 **rc 1**（man：「If any of the ID numbers cannot be mapped into
  names, the number will be displayed as usual」）。
- `-p`：印數字、rc 0、**不印 warning**（且名稱找不到時沒有 `login` 列）。
- `-P` 找不到條目 → `id: No such id: <uid>`（無 operand）或 `id: '<op>': no such user`。

### 指定 USER 時的預設格式（man 優先於本機參考）
`docs/man/id.1.gz` 尾部：「If a user (login name or user ID) is specified, the user and
group IDs of that user are displayed. In this case, the real and effective IDs are
assumed to be the same.」→ 我方對指定 USER 只印 `uid=/gid=/groups=`（groups 取自該使用者
在 `/etc/group` 的成員關係）。本機參考在同樣輸入下會混入**目前程序**的 `euid=/egid=`
與補充群組（例 `id root` → `uid=0(root) gid=0(root) euid=1000(chenpc) egid=1000(chenpc)
groups=<目前程序群組>`），與 man 描述不符 → 記錄為已知分歧。

### `-A` 與 `-a`
- `-a, --ignore`：兼容性 no-op（man：「ignore, for compatibility with other versions」）。
- `-A`：man 寫「Display the process audit user ID … which requires privilege (not
  available on Linux)」→ 我方完全無輸出（rc 0），且**連不存在的 USER 也不報錯**（參考實測相同）。

### 照抄的參考怪癖
- `-P` 給多個 operand 時只處理第一個。
- `-p` 給指定使用者時輸出三列：`uid\t<name>`、`groups\t`（值空）、第三列才是群組清單。
- operand 的引號包裝：operand 內含 `'` 時改用雙引號（`id a'b` → `id: "a'b": no such user`），
  否則單引號（空 operand → `id: '': no such user`）。
- whoami/logname 無 positional → clap 錯誤**沒有 tip 行**（`error:` / 空 / `Usage: whoami` / 空 /
  `For more information, try '--help'.`）；有 positional 的 id/groups 才有 tip。
- `id --bogus` 之類的 fuzzy tip（參考會給 `--groups`）不重現，一律 `  tip: to pass ...`。

### logname / whoami 的取名順序
- `whoami`：`geteuid()` → `/etc/passwd`（不受 `LOGNAME`/`USER` 環境影響，實測兩邊一致）。
- `logname`：`getlogin(3)` 優先，空則退回 `uid → /etc/passwd`；guest 無 utmp → 走退回路徑
  （沙箱實測：掛自訂 passwd 即成功，無 db 則 `logname: no login name`、rc 1）。
- `groups`：無 operand 時**沒有** ` : ` 前綴；有 operand 時標籤是 operand 原文（`groups 0` → `0 : root`）。
- `hostid`：印 `gethostid()` 的 8 位小寫十六進位；man 未載任何寫入模式（無 GNU 的 `-a`）。

### 本叢集 guest 結果
`tests/id.rs`(12)、`tests/groups.rs`(9)、`tests/whoami.rs`(6)、`tests/logname.rs`(6)、
`tests/hostid.rs`(4) 全綠；coverage（門檻 80）id 91.21%、groups 97.04%、whoami 97.59%、
logname 83.52%、hostid 86.84%。guest 內 uid/gid=0 且 `getgroups` 只回 `[0]`；
guest 的 `/etc` 不存在（tmpfs、init 未建）→ 測試需 `create_dir_all("/etc")` 才能自建 db。

## E3 叢集：arch / uname / nproc / users / who

### arch
- 輸出直接取自 `uname(2)` 的 `machine` 欄（`src/sys.rs: machine_name()`），不讀 `/proc`、不猜架構。
- arch **沒有 positional 參數**，所以未知引數的 clap 錯誤不附 tip 行：
  `error: unexpected argument 'x' found` + 空行 + `Usage: arch` + 空行 + `For more information, try '--help'.`（rc1）。
  佐證：本機參考 `arch x`、`arch --bogus`、`arch -- --bogus` 皆無 tip。
- `--` 是 end-of-options（不是 operand）：`arch --` → rc0 印 `x86_64`（參考實測）。
- 短旗標簇：第一個未知字元即錯誤（`arch -xh` → `error: unexpected argument '-x' found`），
  但 `arch -Vx`/`arch -hx` 會在遇到 `V`/`h` 時直接輸出並 rc0（參考實測）。

### uname
- 欄位全部來自 `uname(2)`（`src/sys.rs: utsname()`）；`-o/--operating-system` 硬編碼 `GNU/Linux`
  （man 與參考皆如此寫法，不偵測 libc 類型）。
- man 未寫「無選項時的預設輸出」→ 採參考行為：只印 sysname（等同 `-s`）。
- `-a` 的順序固定為 sysname、nodename、release、version、machine、`GNU/Linux`；
  同時指定多個選項時一律照這個 canonical order 輸出（不是命令行順序）。
- 重複選項錯誤用長名稱含 `--`（`error: the argument '--all' cannot be used multiple times`）。
- `--` 之後的 token 一律是 positional（uname 無 positional → unexpected）。

### nproc
- base 數值：預設用 `sched_getaffinity` 的 CPU 數（「available to the current process」），
  呼叫失敗才退回數 `/proc/stat` 的 `cpuN` 行（`src/sys.rs: present_cpu_count()`）；
  `--all` 反過來優先使用系統層級的 `present_cpu_count()`。
- 環境變數語意（man 只說「determine the minimum and maximum returned value」）：
  `OMP_NUM_THREADS` **取代** base（>0 且可解析才算設定，`--all` 時不生效）；
  `OMP_THREAD_LIMIT` 在 `--ignore` 之後當上限；結果最低 1。
- 錯誤結尾只有一空行 + `For more information, try '--help'.`，**沒有** Usage 段也沒有 `Try '...'` 行（參考實測）。

### users / who（utmp 解析）
- 自行解析 utmp（`src/utmp.rs`）而非 libc `getutent`：`RECORD_SIZE = 384`，
  欄位位移依 glibc `struct utmp`：ut_type@0、ut_pid@4、ut_line@8(32)、ut_id@40(4)、
  ut_user@44(32)、ut_host@76(256)、`__exit_status`@332（`e_term`@332、`e_termination`@334）、
  ut_session@336、ut_tv.tv_sec@340、tv_usec@344、ut_addr_v6@348。
  who 的 EXIT 欄印成 `term=<e_term> exit=<e_termination>`。
- 預設檔案 `/var/run/utmp`；檔案不存在 → rc0 且無輸出（man 未定義，採參考行為）。
- 時間欄格式 `YYYY-MM-DD HH:MM`，用 `localtime`（man 未寫明格式細節）。
- who 的欄位寬度與 heading 採**固定字串表**（`fn heading()`），依本機參考實測逐字對齊：
  `-a` → `NAME       LINE         TIME       IDLE          PID COMMENT  EXIT`（NAME 寬 10）；
  `-d` → 同 `-a` 但 NAME 寬 **8**；`-u/-l/-r` → `NAME     LINE         TIME       IDLE          PID COMMENT`；
  `-p/-b/-t` → `NAME     LINE         TIME              PID COMMENT`（保留 IDLE 欄寬但**不印標籤**）；
  `-T` → `NAME       LINE         TIME       COMMENT`；預設 → `NAME     LINE         TIME       COMMENT`。
  優先序：`-a` > `-d` > `-u/-l/-r` > `-p/-b/-t` > `-T` > 短格式。
- message status 欄（`-T`/`-a`）：GNU 用 terminal ioctl 判斷；本實作改用 stat 推定 —
  `/dev/<ut_line>` 不存在 → `?`，mode & 0o222 ≠ 0 → `+`，否則 `-`；
  該字元放在 NAME 欄最後一個字元位置（因此 `-a`/`-T` 的 NAME 寬是 10）。這是記錄在案的實作替換。
- `-q`：先印 user 名字（每 14 個一列、空白分隔），再印 `# users=N`；
  0 個 user 時仍先印一個空行（參考實測）。
- DEAD_PROCESS 記錄：IDLE 欄印 `gone`，並輸出 EXIT 欄。
- `users` 只取 USER_PROCESS 的 user 名稱、排序但保留重複（man：「like who -q 但不印時間」）。

### 已知分歧（不再追本機參考的 clap 細部）
- 長選項前綴推論（clap `infer_long_args`）與「旗標收到 inline 值」的
  `error: unexpected value '1' for '--help' found; no more were expected` 只有 **arch** 完整實作
  （共用 helper：`src/pathname.rs:619 resolve_long()`、`:637 clap_unexpected_value()`）；
  uname/users/who/nproc 目前只認精確長選項名，`--heading=1` 這類 inline 值會被安靜接受。
  兩條措辭皆有參考佐證（`arch --hel`、`arch --help=1`、`who --heading=1`、`nproc --all=1`、
  `uname --ker` → `tip: a similar argument exists: '--kernel-name'`），列為後續可補的 CLI 層工作。

### 本叢集 guest 結果
`tests/arch.rs`(5)、`tests/uname.rs`(11)、`tests/nproc.rs`(9)、`tests/users.rs`(8)、`tests/who.rs`(11) 全綠；
coverage（門檻 80）arch 84.44%、uname 80.68%、nproc 84.54%、users 82.96%、who 89.48%。
