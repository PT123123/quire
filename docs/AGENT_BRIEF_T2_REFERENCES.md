# Agent brief · Track 2 — 引用、提及与反向链接（SPEC §四十 / M13）

你是 **Quire** 的 Track 2 agent。Quire 是一个本地优先、Rust + Slint 1.18 的
Notion 式笔记应用：单进程、无 WebView、无 JS/WASM 运行时、没有网络客户端。
本文件是你的全部范围定义。**只做本文件列的五个 slice**。

---

## 1 · 开工前必读（按顺序，别跳）

| 文档 | 你要拿走的东西 |
|------|----------------|
| `docs/ARCHITECTURE.md` | 分层与六条硬规则（UI 不碰 SQL/IO、核心不知道颜色、状态全在 `UIState`、语义操作一律走 Command） |
| `docs/SPEC.md` **§四十**（`四十、第二十六阶段：引用、提及与反向链接`，行 ~2537 起） | 你的需求原文，四段：`@page mention` / `@date` / `反向链接区` / `页面别名` |
| `docs/SPEC.md` **§二十**（搜索索引）、**§十**（Inline Model）、**§二十二**（性能预算）、**§二十六**（Markdown 通道）、**§三十七 批次 C**（`synced block` 那一行） | 你每一刀都要撞的约束 |
| `docs/DECISIONS.md` **ADR-0026** | Page / Link-to-page 块怎么共享 `blocks.page_ref`、以及「引用存 id、悬空引用不得炸库」这条契约。你的 mention 必须与它同构 |
| `docs/DECISIONS.md` **ADR-0014**（FTS5 索引 + 异步搜索）、**ADR-0029**（悬空引用加载成「缺失」而不是失败）、**ADR-0028**（折叠子树零 realized row）、**ADR-0039**（TOC 每次投影现算、派生数据不入库）、**ADR-0041**（带标记的行按词断行，一个 mark 是一格） | 你的五个 slice 会反复引用它们 |
| `docs/EDITOR_ARCHITECTURE.md` | 命令系统、单个 `TextEdit`、`runs` 通道与渲染契约 |
| `docs/UI_ARCHITECTURE.md` §State flow / §Token discipline / §Slint geometry traps / §Slint language traps | 加组件与踩坑清单 |
| `docs/PERFORMANCE.md` §Method | 「量什么才算通过」 |
| `PLAN.md` 最后三节 | 你的收尾报告要照这个文体与信息量写 |
| `docs/ROADMAP.md` §Definition of done | 五条，缺一条不算完 |

---

## 2 · 并行纪律（同一仓库同时有四条 track，必须照做）

### 2.1 分支与提交

- 一条 track 一个分支：`track/2-references`，从 `master` 起。
- **一个 slice 一个提交**，只 `git add` 你这一刀明确改到的文件，**永不** `git add -A` / `git add .`。
- 永不做 `git checkout` / `switch` / `restore` / `reset` / `stash` / `rebase`。
- 只在 `cargo check --all-targets` 干净时才提交。不 push（除非用户明确说了 push）。

### 2.2 四条 track 的文件归属

| Track | 主题 | 主要 territory |
|-------|------|---------------|
| 1 | 页面外观 | `Page` 结构体、`pages` 列、`core/icon.rs`、封面/锁/模板/版本历史、`SidebarItem`/`IconPicker` |
| **2（你）** | 引用与反向链接 | `MarkKind` / `Mark`、`core/{mention,reference}.rs`(新)、`storage/search_index.rs` 的增量索引、页面底部反向链接面板、`@` 选择器 |
| 3 | Database | 新模块为主（`core/database*`、`storage/database*`、`ui/components/Database*`）+ 新迁移 |
| 4 | 欠账清零与 RC 收官 | `src/platform/**`、`install/**`、`benchmarks/**`、`Cargo.toml` 依赖、弹窗滚动 |

### 2.3 共享接缝（四个热文件）——本 brief 最重要的一节

1. `src/core/types.rs` — **只追加**。`MarkKind` 的新变体加在枚举末尾、`as_str` / `try_from_str` 各加一行，**永不重编号、永不重排**；`Mark` 加字段放在结构体末尾并写清空值语义。注意 `marks` 表的 `url TEXT NOT NULL DEFAULT ''` 这一列已经存在（`MarkKind::Link` 在用），**mention 的载荷是复用它还是加列，是你第一个要出的 ADR**。
2. `src/storage/migrations.rs` — **串行接缝，先到先得**。runner 按数组顺序应用且不允许跳号：你的新步 `version` = **提交那一刻**的 `CURRENT_VERSION + 1`；提交前重新读一次这个文件，号被占了就改号并同步改测试名。迁移体走 `add_page_columns()` 那种「缺哪列补哪列」的范式（v10/v11 已立好），并配「vN-1 库升上来读回原值」的断言。
   > `marks` 表主键是 `(block, start, kind)`：**同一块、同一起点、同一种 mark 只能有一条**。一个 mention 与一个 link 落在同一位置是否可以共存，是你要在 ADR 里回答的问题（它决定要不要改主键，而改主键是一次真迁移）。
