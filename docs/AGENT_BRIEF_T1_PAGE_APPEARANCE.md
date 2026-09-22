# Agent brief · Track 1 — 页面外观收尾（SPEC §三十八）

你是 **Quire** 的 Track 1 agent。Quire 是一个本地优先、Rust + Slint 1.18 的
Notion 式笔记应用：单进程、无 WebView、无 JS/WASM 运行时、附件与数据都在本机。
本文件是你的全部范围定义。**只做本文件列的五个 slice**，其余是另外三条 track 的。

---

## 1 · 开工前必读（按顺序，别跳）

| 文档 | 你要拿走的东西 |
|------|----------------|
| `docs/ARCHITECTURE.md` | 分层与六条硬规则（UI 不碰 SQL/IO、核心不知道颜色、状态全在 `UIState`、语义操作一律走 Command） |
| `docs/SPEC.md` **§三十八**（`三十八、第二十四阶段：Page 外观与属性`，行 ~2433 起） | 你的需求原文。`icon` / `cover` / `lock` / `version history` / `模板` 五节 + `## 本阶段不做` |
| `docs/SPEC.md` **§十八**（迁移）、**§二十一**（可读性/对比度）、**§二十二**（性能预算）、**§二十五**（快照）、**§二十六**（Markdown 通道） | 你每个 slice 都要撞的约束 |
| `docs/DECISIONS.md` **ADR-0044** | 上一个 slice（font / full width / small text）怎么落的、为什么 `PageType` 是派生的、为什么长相不进 undo。你的五个 slice 必须与它同构 |
| `docs/EDITOR_ARCHITECTURE.md` | 命令系统、单个 `TextEdit`、渲染契约 |
| `docs/UI_ARCHITECTURE.md` §State flow / §Token discipline / §Slint geometry traps / §Slint language traps / §Adding a component | 加组件与踩坑清单 |
| `docs/PERFORMANCE.md` §Method | 「量什么才算通过」 |
| `PLAN.md` 最后三节 | 你的收尾报告要照这个文体与信息量写 |
| `docs/ROADMAP.md` §Definition of done | 五条，缺一条不算完 |
| `docs/AGENT_BRIEF_M8_TAIL.md` | 上一轮多 agent 协作的纪律范例 |

---

## 2 · 并行纪律（同一仓库同时有四条 track，必须照做）

### 2.1 分支与提交

- 一条 track 一个分支：`track/1-page-appearance`，从 `master` 起。
- **一个 slice 一个提交**，只 `git add` 你这一刀明确改到的文件，**永不** `git add -A` / `git add .`。
- 永不做 `git checkout` / `switch` / `restore` / `reset` / `stash` / `rebase`（它们会连带别人的改动）。
- 只在 `cargo check --all-targets` 干净时才提交。
- 不 push（除非用户在这次对话里明确说了 push）。

### 2.2 四条 track 的文件归属

| Track | 主题 | 主要 territory |
|-------|------|--------------|
| **1（你）** | 页面外观 | `src/core/types.rs` 的 `Page`、`src/core/icon.rs`、`src/core/cover.rs`(新)、`src/storage/migrations.rs`、`src/app/{state,workspace,controller}.rs` 的页属性 setter、`ui/components/IconPicker.slint`、`ui/components/SidebarItem.slint`、`ui/Typography.slint` |
| 2 | 引用与反向链接 | `MarkKind`、`src/core/{mention,reference}.rs`、backlink 索引、页面底部面板 |
| 3 | Database | 新模块为主（`core/database*`、`storage/database*`、`ui/components/Database*`）+ 新迁移 |
| 4 | 欠账清零与 RC 收官 | `src/platform/**`、`install/**`、`benchmarks/**`、`Cargo.toml` 依赖、弹窗滚动 |

### 2.3 共享接缝（四个热文件）——这是本 brief 最重要的一节

四条 track 都会碰到这四个文件，冲突只会出在这里，规则如下：

1. `src/core/types.rs` — **只追加**。新增枚举变体只加在末尾，**永不重编号**、永不重排、永不改既有变体的 `as_str`；不要整段重格式化。给 `Page` 加字段时放在结构体末尾，并在文档注释里写清空值语义（照 `icon` 那一版的写法）。
2. `src/storage/migrations.rs` — **串行接缝，先到先得**。runner 按数组顺序应用且不允许跳号，所以你的新步 `version` = **提交那一刻**的 `CURRENT_VERSION + 1`；每次提交前重新读一遍这个文件，号被占了就改号并同步改你的测试名。迁移体一律走 `add_page_columns()` 那种「按列判存、缺哪列补哪列」的形状（v10/v11 已立好范式），并配一个「vN-1 库升上来读回原值」的断言（照 `the_v10_step_adds_the_page_look_to_a_v9_database`）。
   **本文件你享有优先权**：在途的 v11（`pages.icon`）归你，`cover` / `locked` 也在你手里。
