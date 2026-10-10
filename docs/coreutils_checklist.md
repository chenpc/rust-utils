# Coreutils 實作清單（Rust）

來源：已安裝的 `rust-coreutils`（uutils coreutils **0.10.0-1ubuntu2~26.04.1**），
用 `dpkg -L rust-coreutils | grep '^/usr/lib/cargo/bin/coreutils/'` 取得實際 applet 清單。
共 **115** 個 applet。

狀態欄：`☐` 未開始、`◐` 進行中、`☑` 完成且輸出與參考實作一致。

---

## 實作規則（硬性）

1. **只能靠 man page 推敲行為。** 不可以上網看 coreutils 的 source code（GNU 或 uutils 都不行）。
   唯一允許的依據是 man page；實作者的行為模型 = 從 man page 推導出来的行為。
   man page 存放於 `docs/man/`（本機 mirror，見下節）。
   若 man page 沒寫到的行為，視為未定義，不得照抄其他實作。
2. **每一個 tool 都要有 e2e test（`cargo test`）。**
3. **`cargo test` 一律在 qemu guest 內跑，不能在本機跑。**
   執行方式：`./run.sh <test-id>`，test payload 放在 `guest-share/tests/`，
   判定依據是 serial log 裡的 `QEMU-INIT` / `QEMU-TEST` marker。
   理由：本機環境不可控（已裝的 coreutils、locale、時區、權限都會影響結果），
   guest 是乾淨且可重製的基準環境。
4. **每個 tool 的基本測試 coverage ≥ 80%。**
   量測方式：`cargo llvm-cov --summary-only`（在 guest 內執行），門檻寫在 `coverage.toml`。
5. **每個 tool 都要有 fuzz test** 驗證 random 行為：
   - 輸入隨機（長度、編碼、邊界值、空輸入、多檔、stdin）
   - 參數組合隨機
   - fuzz 的判定不能靠「跟 GNU 輸出相同」，因為規則 1 禁止看 source；
     判定必須靠 man page 可推導的**不變量**（例如 `wc` 的 byte 數 = 輸入長度、`tr` 的 1:1 映射、`sort` 的輸出是輸入的排列且單調）。

---

## Man page mirror（規則 1 的唯一依據）

本機 mirror 于 `docs/man/`，來源 `/usr/share/man/man1/`（uutils 版，實際檔案是 `rust-<name>.1.gz`）。

- **105 / 115 有本機 man page** → 已 mirror 到 `docs/man/<applet>.1.gz`。
- **10 / 115 本機缺**（symlink 是 dangling，`rust-*.1.gz` 不存在）→ **實作該 tool 時再用 web search 找**，找到後存入 `docs/man/` 並在該 tool 的條目記錄來源 URL 與 SHA512。

| applet | 本機 man | 實作時要查 |
|---|---|---|
| `b3sum` | ☑ 100.00 | BLAKE3 的 man page / 文件 |
| `hashsum` | ☑ 100.00 | |
| `relpath` | ☐ | |
| `sha3-224sum` | ☑ 100.00 | |
| `sha3-256sum` | ☑ 100.00 | |
| `sha3-384sum` | ☑ 100.00 | |
| `sha3-512sum` | ☑ 100.00 | |
| `sha3sum` | ☑ 100.00 | |
| `shake128sum` | ☑ 100.00 | |
| `shake256sum` | ☑ 100.00 | |

注意：本機 mirror 的是 **uutils（rust-）版** man page，不是 GNU 版。兩者對同一 applet 的描述可能不同
（例如 `sort` 的 locale、`wc` 的計數定義）。實作時以本機 mirror 為準，並在 `docs/man/SOURCES.tsv`
記錄每個檔案的來源與 SHA512，這樣「行為模型」可追溯。

---

## A. 純流式／文字處理（34）