3. `ui/Types.slint`（`UIState`）与 `src/app/controller.rs` — 只加你自己的 property / callback / match 分支，不重排既有行；`apply_scene` 只追加你的场景。
4. `ui/components/Editor.slint`、`ui/AppWindow.slint` — 最热的两张图。**你的区域**：页面底部（反向链接面板）、正文里的 mention chip、`@` 选择器弹窗、悬停/点击行为。Track 1 在**标题与封面那一带**（页首）动手，你们不要顺手整理对方的区域，也不要整段重排。

### 2.4 ADR 编号配额

ADR-0001…0044 已占用。**你的号段是 ADR-0050…0059**，按顺序取，一条非显然决定一个 ADR，不重号不跳号。四条 track 同时追加，所以 **ADR 直接追加在文件末尾**，不要插到中间。

### 2.5 只有整合者能改的文件

`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` 由整合者统一收口，**你不直接改**。把下面四样写进 `docs/REPORT_TRACK2.md`（新文件，只有你写）：
1. 每个 slice 的 PLAN 段落草稿（中文，照 `PLAN.md` 最后三节的文体）；
2. CHANGELOG 条目草稿（英文 bullets）；
3. ADR 全文（含编号）；
4. 未验证边界与给整合者的注意事项。

> 例外：`PLAN.md` 允许你**在末尾追加** `## Track 2 · <slice 名>` 小节。

### 2.6 环境坑（本机实测，别踩第二遍）

- 本机把 **bash 的 `rm -rf`** 重定向到回收站：批量删除（阈值 50）返回非 0，stderr 只有一行 `SAFE_DELETE_BULK_CONFIRM_REQUIRED`，文件原样保留；**返回非 0 会让 `rm -rf x && next` 整条短路**，`next` 静默不执行。清目录走脚本自带路径或 `just clean`；不要把清理与后续步骤用 `&&` 串起来。
- 重定向到文件的日志，命令没执行时**既不创建也不截断**，`tail` 读到的是上一次的陈旧内容。用带时间戳的独立文件名，或先看退出码。
- 同一文件**不要在同一条消息里发多个 `Edit`**（互相覆盖）；同一文件串行、不同文件可并行。
- 改完必须**真跑一遍**，不要只信编辑返回的 success。
- 测试建目录一律用 `quire::testing::ScratchDir`；跑数据只写 `.scratch/`，**绝不碰 `%APPDATA%\Quire` 或仓库旁的 `appdata/`**；GUI 验证用 `--db .scratch/<dir>/quire.db --auto-exit <secs>`。

---

## 3 · 任务清单

顺序即依赖顺序（T2.4 随 T2.1 一起，T2.5 最后）。

### T2.1 · `@page mention`（本 track 的地基）
**需求原文**：§十 的 Inline Model 新增一种 span（**存目标 page id，不存标题**）。输入 `@` 弹页面选择器，**复用 §十五 slash 弹窗的第三种模式**（ADR-0026 已验证这条弹窗可复用）。

- **载荷与存储形状**（你的第一个 ADR）：复用 `marks.url`（写成 `quire://page/<id>` 这类可解析形式）还是加列？两种都能自洽，但必须写下来并说明代价（`url` 复用省一次迁移，代价是 link 与 mention 共享一个字段的语义）。
- **`@` 触发**：在三种编辑面里都要能用（块、表格单元格、分栏内的行），照 ADR-0042 里 Ctrl+M 的接法确认这三处都在。选择器要**可过滤、要量自己的高度**（这个项目在「菜单高度写死导致跑出窗口」上栽过一次，见 ADR-0032 那条修复），并复用既有的页面选择器交互（`Move to` / `Link to page` 已有）。
- **渲染**：一个短 chip（页面图标 + 当前标题）。**显示的是 title，存的是 id** —— 改名后所有引用自动跟着改，这是「页面别名」那一条的实现，不是额外功能。渲染走 ADR-0041 的 runs 通道（一个 mark 是一个 layout 格、不可断行），所以 chip 必须短（过长要截断，别让一个长标题把整行挤爆）。
- **点击**：应用内跳转到目标页（复用既有的 `quire://page` 跳转路径）。
- **悬空引用**：目标页被删掉之后，这个 span 必须**可见地退化**（明确的「已删除」样式或文本），**不得**让投影失败、不得让整页打不开 —— 这就是 ADR-0029 对附件、ADR-0026 对 page_ref 立的那条契约。要有测试钉住。
- **Markdown 往返**（§二十六 内容通道）：决定语法并出 ADR。要求是「读回来还是它自己」，且在别的渲染器里读起来也不难看；导入侧要能把这个语法认回 mention（`@date` 同理，一起定）。
- **接线清单**（这个是本项目的硬性约束，缺一处视为未完成）：`core/types.rs` 的 `MarkKind` → storage 的 kind 字符串 → Markdown 导入 + 导出 → 行内 runs 渲染 → 截图场景。**mark 没有「Turn into / slash 菜单」这两个接点**（那是块的），但截图场景与导入导出一个都不能少。

