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
