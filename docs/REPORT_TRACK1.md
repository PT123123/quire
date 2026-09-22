# Track 1 · Page 外观与属性（SPEC §三十八 / M12）— 六片收口报告

写下时：2026-09-22，分支 `track/1-page-appearance`，worktree `.scratch/wt/t1`。
本报告是 brief §2.5 与 §7 要的那份文件：**前五片已经各自把 PLAN 段落、CHANGELOG 条目、ADR 全文直接落进了
那四个文件**（见 §6 第 2 条偏离说明），所以这里不重贴草稿，而是把六片合起来报一次状态、闸门、偏离与欠账。
每片的完整叙事在 `PLAN.md` 末尾对应的 `## M12 …` 小节里，决定在 `docs/DECISIONS.md` 头部。

## 0 · 结论

§三十八 六个特性全部交付：`icon` / `cover` / `font`+`full width`+`small text` / `lock` / 模板按钮 + 模板库 /
version history。schema 从 v9 走到 **v14**，每片一列，而**最后一片一条迁移都没加**（`CURRENT_VERSION = 14`
不动）。ADR-0044 … ADR-0050 七条（其中 0046 是一条「决定不做」；那条 0050 与 Track 2 的 mention/date 重号，2026-09-22 由整合者让号为 **ADR-0091**，本文件按写下之时的号记账所以保留 0050）。闸门全绿：**438 passed / 0 failed / 13
ignored**，`cargo check --all-targets` 与 `cargo build --release` 零警告，视觉基线
`.scratch/sweep39`（**83 场景**）。真人一次都没点过这六片里的任何一个 —— 手测清单在 §8。

## 1 · 六片一览

| # | 切片 | ADR | schema | 它的形状，一句话 | passed / ignored | sweep | 场景数 |
|---|------|-----|--------|------------------|------------------|-------|--------|
| 1 | 版式 | 0044 | **v10** `pages.font` + layout 两 bool | 一页的长相存在页上，没有一个块持有字号 | 382 / 12 | 31→32 | 64 |
| 2 | icon | 0045（+0046 不做本地图片） | **v11** `pages.icon` | 存的就是那个 emoji 本身，空槽按位置读成三条 | 388 / 12 | 32→33 | 67 |
| 3 | cover | 0047 | **v12** `pages.cover` | 一个 AttachmentId + 一层固定遮罩，标题对比度量在像素上 | 391 / 12 | 33→35 | 72 |
| 4 | lock | 0048 | **v13** `pages.locked` | 两层门：Rust 拒写 + .slint 拒光标，而拒绝要能听见 | 395 / 12 | 35→36 | 76 |
| 5 | templates | 0049 | **v14** `pages.template` | 模板就是一张页，「看不见」= 不挂树；预置走 §二十六 通道 | 409 / 12 | 36→38 | 80 |
| 6 | version history | 0050 | **不加**（仍 14） | 一个版本是一个 SQLite 文件，缩到只剩那一页 | 438 / 13 | 38→39 | 83 |

## 2 · 这一片（version history）改了哪些文件

新增三个文件：

* `src/storage/versions.rs`（318 行）—— 造/读/列/删一个版本文件。`save` 用 §二十五 `backup::snapshot` 那句
  `VACUUM INTO`，随后 `DELETE FROM pages WHERE id != ?1`（FK 级联带走 `blocks` / `block_children` / `marks`）、
  `clear_index` 清空两张 FTS、`VACUUM` 还空间；`read` 用 `SqliteRepository::from_database` +
  `Repository::load`，也就是 App 打开库的那个 loader。`MAX_PER_PAGE = 20` 在这个文件里，面板的说明句从它
  现算（`src/app/state.rs:322`、`:8547`）。
* `src/core/diff.rs`（408 行，含 10 条测试）—— `compare(before, after)` 剪公共头尾、中段按 block id 做
  LCS，`ALIGN_CELLS = 1 << 18` 之外报告为整页重写；`age_text` 是全应用共用的相对时间读法。
* `ui/components/VersionPanel.slint`（388 行）—— 一个 460 px 宽、高度固定的窗口装两个视图（列表 / 对比），
  底部命名框常驻，Restore 只在看过对比之后出现。

改动：