### T2.2 · `@date`
**需求原文**：落 date 型 inline span。

- 存储形状：ISO 日期字符串（`YYYY-MM-DD`）还是带时区的时间戳？**纯本地应用没有账号也没有云端时区**，选一个并写进 ADR；关键是十年后读回来还认得。
- 输入路径：`@` 选择器里除了页面，还要有一个日期项（选择器现在是「页面」，现在要多一种 —— 形状自己定，但别做成两个互相抢 `@` 的入口）。
- 渲染：一个短 chip；**不跟随"今天"变化的那部分（比如"3 天后"）如果做了，必须是绘制时算的派生值，不入库**（照 ADR-0039 TOC 那条纪律）。
- Markdown 往返与 T2.1 同一次定。

### T2.3 · 反向链接区（页面底部）
**需求原文**：页面底部列出所有引用本页的块。**派生数据，不双写入库**；**反向链接索引与 §二十 的搜索索引一起增量维护，不得每次打开页面全库扫描**。

- 先读 `src/storage/search_index.rs`、ADR-0014、`docs/SPEC.md §二十`：FTS5 两张虚表（`search_pages` / `search_blocks`）已经是「写区块时就更新」的增量路径。反向链接**挂进同一条路径**（同一张索引表加一列，或一张由它维护的派生表），不要另造一套扫描器。
- **「不得全库扫描」是硬要求**，要有证据：构造一个「页数足够多、引用足够多」的库，量**打开一个页面的耗时**与**打开一个没有任何反向链接的页面的耗时**，两臂都进 `docs/PERFORMANCE.md`。
- 面板是**派生投影**：和 TOC 一样，每次投影现算，不入库。它的 per-projection 成本要照 ADR-0039 的做法量（那一次量出 10 000 行投影 ≈38 ms 与目录 1 000 行无差别的结论，手法可以直接借）。
- 交互：点一条 → 跳到来源块（复用 M8 就有的 `quire://block` 锚点路径）。**已知限制**：锚点移动光标但**不滚动视口**（Slint 1.18 的普通 `ListView` 没有 `bring-into-view`），TOC 与所有锚点共享这条限制 —— 在本 slice 里**如实写下来**即可。
  > **可选加餐（要出 ADR + 性能数字）**：做一个「变高行的 reveal」，一次把锚点跳转、TOC 点击、反向链接点击三处一起修好。这不是本 slice 的必需项，做了要说清它动了哪些行、成本多少。
- 计数/折叠：一份页面被引用 200 次时长什么样？给一个「列表 + 折叠」的形状，别让面板把正文挤没。

### T2.4 · 页面别名与悬空引用（横切，随 T2.1 落地）
**需求原文**：因为引用存的是 ID，重命名后所有引用自然显示新标题。

- 这一条**不需要新功能**，需要的是**证明**：改名后侧栏、mention chip、反向链接区、Markdown 导出同时跟着变，一条测试或一组截图读数钉住。
- 同时钉住另外三件事（都属于「引用存 id」的代价，必须明确定义）：目标页**被删**、目标页**被移进别的父页**、目标 id **指向一个本库不存在的页**（手改过数据库 / 旧备份）。三种情况都必须**可见退化**，不得崩、不得静默显示错的标题。

### T2.5 · `synced block`（同步块）
**需求原文**：§三十七 批次 C「synced block：依赖 §四十 的引用基础设施，排在它之后」—— 现在 §四十 就是你，所以它是你的最后一刀。