3. `ui/Types.slint`（`UIState` 全局）与 `src/app/controller.rs`（回调分发）—— 只加你自己的 property / callback / match 分支，不重排既有行。`controller.rs` 的 `apply_scene` 是追加场景，不要改别人的场景。
4. `ui/AppWindow.slint`、`ui/components/Editor.slint` —— 最热的两张图。**只在你自己的区域改**（你的区域：顶栏 ⋯ → Style 子菜单、封面带、锁定态提示条、模板入口），不要顺手整理别处，不要整段重排。

### 2.4 ADR 编号配额

`docs/DECISIONS.md` 里 ADR-0001…0044 已占用。**你的号段是 ADR-0045…0049**，按顺序取，一条非显然决定一个 ADR，不重号、不跳号。
四条 track 同时往后追加，所以：**ADR 直接追加在文件末尾**，不要去插入到中间（中间插入必然四方冲突）。

### 2.5 只有整合者能改的文件

`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` 由整合者（Track 1 或用户指定的主 agent）统一收口，**你不直接改**。
你要把下面四样写在自己的报告文件 `docs/REPORT_TRACK1.md`（新文件，只有你写）里：
1. 每个 slice 的 PLAN 段落草稿（中文，照 `PLAN.md` 最后三节的文体：缘起 / 存成什么形状 / 接缝 / 像素读数 / 验证 / 未验证边界）；
2. CHANGELOG 条目草稿（英文 bullets，落进 `### Editor` / `### Workspace` 对应小节）；
3. ADR 全文（含编号）；
4. 未验证项 / 已知边界，以及给整合者的注意事项。

> 例外：`PLAN.md` 允许你**在文件末尾追加**你的 `## Track 1 · <slice 名>` 小节。这是可合并的，且省整合者一道手。

### 2.6 环境坑（本机实测，别踩第二遍）

- 本机把 Node 的 `fs.rmSync` 与 **bash 的 `rm -rf`** 都重定向到回收站：批量删除（阈值 50）会返回非 0，stderr 只有一行 `SAFE_DELETE_BULK_CONFIRM_REQUIRED`，文件原样保留；**返回非 0 会让 `rm -rf x && next` 整条短路**，`next` 静默不执行。所以：不要用 `rm -rf` 清目录，清理走脚本自带的清理路径或 `just clean`；不要把清理动作和后续步骤用 `&&` 串起来。
- 重定向到文件的日志，命令没执行时**既不创建也不截断**，`tail` 读到的是上一次的陈旧内容。落盘日志用带时间戳的独立文件名，或先看退出码。
- 同一文件**不要在同一条消息里发多个 `Edit`**（会互相覆盖），同一文件的编辑串行、不同文件可并行。
- 改完必须**真跑一遍**（`cargo check` / 测试 / shot），不要只信编辑返回的 success。

### 2.7 测试与临时数据

- 新测试挂进已注册的 suite（`tests/integration/*.rs`，见 `Cargo.toml` 的 `[[test]]`）或模块内 `#[cfg(test)]`。**不要增删 `[[test]]` 条目**。
- 测试要建目录一律用 `quire::testing::ScratchDir`（它结束时自删）。
- 任何跑数据库/日志/附件的操作都写在 `.scratch/` 下；**绝不碰 `%APPDATA%\Quire` 或仓库旁的 `appdata/`**。
- GUI 验证用 `--db .scratch/<dir>/quire.db --auto-exit <secs>`。

---

## 3 · 在途状态：你的第一件事是把 icon 切片收口

工作树里有一份**已实现但未提交**的 `icon` 切片（`git status` 里 24 个文件 modified + `src/core/icon.rs`、`ui/components/IconPicker.slint` 两个 untracked）。已经落到的程度：