只需 stdio，無 syscall，最容易先完成。

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 1 | `[` | ☑ 93.12 | 與 `test` 同一實作，只差 argv[0] |
| 2 | `test` | ☑ 93.12 | |
| 3 | `echo` | ☑ | 需支援 `-n -e -E`；guest coverage 94.80% |
| 4 | `printf` | ☑ 89.88 | 格式引擎最費工（`%b`、`%(...)d`、可重複格式） |
| 5 | `true` | ☑ | guest coverage 100.00% |
| 6 | `false` | ☑ | guest coverage 100.00% |
| 7 | `yes` | ☑ | guest coverage 100.00% |
| 8 | `cat` | ☑ | guest coverage 98.31% |
| 9 | `tr` | ☑ | 字集/範圍/`[:class:]`；guest coverage 94.22% |
| 10 | `head` | ☑ | 84.32% |
| 11 | `tail` | ☑ | 83.03% |
| 12 | `nl` | ☑ | 87.61% |
| 13 | `od` | ☑ 86.08 | 輸出格式（`-A -t -w`） |
| 14 | `expand` | ☑ | 91.45% |
| 15 | `unexpand` | ☑ | 88.70% |
| 16 | `fmt` | ☑ 90.49 | |
| 17 | `fold` | ☑ | 80.84% |
| 18 | `wc` | ☑ | 行/字/byte/最大行寬；guest coverage 83.73% |
| 19 | `sort` | ☑ 86.52 | key、locale、穩定排序、外部記憶體 |
| 20 | `uniq` | ☑ | 89.28% |
| 21 | `comm` | ☑ | 92.86% |
| 22 | `join` | ☑ | 85.08% |
| 23 | `csplit` | ☑ | 86.82% |
| 24 | `split` | ☑ | 88.39% |
| 25 | `expr` | ☑ | 93.73% | 需寫表達式解析器 |
| 26 | `tsort` | ☑ | 95.97% | 拓撲排序 |
| 27 | `ptx` | ☑ 88.91 | |
| 28 | `numfmt` | ☑ | 92.41% |
| 29 | `shuf` | ☑ | 81.29% |
| 30 | `tac` | ☑ | 91.46% |
| 31 | `paste` | ☑ | 83.91% |
| 32 | `cut` | ☑ | 87.92% |
| 33 | `seq` | ☐ | |
| 34 | `tee` | ☑ | 80.28% |

## B. 編碼／checksum／hash（21）

純演算法，無 syscall。`hashsum` 是共用主體，其餘是同一實作加不同演算法。

共用核心 `src/hashsum.rs` 另計（`scripts/coverage.sh` 只看 `src/applets/<applet>.rs`）：由 `tests/hashsum_core.rs` 單獨量測 = **50.02%** region coverage（2026-10-09）。check mode 的路徑由 applet 測試覆蓋，但 `coverage.sh` 不會把跨 binary 的共用核心加總。

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 35 | `base32` | ☑ 100.00 | 共用 `src/basen.rs` 核心 |
| 36 | `base64` | ☑ 100.00 | 共用 `src/basen.rs` 核心 |
| 37 | `basenc` | ☑ 100.00 | 9 種編碼；核心 `src/basen.rs` 87.04% |
| 38 | `cksum` | ☑ 100.00 | BSD checksum |
| 39 | `sum` | ☑ 100.00 | 1/2/3 三種演算法 |
| 40 | `md5sum` | ☑ 100.00 | |
| 41 | `sha1sum` | ☑ 100.00 | |
| 42 | `sha224sum` | ☑ 100.00 | |
| 43 | `sha256sum` | ☑ 100.00 | |
| 44 | `sha384sum` | ☑ 100.00 | |
| 45 | `sha512sum` | ☑ 100.00 | |
| 46 | `sha3-224sum` | ☑ 100.00 | |
| 47 | `sha3-256sum` | ☑ 100.00 | |
| 48 | `sha3-384sum` | ☑ 100.00 | |
| 49 | `sha3-512sum` | ☑ 100.00 | |
| 50 | `sha3sum` | ☑ 100.00 | GNU 較新 |
| 51 | `shake128sum` | ☑ 100.00 | GNU 較新 |
| 52 | `shake256sum` | ☑ 100.00 | GNU 較新 |
| 53 | `b2sum` | ☑ 100.00 | BLAKE2 |
| 54 | `b3sum` | ☑ 100.00 | BLAKE3 |
| 55 | `hashsum` | ☑ 100.00 | 共用主體 |

## C. 路徑／名稱（6）

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 56 | `basename` | ☑ 83.42 | |
| 57 | `dirname` | ☑ 98.94 | |
| 58 | `pathchk` | ☑ 100.00 | |
| 59 | `realpath` | ☑ 96.76 | |
| 60 | `relpath` | ☑ 91.81 | GNU 較新；參考 binary 無此 applet，見 assumptions |
| 61 | `readlink` | ☑ 97.22 | 需要 syscall |

