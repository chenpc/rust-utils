# 最終報告：115 applet × guest test × coverage × fuzz

本報告由 `scripts/gen_report.py` 從 `logs/` 證據生成，與 `docs/coreutils_checklist.md` 逐條核對。

| # | applet | checklist | coverage% | guest test（最新 log） | fuzz | man 來源 |
|---|---|---|---|---|---|---|
| 1 | `[` | ☑ 93.12 | 93.12 | PASS 21/21（test-5fc4b8c66373b257.log） | ☑ | 本機 mirror |
| 2 | `test` | ☑ 93.12 | 93.12 | PASS 21/21（test-5fc4b8c66373b257.log） | ☑ | 本機 mirror |
| 3 | `echo` | ☑ | 94.8 | PASS 16/16（echo-8ab622b758a70424.log） | ☑ | 本機 mirror |
| 4 | `printf` | ☑ 89.88 | 89.88 | PASS 20/20（printf-98e4466bfd595e10.log） | ☑ | 本機 mirror |
| 5 | `true` | ☑ | 100.0 | PASS 5/5（true-fa9f25027cb98b2d.log） | ☑ | 本機 mirror |
| 6 | `false` | ☑ | 100.0 | PASS 6/6（false-167c680370dc57e9.log） | ☑ | 本機 mirror |
| 7 | `yes` | ☑ | 100.0 | PASS 7/7（yes-56ea540f8b6a6a0c.log） | ☑ | 本機 mirror |
| 8 | `cat` | ☑ | 98.31 | PASS 11/11（cat-14a802f6a4d2231f.log） | ☑ | 本機 mirror |
| 9 | `tr` | ☑ | 94.22 | PASS 17/17（tr-1fa72ecb603728b7.log） | ☑ | 本機 mirror |
| 10 | `head` | ☑ | 84.32 | PASS 16/16（head-1a591b8e5746f26b.log） | ☑ | 本機 mirror |
| 11 | `tail` | ☑ | 83.03 | PASS 15/15（tail-0387f39a8f521f4d.log） | ☑ | 本機 mirror |
| 12 | `nl` | ☑ | 87.39 | PASS 23/23（nl-b7ae372809d385be.log） | ☑ | 本機 mirror |
| 13 | `od` | ☑ 86.08 | 86.08 | PASS 31/31（od-b1104b04201cc9cb.log） | ☑ | 本機 mirror |
| 14 | `expand` | ☑ | 91.53 | PASS 15/15（expand-e7ba0ae485757cc3.log） | ☑ | 本機 mirror |
| 15 | `unexpand` | ☑ | 89.12 | PASS 13/13（unexpand-ddab20b5416d87e8.log） | ☑ | 本機 mirror |
| 16 | `fmt` | ☑ 90.49 | 90.49 | PASS 21/21（fmt-96f99ca3808ef05c.log） | ☑ | 本機 mirror |
| 17 | `fold` | ☑ | 80.56 | PASS 12/12（fold-01cb21159745641c.log） | ☑ | 本機 mirror |
| 18 | `wc` | ☑ | 83.73 | PASS 13/13（wc-f0c90dd9209f1f42.log） | ☑ | 本機 mirror |
| 19 | `sort` | ☑ 86.52 | 86.52 | PASS 33/33（sort-2c93902a1729f10a.log） | ☑ | 本機 mirror |
| 20 | `uniq` | ☑ | 85.37 | PASS 18/18（uniq-1cb39ff40eb54912.log） | ☑ | 本機 mirror |
| 21 | `comm` | ☑ | 92.86 | PASS 12/12（comm-380041310dda094f.log） | ☑ | 本機 mirror |
| 22 | `join` | ☑ | 85.08 | PASS 17/17（join-64daef22b849c404.log） | ☑ | 本機 mirror |
| 23 | `csplit` | ☑ | 86.82 | PASS 14/14（csplit-1be6be10e26b9962.log） | ☑ | 本機 mirror |
| 24 | `split` | ☑ | 88.39 | PASS 19/19（split-c4eb0903e3589363.log） | ☑ | 本機 mirror |
| 25 | `expr` | ☑ | 93.73 | PASS 17/17（expr-6fa192076f2ef372.log） | ☑ | 本機 mirror |
| 26 | `tsort` | ☑ | 95.97 | PASS 10/10（tsort-c6ebd69fbf818044.log） | ☑ | 本機 mirror |
| 27 | `ptx` | ☑ 88.91 | 88.91 | PASS 17/17（ptx-a6adada75fe4c4b5.log） | ☑ | 本機 mirror |
| 28 | `numfmt` | ☑ | 92.41 | PASS 35/35（numfmt-2f40204c41136c38.log） | ☑ | 本機 mirror |
| 29 | `shuf` | ☑ | 81.29 | PASS 16/16（shuf-56125b9fa63532c0.log） | ☑ | 本機 mirror |
| 30 | `tac` | ☑ | 91.46 | PASS 14/14（tac-981b6cc4c49a4f1d.log） | ☑ | 本機 mirror |
| 31 | `paste` | ☑ | 83.91 | PASS 14/14（paste-5b65792c1732e8b4.log） | ☑ | 本機 mirror |
| 32 | `cut` | ☑ | 87.92 | PASS 27/27（cut-e1dd8d2f2284bc52.log） | ☑ | 本機 mirror |
| 33 | `seq` | ☑ | 80.21 | PASS 48/48（seq-7bcb486b702d51ff.log） | ☑ | 本機 mirror |
| 34 | `tee` | ☑ | 80.56 | PASS 14/14（tee-c235c66bc666ffad.log） | ☑ | 本機 mirror |
| 35 | `base32` | ☑ 100.00 | 100.0 | PASS 17/17（base32-833ef881ef9881ca.log） | ☑ | 本機 mirror |
| 36 | `base64` | ☑ 100.00 | 100.0 | PASS 14/14（base64-eea38b7ac51eabcc.log） | ☑ | 本機 mirror |
| 37 | `basenc` | ☑ 100.00 | 100.0 | PASS 18/18（basenc-b96149578f24514b.log） | ☑ | 本機 mirror |
| 38 | `cksum` | ☑ 100.00 | 100.0 | PASS 6/6（cksum-ee3649cfadff6466.log） | ☑ | 本機 mirror |
| 39 | `sum` | ☑ 100.00 | 100.0 | PASS 3/3（sum-5a872d959b30e252.log） | ☑ | 本機 mirror |
| 40 | `md5sum` | ☑ 100.00 | 100.0 | PASS 3/3（md5sum-a3c298a383f21e31.log） | ☑ | 本機 mirror |
| 41 | `sha1sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha1sum-96dedf979c52b17f.log） | ☑ | 本機 mirror |
| 42 | `sha224sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha224sum-458b7d4f4480fdc6.log） | ☑ | 本機 mirror |
| 43 | `sha256sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha256sum-4eecc0160eeb173f.log） | ☑ | 本機 mirror |
| 44 | `sha384sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha384sum-071bcde3ee06c316.log） | ☑ | 本機 mirror |
| 45 | `sha512sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha512sum-0ad640d584b7a76b.log） | ☑ | 本機 mirror |
| 46 | `sha3-224sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha3_224sum-1069ebb735f9b5aa.log） | ☑ | web |
| 47 | `sha3-256sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha3_256sum-7125b746189ccf9c.log） | ☑ | web |
| 48 | `sha3-384sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha3_384sum-b9a3484fd145ecf5.log） | ☑ | web |
| 49 | `sha3-512sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha3_512sum-0b0b2552a4d774e3.log） | ☑ | web |
| 50 | `sha3sum` | ☑ 100.00 | 100.0 | PASS 3/3（sha3sum-0572495ad2e0bd33.log） | ☑ | web |
| 51 | `shake128sum` | ☑ 100.00 | 100.0 | PASS 3/3（shake128sum-42cf10fa26863e26.log） | ☑ | web |
| 52 | `shake256sum` | ☑ 100.00 | 100.0 | PASS 3/3（shake256sum-9dbf2e55c4851029.log） | ☑ | web |
| 53 | `b2sum` | ☑ 100.00 | 100.0 | PASS 3/3（b2sum-6e337d2f95c56548.log） | ☑ | 本機 mirror |
| 54 | `b3sum` | ☑ 100.00 | 100.0 | PASS 3/3（b3sum-6cbe7e0987615092.log） | ☑ | (無 man page mirror：Debian rust-coreutils 無此頁，uutils docs/mankier/arch 皆 404) |
| 55 | `hashsum` | ☑ 100.00 | 100.0 | PASS 5/5（hashsum-a0908477c95efb35.log） | ☑ | web |
| 56 | `basename` | ☑ 83.42 | 83.42 | PASS 6/6（basename-c4fc26b17f82c648.log） | ☑ | 本機 mirror |
| 57 | `dirname` | ☑ 98.94 | 98.94 | PASS 6/6（dirname-938e45fdbc18cce9.log） | ☑ | 本機 mirror |
| 58 | `pathchk` | ☑ 100.00 | 100.0 | PASS 6/6（pathchk-98798aa8667b6050.log） | ☑ | 本機 mirror |
| 59 | `realpath` | ☑ 96.76 | 96.76 | PASS 9/9（realpath-e9d46e1fcd853113.log） | ☑ | 本機 mirror |
| 60 | `relpath` | ☑ 91.81 | 91.81 | PASS 6/6（relpath-85de513c85ad2d25.log） | ☑ | - |
| 61 | `readlink` | ☑ 97.22 | 97.22 | PASS 7/7（readlink-2d8e92bbe3d94a5b.log） | ☑ | 本機 mirror |
| 62 | `ls` | ☑ 83.15 | 83.15 | PASS 16/16（ls-64a33985bc8cc0ce.log） | ☑ | 本機 mirror |
| 63 | `dir` | ☑ 83.15 | 83.15 | PASS 3/3（dir-2a477a5915a94490.log） | ☑ | 本機 mirror |
| 64 | `vdir` | ☑ 83.15 | 83.15 | PASS 2/2（vdir-b2303462baedf847.log） | ☑ | 本機 mirror |
| 65 | `stat` | ☑ | 85.37 | PASS 11/11（stat-57deb5bcbb32d026.log） | ☑ | 本機 mirror |
| 66 | `mkdir` | ☑ | 83.16 | PASS 7/7（mkdir-3975076cf230569a.log） | ☑ | 本機 mirror |
| 67 | `rmdir` | ☑ | 98.61 | PASS 5/5（rmdir-40bdac021e9270e4.log） | ☑ | 本機 mirror |
| 68 | `ln` | ☑ | 89.03 | PASS 6/6（ln-c8b99f128fef3850.log） | ☑ | 本機 mirror |
| 69 | `link` | ☑ | 93.85 | PASS 2/2（link-4f54a0d278037636.log） | ☑ | 本機 mirror |
| 70 | `unlink` | ☑ | 96.49 | PASS 2/2（unlink-5a2f016c2b326017.log） | ☑ | 本機 mirror |
| 71 | `mv` | ☑ | 84.66 | PASS 8/8（mv-0f1c16bcbdfc3b04.log） | ☑ | 本機 mirror |
| 72 | `cp` | ☑ | 81.44 | PASS 11/11（cp-c5323cdd7ccac195.log） | ☑ | 本機 mirror |
| 73 | `rm` | ☑ | 83.06 | PASS 7/7（rm-fdbf13baf6ba0ae5.log） | ☑ | 本機 mirror |
| 74 | `touch` | ☑ | 94.19 | PASS 7/7（touch-852ed4499784e0c9.log） | ☑ | 本機 mirror |
| 75 | `install` | ☑ | 80.02 | PASS 13/13（install-97f8d8f1fcae5e86.log） | ☑ | 本機 mirror |
| 76 | `mkfifo` | ☑ | 93.31 | PASS 12/12（mkfifo-f3c5133380892853.log） | ☑ | 本機 mirror |
| 77 | `mknod` | ☑ | 92.27 | PASS 17/17（mknod-12ae6bb690ef68d3.log） | ☑ | 本機 mirror |
| 78 | `dircolors` | ☑ | 91.73 | PASS 11/11（dircolors-345e372678434578.log） | ☑ | 本機 mirror |
| 79 | `chmod` | ☑ | 89.26 | PASS 8/8（chmod-de9dd11163cd0ff8.log） | ☑ | 本機 mirror |
| 80 | `chgrp` | ☑ | 94.83 | PASS 4/4（chgrp-d6a55e24202743f2.log） | ☑ | 本機 mirror |
| 81 | `chown` | ☑ | 100.0 | PASS 7/7（chown-178dba4d715360b6.log） | ☑ | 本機 mirror |
| 82 | `chcon` | ☑ | 85.51 | PASS 15/15（chcon-5dbdb03d40626aa6.log） | ☑ | 本機 mirror |
| 83 | `truncate` | ☑ | 88.44 | PASS 5/5（truncate-a13b7416e91eb9c7.log） | ☑ | 本機 mirror |
| 84 | `mktemp` | ☑ | 82.29 | PASS 15/15（mktemp-7063aaa16bfffe96.log） | ☑ | 本機 mirror |
| 85 | `shred` | ☑ | 86.82 | PASS 16/16（shred-69c2fa3f591389b6.log） | ☑ | 本機 mirror |
| 86 | `env` | ☑ | 87.03 | PASS 19/19（env-2c5045b6476d5573.log） | ☑ | 本機 mirror |
| 87 | `printenv` | ☑ | 97.26 | PASS 12/12（printenv-e2c040eb405a1cc9.log） | ☑ | 本機 mirror |
| 88 | `pwd` | ☑ | 96.28 | PASS 10/10（pwd-3b9d3bcf62a26c59.log） | ☑ | 本機 mirror |
| 89 | `nohup` | ☑ | 83.02 | PASS 14/14（nohup-3870970d95078124.log） | ☑ | 本機 mirror |
| 90 | `nice` | ☑ | 83.2 | PASS 12/12（nice-b95f7ffd3760fcf6.log） | ☑ | 本機 mirror |
| 91 | `stdbuf` | ☑ | 92.37 | PASS 13/13（stdbuf-e6978cb522e6823f.log） | ☑ | 本機 mirror |
| 92 | `stty` | ☑ | 81.76 | PASS 8/8（stty-3669d10ae970b4d0.log） | ☑ | 本機 mirror |
| 93 | `tty` | ☑ | 91.56 | PASS 10/10（tty-020b7670d9e559a6.log） | ☑ | 本機 mirror |
| 94 | `timeout` | ☑ | 82.2 | PASS 18/18（timeout-c18b16f8b3c21450.log） | ☑ | 本機 mirror |
| 95 | `sleep` | ☑ | 99.39 | PASS 10/10（sleep-b919da0252328e01.log） | ☑ | 本機 mirror |
| 96 | `runcon` | ☑ | 84.41 | PASS 13/13（runcon-82ead663c06b3fb2.log） | ☑ | 本機 mirror |
| 97 | `chroot` | ☑ | 81.86 | PASS 20/20（chroot-ebca449e32e05d1b.log） | ☑ | 本機 mirror |
| 98 | `groups` | ☑ | 97.04 | PASS 9/9（groups-596de40e1864e070.log） | ☑ | 本機 mirror |
| 99 | `id` | ☑ | 91.21 | PASS 12/12（id-68c3d27e7601580d.log） | ☑ | 本機 mirror |
| 100 | `users` | ☑ | 82.96 | PASS 8/8（users-05cbc7ba6dbf1afa.log） | ☑ | 本機 mirror |
| 101 | `who` | ☑ | 89.48 | PASS 12/12（who-82621ee2505cd503.log） | ☑ | 本機 mirror |
| 102 | `whoami` | ☑ | 100.0 | PASS 7/7（whoami-dd41f97e4c8e7615.log） | ☑ | 本機 mirror |
| 103 | `logname` | ☑ | 94.51 | PASS 7/7（logname-aa5eeda3beb1444c.log） | ☑ | 本機 mirror |
| 104 | `hostid` | ☑ | 97.37 | PASS 5/5（hostid-622715edd9d28418.log） | ☑ | 本機 mirror |
| 105 | `arch` | ☑ | 84.44 | PASS 6/6（arch-0785ae914d1be6dc.log） | ☑ | 本機 mirror |
| 106 | `uname` | ☑ | 80.68 | PASS 9/9（uname-d239d7e47b06fa46.log） | ☑ | 本機 mirror |
| 107 | `nproc` | ☑ | 85.05 | PASS 6/6（nproc-d4963b10bdd74e2e.log） | ☑ | 本機 mirror |
| 108 | `date` | ☑ | 87.42 | PASS 15/15（date-bd2c2699dbb14f06.log） | ☑ | 本機 mirror |
| 109 | `factor` | ☑ | 88.81 | PASS 13/13（factor-7ef8ebfea21d670f.log） | ☑ | 本機 mirror |
| 110 | `pr` | ☑ | 82.58 | PASS 14/14（pr-adcfc3d82bab86c9.log） | ☑ | 本機 mirror |
| 111 | `pinky` | ☑ | 86.21 | PASS 9/9（pinky-45387e16e3c062e8.log） | ☑ | 本機 mirror |
| 112 | `df` | ☑ | 80.45 | PASS 17/17（df-9dbf7c32cb6d575e.log） | ☑ | 本機 mirror |
| 113 | `du` | ☑ | 92.3 | PASS 27/27（du-8c73cafeaad77ea1.log） | ☑ | 本機 mirror |
| 114 | `sync` | ☑ | 93.23 | PASS 12/12（sync-3f162cf516173c1d.log） | ☑ | 本機 mirror |
| 115 | `dd` | ☑ | 82.78 | PASS 26/26（dd-2a5f26f8a456bd62.log） | ☑ | 本機 mirror |

- applet 列數：115；有 coverage 數字：115；coverage 最小值：80.02%；平均：91.22%
- 有 fuzz 不變量測試：115/115
- 跳過（☐/原因）：無——115 個 applet 全數實作並通過 guest test（跳過清單為空；歷史上曾記錄的「本機參考無此程式」等偏差見 checklist 備註與 docs/assumptions.md）

## 不一致清單

- 無：每個 ☑ 都有 coverage ≥80%、最新 guest test log 為 PASS、且 tests/<applet>.rs 含 fuzz 不變量。