* `src/app/state.rs` —— 版本 API 全部在 `:3949-4237` 这一段：`version_db` / `page_versions:3956` /
  `version_at:3968`（行号→`(created, label)`，因为 Slint 的 `int` 是 32 位而 unix second 不是）/
  `save_page_version:3987`（先 `persistence_force_flush`，同秒冲突重试四次；剪枝在 `:4017`）/
  `sweep_version_files:4067` / `version_diff:4087` / `restore_version:4116` / `delete_version:4154` /
  `version_pinned_attachments:4178` / `forget_versions:4189`（`delete_page` 对每棵子页调用）。四个纯投影
  `version_rows` / `versions_note` / `version_heading` / `version_diff_note` 在 `:298-345`，运行时与
  headless 场景共用它们。`reclaim_attachments:2329` 多了一项（`:2361`）。
* `src/app/controller.rs` —— `:648` 起是四个 handler（save / view / restore / delete），`:104-105` 推两个
  model，`:537` 是菜单那一行的入口；`apply_scene` 里三支新场景走 `seed_versions` + 同一批投影。
* `src/storage/repository.rs` —— 只加 `from_database`（+10 行）：一个「已经开好的库」的读把手，不转 `.bak`
  家族、不走损坏文件流程、`path` 留 `None`，因此写侧不可能指到它。
* `src/core/mod.rs` / `src/storage/mod.rs` —— 各加一行 `pub mod`。
* `ui/Types.slint`（+58）—— `VersionRow` / `DiffRow` 两个 struct 与 10 个 UIState 属性 + 4 个 callback。
* `ui/AppWindow.slint`（+21）—— 挂 `VersionPanel`，页面 ⋯ 菜单长到十四行。
* `tests/integration/backup_test.rs`（+429）—— 8 条（7 支跑 + `version_cost` 带 `#[ignore]`，它是那三个磁盘
  数字的来源，留在树上是因为帽是别人将来要重量的数）。
* `benchmarks/scripts/sweep.ps1` —— 只往 `$all` 追加三个场景名。

## 3 · 前五片改了什么

不重复：每片的文件清单、行号、接缝与像素读数以 `PLAN.md` 的 `## M12 版式 / 图标 / 封面 / 锁定 / 模板` 五节
为准（它们是各自落地当轮写的，行号对得上当轮 commit：`f580939` 版式、`62a86c2` icon、`78ddf35` ADR-0046、
`cb57779` cover、`90d7d23` lock、`a3ce47b` templates）。

## 4 · 跑过的命令与关键输出

```
cargo check --all-targets        → 0 warnings
cargo test --all-targets         → 438 passed / 0 failed / 13 ignored（逐 target 相加）
cargo build --release            → 0 warnings
cargo build --features software --bin quire-shot → ok
grep -rn "#\[test\]" src tests | wc -l → 451 = 438 + 13   （与上一片在册的 421 差 30 = 本片新测试）
benchmarks/scripts/sweep.ps1 -OutDir .scratch/sweep39 -Baseline .scratch/sweep38
benchmarks/scripts/diffbbox.ps1 -OldDir .scratch/sweep38 -NewDir .scratch/sweep39
cargo test --release --test backup version_cost -- --ignored --nocapture
```

`version_cost` 的关键行（夹具 201 页 / 17 000 块 / 库 9 888 480 B）：60 行页版本 **122 880 B = 1%** /
1 031 ms，5 000 行页 **790 528 B = 7%** / 469 ms，**20 个长页版本 15 810 560 B = 159%** / 643 ms each；
Debug 臂 614 / 469 / 577 ms —— **字节跨 profile 相同、毫秒不可比**。修 FTS 之前同夹具是 1 560 576 /
2 228 224 / 44 564 480 B（15% / 22% / 450%）。

## 5 · 门槛结果

**测试**：30 条新测试（lib 22 = `app::state` 12 + `core::diff` 10；storage 8，其中 1 支 `#[ignore]`）。最不容
易被替代的几条是：版本以**点击时**的页为准而非上次 flush、无名版本按「第几版」被命名、被丢掉的是最旧的、版本
活得过它自己的 session、锁页拒绝它被要求的那次恢复、只在原页恢复、删页忘掉版本、版本钉住图片、面板的行与句子
读出同一批数字、以及 `a_version_is_the_library_narrowed_to_one_page` 里那两行 `_data` 断言（见 §6 第 1 条）。