- **先出 ADR**，因为它动摇的是最核心的假设：一份内容出现在两个位置，谁拥有它？Notion 的语义是「引用同一个块子树」，本项目最省的同构做法是一个块引用另一个块（照 ADR-0026 `blocks.page_ref` 的形状，再借 ADR-0028 折叠子树那套「投影时把行的生死算清」）。
- 必须回答并写下来：**删除语义**（删掉引用方 / 删掉被引用方分别发生什么）、**undo 语义**（两处同时变，一步撤销的是什么）、**环检测**（A 引用 B、B 又引用 A —— 参照 SPEC §三十九 对 relation 的要求：**保存在保存时做检测，不在渲染时做**）、**Markdown 导出**（摊平还是原样两次？摊平更安全，照 ADR-0032 columns 的先例）。
- **硬性约束（凡「行是动态的」块都要多改两处，SPEC §三十七 明写）**：① `project_blocks` 必须真的把不该出现的子树从 rows 里删掉，**不是**留一个 `visible: false` 的 delegate（`visible: false` 拦不住 binding，这个坑 ADR-0042 记过）；② 凡是拿 row index 当 model index 用的地方（现在是 §八 的拖拽落点，而且可能更多）都要做一次 row→model 换算。
- 新块种类要**同时改到六个接点**（SPEC §三十七 硬性约束，少一处即视为未完成）：`core/types.rs` 的 `BlockKind` → storage 的 kind 字符串 → Markdown 导入 + 导出 → ⋮⋮ 的 Turn into → slash 菜单 → 截图场景。

---

## 4 · 每个 slice 都要过的门槛

1. `cargo check --all-targets` 干净、`cargo test --all-targets` 全绿（报数：passed / failed / ignored，按 target 分开报）、`cargo build --release` 零警告。
2. **视觉**：新场景加进 `apply_scene`（含 `dark-*` 臂，mention / date / 反向链接面板 / 悬空引用都要有臂），然后
   `benchmarks/scripts/sweep.ps1 -OutDir .scratch/sweep<N> -Baseline .scratch/sweep<N-1>`，
   再用 `benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` 出 bbox 表，**逐个场景说明谁动了、动在哪、为什么是它的改动解释的**。当前基线是 `.scratch/sweep32`（64 场景），你落地时的最新基线以那一轮为准。
3. **性能**：给每行/每次投影/每次开页加了成本就必须量，并写进 `docs/PERFORMANCE.md`；**必须有对照构建**（前一刀的 commit 在干净 worktree 里编出来，两臂交替跑，各自先自我标识 md5/size）—— 这个项目把四次「同向上涨」记成过「会话漂移」，后来靠同会话对照才排除掉。若确实没有 per-row 成本，明确写「不欠 RAM 闸」+ 替代证据（像素读数或形状论证），不要沉默跳过。
4. `docs/SPEC.md` 对应小节标注已交付 + ADR 号（照 §三十七 各条的写法）。
5. `docs/REPORT_TRACK2.md` 更新（PLAN 段落 / CHANGELOG 条目 / ADR 全文 / 未验证边界）。
6. 未验证项**诚实列出**：headless 场景证明不了的交互（`@` 选择器的键盘操作、chip 的悬停、真点一次跳转）、需要真人手感的，全都写下来。未完成但诚实上报，胜过看起来完成。

---

## 5 · 明确不做 / 不要碰

- **`bookmark` 不是你的**（M11 批次 C 欠的那一条）。它需要网络客户端，属于 Track 4 —— 不要顺手做掉，也不要改 `core/embed.rs` 的卡片形状。
- **M9 Android 仍然停着**（用户 2026-09-20 明确决定）。不要为它改 cfg、加依赖、动 `platform::data_dir`。
- 不引入 WebView、JS/WASM 运行时、网络客户端、云同步、协作、评论、发布站点（SPEC §一/§三十三）。
- 不要动 `[profile.release]`（ADR-0024 钉住的）。
- 不要改 Track 1/3/4 的功能代码；发现那边的 bug → 报出来，不要顺手修。
- 反向链接**不要**做成「每次打开页面全库扫描」的偷懒版本（SPEC 原文禁止）；也不要为了省事把反向链接**双写进库**（SPEC 原文禁止）。

---

## 6 · 报告格式

每个 slice 完成后（以及最后）报告一次：

- **改了哪些文件**：路径 + 为什么，关键行号；
- **跑了哪些命令**，输出是什么（粘关键行）；
- **门槛结果**：测试数、sweep 对比表、性能数字（或「不欠」的理由）；
- **偏离本 brief 的地方和为什么**（偏离本身不是错，隐瞒才是）；
- **给整合者的注意事项**：要折进 PLAN / CHANGELOG / DECISIONS / PERFORMANCE 的内容；
- **你没碰但注意到的别人 territory 的问题**。

做不完就诚实报状态。**一个未完成但诚实上报的 slice，胜过一个悄悄改坏了范围的 slice。**