## D. 檔案系統／權限（24）— 需要 syscall

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 62 | `ls` | ☐ | `dir`/`vdir` 共用同一實作 |
| 63 | `dir` | ☐ | |
| 64 | `vdir` | ☐ | |
| 65 | `stat` | ☐ | |
| 66 | `mkdir` | ☑ | coverage 83.29% |
| 67 | `rmdir` | ☑ | coverage 98.61% |
| 68 | `ln` | ☑ | coverage 89.03% |
| 69 | `link` | ☑ | coverage 93.85% |
| 70 | `unlink` | ☑ | coverage 96.49% |
| 71 | `mv` | ☐ | 跨裝置要 copy+unlink |
| 72 | `cp` | ☐ | 最費工：mode/ownership/symlink/`-a` |
| 73 | `rm` | ☐ | |
| 74 | `touch` | ☐ | |
| 75 | `install` | ☐ | |
| 76 | `mkfifo` | ☐ | 需要特權或 syscall |
| 77 | `mknod` | ☐ | 需要特權 |
| 78 | `dircolors` | ☐ | |
| 79 | `chmod` | ☐ | |
| 80 | `chgrp` | ☐ | |
| 81 | `chown` | ☐ | |
| 82 | `chcon` | ☐ | 需要 SELinux |
| 83 | `truncate` | ☑ | coverage 88.44% |
| 84 | `mktemp` | ☐ | |
| 85 | `shred` | ☐ | |

## E. 程序／環境／身分（26）

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 86 | `env` | ☐ | |
| 87 | `printenv` | ☐ | |
| 88 | `pwd` | ☐ | shell builtin 與外部版行為不同 |
| 89 | `nohup` | ☐ | |
| 90 | `nice` | ☐ | 需要 syscall/特權 |
| 91 | `stdbuf` | ☐ | 依賴 `LD_PRELOAD`（`libstdbuf.so`），不是單純 applet |
| 92 | `stty` | ☐ | termios |
| 93 | `tty` | ☐ | |
| 94 | `timeout` | ☐ | signal 處理 |
| 95 | `sleep` | ☐ | |
| 96 | `runcon` | ☐ | 需要 SELinux |
| 97 | `chroot` | ☐ | 需要特權 |
| 98 | `groups` | ☐ | |
| 99 | `id` | ☐ | |
| 100 | `users` | ☐ | |
| 101 | `who` | ☐ | |
| 102 | `whoami` | ☐ | |
| 103 | `logname` | ☐ | |
| 104 | `hostid` | ☐ | |
| 105 | `arch` | ☐ | |
| 106 | `uname` | ☐ | |
| 107 | `nproc` | ☐ | |
| 108 | `date` | ☐ | 解析 + 輸出格式 |
| 109 | `factor` | ☐ | |
| 110 | `pr` | ☐ | |
| 111 | `pinky` | ☐ | |

## F. 磁碟／系統（4）

| # | applet | 狀態 | 備註 |
|---|---|---|---|
| 112 | `df` | ☑ | coverage 80.45%；掛載表/去重（st_dev）/--output/human |
| 113 | `du` | ☑ | coverage 92.30%；-a/-s/-b/--exclude/--time/--files0-from |
| 114 | `sync` | ☑ | coverage 93.23%；-f(syncfs)/-d(fdatasync)/檔案 fsync |
| 115 | `dd` | ☑ | coverage 82.78%；ibs/obs/cbs/conv（除 EBCDIC/sparse）/seek/iflag/oflag |

---

## 建議實作順序

**A → B → C → F → D → E**

理由：先做完全無 syscall 的（A、B、C），可以立刻用「輸出與參考實作是否逐 byte 相同」當指標；
再進到需要 syscall 的（F、D），最後才是需要特權/SELinux/termios 的（E）。

## 比對基準（要選一個）

- 對 **GNU coreutils**：行為最標準，但 uutils 本身有意的差異（例如 `sort` 的 locale）會造成假陽性。
- 對 **uutils coreutils 0.10.0**（本機已裝）：容易自動化、可逐 applet 跑 diff，但基準本身不是 GNU。

建議：以 GNU coreutils 的 `info coreutils` 與 `tests/` 行為為準，uutils 只當第二參考。

## 備註

- `[` 與 `test` 是同一實作；`ls`/`dir`/`vdir` 共用；`hashsum` 與 B 群其餘是同一實作加不同演算法。
- `sha3sum`、`shake128sum`、`shake256sum`、`relpath` 是 GNU 較新的 applet。
- `chcon`/`runcon` 需要 SELinux；`chroot`/`mknod`/`mkfifo`/`nice` 需要特權或 raw syscall —— 這幾個剛好可當「非 root 能否實作」的測試點。