**像素**：sweep38 → sweep39 动 **2 of 80**、新增 **3**（83 张）。两个动过的都是页面 ⋯ 菜单在两个锚点上的两种
数字：`menu.png` **5 个采样像素**在单列 x 414 / y 672..680（弹窗早被 `min(rows*30+8, …)` 夹住，只剩滚动条
滑块），`page-lock-menu.png` **575 px** 在 x 240..422 / y 558..670（十四行全画出来，新行以下都位移）。三张
新场景各自对 `default.png`：`page-versions` 7 689 px、`page-versions-diff` 8 726 px，都停在面板自己的盒子
**x 410..868 / y 162..638** 里（背后那页零像素）；`dark-page-versions` 重画整帧 255 505 px，与所有 `dark-*`
一样。

**性能**：这一片没有 per-row / per-block / per-frame 路径，所以不欠两臂 bench；欠的是 §三十八 明写的两个数
字，量出来写在 `docs/PERFORMANCE.md:1540-1626`（上表 + `size_of::<Block>() = 136 B`、5 000 行 = 680 000 B、
LCS 表 1 MiB、每连接 SQLite 默认 −2000 KiB ≈ 2 MB、留二十个版本花 0 字节 RAM）。

## 6 · 偏离 brief 的地方，和原因

1. **抓到一处真缺陷，代价是这一片多了一次全量重跑**：清空 FTS 的第一版只有 `DELETE FROM search_blocks` /
   `search_pages`，测试同意（两张表 0 行），而"只有一页"的版本文件重 **1 560 576 字节**——FTS5 的词表在
   `<table>_data` 那棵 b-tree 里，普通 delete 会走过去并把它留在原地（实测 368 行 / 1 441 792 字节）。现在
   `clear_index` 跑 `DELETE` + FTS5 的 `rebuild`（`delete-all` 更短，SQLite 在这里直接拒：它只服务
   contentless / external-content 表）。断言换了地方：钉 `_data` 的行数，因为「读者会查的那两张表的行数」正
   是说谎的那个。
2. **ADR 号段超了一位**：brief §2.4 给的是 0045…0049，实际用了 0045…0050。多出的一位是 ADR-0046——它不是一
   个特性，是「icon 不做本地图片，图片归 cover」这条决定的记录（§三十八 原文把 icon 与本地图片写在一起，做
   到那一片时必须显式决定并定价）。号不撞、不跳。
3. **`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` 被直接改了**：brief §2.5 把这三个文件留给整合者，同时把
   整合者认作「Track 1 或用户指定的主 agent」，而本会话就是被指定的那一个，所以每片落地时顺手把条目写进去
   （PLAN 末尾追加那一节本来就是 §2.5 明许的例外）。要退回「只有整合者改」的形状：这四类内容都能从
   `git diff` 里按文件逐条摘出，摘走即可，代码不依赖它们。
4. **`docs/SPEC.md` 动了既有条目的中文注解**（§三十八 的三条 version history + M12 清单那一行）。章节号一个都
   没改（memory 里那条纪律），只是把「未做」改成「已交付 + ADR 号」。
5. **手测为零**：见 §8。这是 brief §3.6 要求诚实列出的那一类，不是偏离范围，但比前几片更大——这一片的入口是
   一个输入框，headless 场景能画两个视图却不能打字。

## 7 · 给整合者的注意事项

* **已落文档清单**（本轮，六处）：`docs/DECISIONS.md` 头部 ADR-0050（现 ADR-0091）；`docs/PERFORMANCE.md` 新节
  `:1540-1626`；`docs/SPEC.md` §三十八 三条 + M12 清单；`docs/ROADMAP.md` M12 行翻成「✅ delivered」+ 那一
  格末尾的中文写法 + 文件末尾一段英文叙事；`CHANGELOG.md` 三条（Editor 的功能条、reclaim 那条把版本 pin 列
  进引用者、Persistence 新条说明版本与 §二十五 共享机制不共享生命周期）；`docs/UI_ARCHITECTURE.md` 场景清单
  + 新节叙事 + 基线行改成 `sweep39`（83）；`PLAN.md` 末尾新节；`docs/WORKTREE_TRACKS.md` §5/§7 改成六刀 +
  `sweep39`，并注明 **Track 1 的迁移优先级在这一片没有花掉**。
* **push 形状**：`master` 与 `origin/master` 都在 `a3ce47b`（前五片已收口），本片是一次**快进**。按 §6 的做
  法：本分支 commit → `git branch -f master <sha>` → `git push origin master:master`；不要在主工作树
  checkout（那里是 `track/3-database` 且有活着的 agent）。