- Schema **v11**（`CURRENT_VERSION = 10 → 11`），`pages.icon TEXT NOT NULL DEFAULT ''`，走新的通用 `add_page_columns()`；`add_page_appearance_columns` 已改写成调用它。
- `core::icon`：96 个 emoji 的 `PICKER`（8 个一行，无分类无搜索 —— 这是这一刀写下来的边界）、`initial(title)`（标题首字符占位）、`slot(stored, title)`（存值优先，否则占位），三个单元测试。
- `Page.icon: String`，`Change::PageIconSet`（可撤销），`workspace.set_icon` / `icon_of`。
- UI：`ui/components/IconPicker.slint`（新）、侧栏 `SidebarItem` 画 `node.icon`、编辑器在标题上方画 `UIState.page-icon`（`PageType.size-page-icon`）、顶栏 ⋯ 菜单入口、`AppWindow` 的 `icon-picker-open` 显隐。
- 场景：`page-icon` / `dark-page-icon` / `icon-picker` 三条已加进 `apply_scene` / `apply_scene_overlay`。
- `tests/integration/storage_test.rs` 有新增断言。
- `cargo check --all-targets` 本次实测**通过**（缓存命中，2.52s）。

**缺口**：SPEC §三十八 原文是「emoji 选择器 **+ 本地图片**」，现在只有 emoji。

**T1.1 交付物**
- 先复核这份在途改动（读 diff，不是信它）：迁移的 v10→v11 升级路径有测试、`CURRENT_VERSION` 与 `MIGRATIONS` 末尾一致、`Page` 的每个构造点都填了 `icon`（`icon: String::new()` / `"".into()` 那些散点要确认没有漏，尤其 `duplicate_page` 与持久化回放）。
- 把 SPEC 要的**本地图片图标**补上，或**出一条 ADR 明确不做并说明代价**（两条路都算完成，但必须选一条并写下来）。若做：字节走既有 `attachments` 通道（ADR-0029/0030 的落盘与降采样缓存），不是新一套存储；渲染尺寸小，需要一张足够小的降采样，别让 16 px 的槽位揣着 4000×3000 的 raster。
- 这一刀补齐 `docs/SPEC.md §三十八「图标与封面」` 的 icon 条目，跑门槛，提交。

---

## 4 · 任务清单

每个 slice 都要**过 §5 的全部门槛**才算完。顺序即依赖顺序（`cover` 与 `lock` 各自加列，谁先都行；`templates` 依赖 `version history` 的同一套「块序列副本」想法——见 T1.4 说明）。

### T1.1 · icon 收口（见上）

### T1.2 · cover（封面）
**需求**：本地图片，可换图 / 移除；**封面之上的标题对比度必须过 §二十一 的可读性要求，不得用最弱配色**。

- 加列（`pages.cover`，指向 `attachments` 的行 id 或空）→ 你自己的迁移号。
- 画在标题上方的一条带（高度是 token，不是魔法数），空态不占位。
- 「对比度必须过」这一条要**量**，不能眼看：`ui/Colors.slint` 里为封面之上的标题加一层保证（scrim 或固定的高对比前景），并把实测比值写进报告（参考 `PLAN.md` 里 ADR-0023 修订那一节的做法：WCAG relative luminance，浅色四面对深色四面）。照抄它的方法是本项目的自觉。
- 换图 / 移除走既有文件选择器与附件落盘；覆盖旧图时注意别把 `attachments` 行泄漏成孤儿（`Change::AttachmentDeleted` 的既有路径）。

### T1.3 · lock（只读开关）
**需求原文**：TextInput、slash 菜单、拖拽、⋮⋮ 的编辑项全部关闭，**并且给出可见的锁定状态，不能静默吞输入**。

- 加列（`pages.locked`）→ 迁移号。**锁是页属性还是可撤销编辑？** 参照 ADR-0044 把 Favorite / Style 放在「不进 undo」一侧的理由，自己判断并出 ADR（两种答案都能自洽，但必须写下来并给出理由）。
- 关闭面要列全并逐个验证：块级 `TextInput` 的进入、标题编辑、slash 菜单、`+`/⋮⋮ 的插入与 Turn into / Duplicate / Move to / 颜色 / 删除、拖拽落点、Ctrl+V 粘贴、Ctrl+D 复制、表格与分栏的增删行列、Markdown 导入写入。
- **不可静默吞输入**：锁定页上每次被拦下的操作都要有可见反馈（锁定徽标 + 提示条），并且**不能**表现为「点了没反应」。这一条要有测试或明确的验证记录。
- 锁定态本身要能一眼看出来，不是只有一个小图标。

### T1.4 · templates（模板）
**需求原文**：页面内模板按钮 + 新建页面时选模板；workspace 模板库：预置若干本地模板，导入导出走 §二十六 的 Markdown 通道；**模板的表示必须是「块序列的副本」，不得引入第二套内容格式**。

- 存储形状是你的第一个决定（出 ADR）：`templates` 表 / `pages.is_template` 标记 / 其它。判据是「模板必须是块序列的副本」这一句——最省的形状是复用 `pages` + `blocks`，只是不入侧栏。
- 两个入口：页面内（把当前页存为模板 / 从模板新建块序列贴进当前页）与新建页面时（选择模板）。
- 预置若干本地模板（比如「会议记录」「周计划」这种），**只走既有块类型**，不许为模板发明新块。
- 导入导出走 §二十六 的 Markdown 通道，不加第二条通道。