* **schema**：`CURRENT_VERSION = 14` 不变，`src/storage/migrations.rs` 这一片一个字没改。Track 3 那份草案不
  需要因为本片往上挪号——但注意 v14 已经被 `pages.template` 花掉了（ADR-0049）。
* **回收扫描的语义变了**：`AppState::reclaim_attachments` 现在把版本文件指着的图片算作引用者，所以同一份库在
  有版本和没版本时，那行报告的字节数会不同。有测试钉住（`a_version_pins_the_picture_its_page_no_longer_shows`）。
* **`versions/` 是库旁边的第三个存放处**（另两个是 `attachments/` 与 `.bak<N>` 家族）。它**没有**被加进 §二十
  五 的备份轮转（也不该被：一个版本由用户命名，不许按年龄剪），也**没有**被 Inno 安装器 / 打包脚本当作要带的
  东西。要不要让一次"拷走整个库"带上版本，是整合者的决定，不是我的越界。
* **模板那一片改了 Markdown 导出的行为**（空的列表项过去整行被丢掉，现在写裸标记 `-` / `>` / `#` / `1.`），
  那是**所有页面**的导出，不只是模板；理由与规则在 §二十六 正文与 `src/services/export_service.rs` 文件头。
* **`.scratch/` 按 worktree 隔离**：`sweep39` 这 83 张只存在于 `.scratch/wt/t1/.scratch/` 里。整合者在别的
  checkout 跑 sweep 时拿不到它，别把「跑不出同名目录」读成丢失。

## 8 · 未验证 / 已知边界（六片累计）

**欠真人手的（一次都没点过）**：
1. 版式片：切三种字体 + full width + small text 后长文档滚动的观感。
2. icon 片：emoji 在真 GPU 上的彩色绘制（headless 那一臂是 skia-software，字体回落链与真机不同）。
3. cover 片：真换一张封面（`rfd` 文件对话框，headless 到不了）、遮罩在任意图片上的可读性。
4. lock 片：那个 pill 与通知条在真人眼前的表现（数据层与拒绝路径有测试）。
5. templates 片：整个 Templates 子菜单，其中 Export / Import 两行走 `rfd`。
6. version history 片：⋯ → Version history → **往输入框里打字** → Save → 点一行 → Restore → Ctrl+Z。场景能
   画两个视图但不能打字，所以 focus-on-open 与「一次恢复一次撤销」的体感仍是空的。

**形状性的边界（是决定，不是没做完）**：版本只含**正文**，标题 / icon / 封面 / 字体 / 锁定不随恢复滚回；没有
自动快照、没有并排视图、没有词级 diff；`versions/` 不出现在应用说出口的任何磁盘数字里；LAN 分享不带版本也不
带模板；icon 不接本地图片（ADR-0046）；模板不是权限（不加密、不为「另一个用户」隐藏）；保存模板从不覆盖，所以
库里会积累同名副本，整理入口只有 Delete。

**代码里已知会拒绝的输入**：没有数据库文件的 session（版本面板是空的，不报错）、锁页的一切写入、翻页之后残留
的恢复点击、同第五个以上的连点版本（回答「这一秒已存在」而不是覆盖）、`read` 打不开的文件（损坏的版本文件报
一句 notice，不会把 App 带走）。

## 9 · 看到但没碰的、别人 territory 里的问题

* `SqliteRepository::replace_all`（`src/storage/repository.rs:400-430`）清空两张 FTS 用的也是裸 `DELETE`，同
  一形状。它的代价与本片不同：那条路径紧接着把整库重插一遍，文件本来就有那么大，所以残留只是上一个状态的词，
  不会凭空多出 1.4 MB 别人页面的文字；而且那里没有 `VACUUM`。我只报不动：这个文件本轮只加了 `from_database`，
  而 bulk 路径的清仓语义归 Track 3 那一侧的形状。若整合者想统一，`clear_index`（`src/storage/versions.rs:139`，
  现在是模块私有）抬成 `pub(crate)` 就能搬。
* `benchmarks/scripts/sweep.ps1` 是四轨共享的场景清单，本轮只追加三个场景名，没动任何逻辑；若合并时它冲突，
  顺序无关紧要，`$all` 是一个平列表。