### T1.5 · version history（版本历史）
**需求原文**：复用 §二十五 的 snapshot 机制，**不另造一套存储**；用户可见：命名版本、与当前版本对比、恢复；**保留策略必须给出磁盘与 RAM 数字，不接受无限增长**。

- 先读 ADR-0015（rotating snapshots，5 代，开机轮转，损坏时开时恢复）与 `src/storage/` 里的快照代码，再决定「命名版本」挂在哪里（ADR）。
- 三个动作都必须可用：命名 / 对比 / 恢复。**「对比」要给出可读的差异**（哪些块改了、增删了什么），不是两个 blob。
- 恢复必须进 undo 还是必须不可撤销？自己判断并写进 ADR（恢复一个旧版本再 Ctrl+Z 回去，这个语义要明确）。
- **保留策略必须落到数字**：命名版本存多少份、每份多大、磁盘与 RAM 各多少，进 `docs/PERFORMANCE.md`。这一刀最容易破 §二十二，必须量。

---

## 5 · 每个 slice 都要过的门槛

1. `cargo check --all-targets` 干净、`cargo test --all-targets` 全绿（报数：passed / failed / ignored，并按 target 分开报）、`cargo build --release` 零警告。
2. **视觉**：新场景加进 `apply_scene`（含 `dark-*` 臂），然后
   `benchmarks/scripts/sweep.ps1 -OutDir .scratch/sweep<N> -Baseline .scratch/sweep<N-1>`，
   逐场景说明「谁动了、动在哪、为什么是它的改动解释的」；用 `benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` 出 bbox 表，**用数字下结论**，不要靠看 42 张图。当前基线是 `.scratch/sweep32`（64 场景），但你落地时的最新基线以你自己那一轮为准。
3. **性能**：这一刀只要给每行/每次投影加了成本（新字段、新绑定、新回调），就必须走 `benchmarks/scripts/bench.ps1` 的两臂对照并把数字写进 `docs/PERFORMANCE.md`；**必须有对照构建**（前一刀的 commit 在干净 worktree 里编出来，两臂交替跑，各自先自我标识 md5/size），因为历史上「同向上涨」被记过四次成「会话漂移」。如果确实没有 per-row 成本（照 ADR-0044 那种「一行读一个全局属性」的形状），**明确写「不欠 RAM 闸」并给替代证据**（像素读数或形状论证），不要沉默跳过。
4. `docs/SPEC.md` 对应小节要标注已交付 + ADR 号（照 §三十七 各条的写法）。
5. `docs/REPORT_TRACK1.md` 更新（PLAN 段落 / CHANGELOG 条目 / ADR 全文 / 未验证边界）。
6. 未验证项要**诚实列出**：没有真窗口点过的交互、需要真人手感的、headless 场景证明不了的，都写下来。这个项目的规矩是「未完成但诚实上报」胜过「看起来完成」。

---

## 6 · 明确不做 / 不要碰

- **M9 Android 仍然停着**（用户 2026-09-20 明确决定「安卓先不做」）。不要为它改 cfg、加依赖、动 `platform::data_dir`。
- 不引入 WebView、JS/WASM 运行时、云同步、协作、评论、发布站点（SPEC §一/§三十三）。
- 不要动 `[profile.release]`（ADR-0024 钉住的，改它必须重跑 A3 审计）。
- 不要改 Track 2/3/4 的功能代码；发现它们那边的 bug → 报出来，不要顺手修。
- 不要动 Markdown 导出的**内容通道**语义来给页属性加 front-matter，除非你出了 ADR（ADR-0044 的「未验证边界」第三条把这件事挂在明面上）。

---

## 7 · 报告格式

每个 slice 完成后（以及最后）报告一次：

- **改了哪些文件**：路径 + 为什么，关键行号；
- **跑了哪些命令**，输出是什么（粘关键行，不要复述）；
- **门槛结果**：测试数、sweep 对比表、性能数字（或「不欠」的理由）；
- **偏离本 brief 的地方和为什么**（偏离本身不是错，隐瞒才是）；
- **给整合者的注意事项**：要折进 PLAN / CHANGELOG / DECISIONS / PERFORMANCE 的内容；
- **你没碰但注意到的别人 territory 的问题**。

做不完就诚实报状态。**一个未完成但诚实上报的 slice，胜过一个悄悄改坏了范围的 slice。**
