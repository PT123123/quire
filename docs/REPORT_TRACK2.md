# Track 2 · 引用、提及与反向链接（REPORT）

分支：`track/2-references`　·　SPEC §四十　·　ADR 号段 0050–0059
承接说明：本 track 的原 agent 预算耗尽，剩余工作由另一个 agent 接手完成（并行纪律见
`docs/WORKTREE_TRACKS.md`：一条 track 一个分支、一个 slice 一个提交、只改自己区域）。

---

## 0 · 一句话状态

| slice | 内容 | 状态 |
|---|---|---|
| T2.1 | `@page mention`：chip、选择器、跳转、悬空退化 | ✅ 已交付 |
| T2.2 | `@date`：同一套 chip 的第二种载荷 | ✅ 已交付 |
| T2.3 | 反向链接区：增量索引 + 页面底部派生面板 + 折叠 | ✅ 已交付 |
| T2.4 | 页面别名与悬空引用：证明，不是新功能 | ✅ 已交付（三种丢失方式都钉住） |
| T2.5 | `synced block` | ❌ **未做**（见 §7，需要用户拍板语义） |

T2.1–T2.4 的门槛已过（测试 / sweep / 性能），ADR-0050 与 ADR-0051 已落
`docs/DECISIONS.md`，§四十 已在 `docs/SPEC.md` 标注交付，性能数字已进
`docs/PERFORMANCE.md`。**T2.5 没有动过一行代码**——它不是"顺手能补"的那一类，见 §7。

---

## 1 · 缘起：这一刀到底在做什么

§四十 只有六句话，但它们合起来是一个架构要求：**"引用存的是 ID，不是标题"**。

这一句话决定了其余的一切：

- 引用存 id → 改名必须**自然**跟着变（不是"加一个同步逻辑"，而是"根本就没有第二份可同步"）；
- 引用存 id → 反向链接**不需要第二份数据**（查谁指向我就是一次 seek）；
- 引用存 id → 目标页消失时，chip 必须**自己**说"它没了"（因为库里那行还在，只是名字没了）。

所以本 track 真正的工作不是"做三个功能"，而是**把 id 这条线从头穿到尾**：从 `marks` 那一列
载荷，到 chip 的形状，到面板的查询，到 Markdown 导出的那一行字。

---

## 2 · 存成什么形状

### 2.1 两种原子，一处载荷（ADR-0050）

`marks` 表自 migration v3 起只有**一列**载荷 `url`，主键 `(block, start, kind)`。
两种新原子都塞进这一列，**不加列、不改主键、不迁移**：

| kind | `url` 列 | `date` 字段 | 渲染 |
|---|---|---|---|
| `mention` | `quire://page/<id>` | — | chip，图标 `page`，文字 = 目标页**当前**标题 |
| `date` | `""` | `"2026-09-22"` | chip，图标 `clock`，文字 = ISO 串本身 |

映射的唯一知情处是 `Mark::stored_payload()` / `Mark::from_stored()`
（`src/core/types.rs`）。**这里踩过一个坑并修掉**：`repository.rs` 的写路径原本写
`m.url`，读路径却走 `stored_payload()`，于是 date 的载荷**存不进去**
（测试 `a_mention_and_a_date_survive_a_reopen_with_their_payloads` 抓到）。

### 2.2 `InsertReference`：一条命令，不是一个命令对

`exec_all` 对批内每条命令都按**同一个 pre-state** 做计划，所以"替换文本 + 打 mark"
不能拆成 `ReplaceText` + `ToggleMark`（后者会按**旧**文本长度 clamp）。
`src/core/command.rs` 加了 `InsertReference { id, at, label, kind, url, date }`：

- 截断文本到 `at`，push `label`；
- 过滤掉与它**同 kind 且相交**的旧 mark（同一处第二次引用是**替换**，不是叠加）；
- emit `BlockTextSet` + `BlockMarksSet`，revert 一次恢复原 text 与原 marks。

### 2.3 反向链接：**两条索引，不是一张表**（ADR-0051）

migration 16（`src/storage/migrations.rs`）：

```sql
CREATE INDEX IF NOT EXISTS idx_marks_reference ON marks(kind, url);
CREATE INDEX IF NOT EXISTS idx_blocks_page_ref ON blocks(page_ref);
```

查询在 `src/storage/backlinks.rs`，一次 UNION 同时接住两种引用形态：

```sql
SELECT b.id, b.page, b.text, 0 AS block_level
  FROM marks m JOIN blocks b ON b.id = m.block
 WHERE m.kind = 'mention' AND m.url = ?1
UNION
SELECT b.id, b.page, b.text, 1
  FROM blocks b
 WHERE b.page_ref = ?2
 ORDER BY 2, 1
```

**这是本 track 唯一一处"本来可以这样、但我们没有"需要向上解释的地方**：brief 的早期
草案（ADR-0051 原文，见 §8）是往 FTS5 的 `search_blocks.content` 里塞 `__backlink:<id>`
token。那条路的形状错了——它要一份**派生数据**，于是删一条 mention 就得把旧 token 从
content 里摘出来（草案自己记成了 Known Gap）；而索引由 SQLite 从**已经在磁盘上的行**
建出来，没有写入路径、没有 rebuild、没有"忘了同步"。

### 2.4 面板是**投影**，不是数据

`refresh_backlinks()`（`src/app/state.rs`）：每次投影做两次读——先 `count(*)`，
**只在计数非零时**再取一个窗口（折叠 5 行 / 展开 50 行，`BACKLINK_WINDOW`）。
和目录（ADR-0039）同一条规则：现算，不入库，不双写。

被引用 200 次的页面画 5 行，不画 200 行；"and N more" 是 Rust 算的字串
（`backlink_fold_label`），因为 Slint 没有格式化。

---

## 3 · 接缝（改了哪些文件）

| 文件 | 改了什么 | 为什么 |
|---|---|---|
| `src/core/types.rs` | `MarkKind` 末尾加 `Mention, Date`；`Mark` 加 `date`；`stored_payload` / `from_stored` | 只追加，绝不重编号（四 track 约定） |
| `src/core/reference.rs` **(新)** | `PAGE_SCHEME` / `page_uri` / `page_of` / `trigger_at` | 引用地址的**唯一**解析点 |
| `src/core/date.rs` **(新)** | `is_iso_date` / `today_iso` / `civil_from_days` | 日期格式的**唯一**定义（导入与选择器共用） |
| `src/core/command.rs` | `InsertReference` | 见 §2.2 |
| `src/storage/migrations.rs` | `CURRENT_VERSION` → 16，追加 step 16 | 串行接缝：不跳号，体走 "缺哪列补哪列" 范式 |
| `src/storage/backlinks.rs` **(新)** | `Reference` / `references_of` / `count_of` | 反向链接的唯一查询点 |
| `src/storage/repository.rs` | 写路径改 `stored_payload()`；加 `references()` / `reference_count()` | 修 date 丢载荷的 bug + 面板入口 |
| `src/app/state.rs` | `MentionTitle` / `MentionTitles`；`build_runs` 加 titles 参；`backlinks` 模型；`refresh_backlinks` / `toggle_backlinks`；`open_slash_mention` / `slash_selected_mention` / `apply_mention`；`delete_page` 里重投影 | 投影层：chip 的活标题、面板、选择器、删页后的可见退化 |
| `src/app/controller.rs` | `@` 触发 + slash 第四模式；`open_slash_at` 抽出公共锚定；`backlink-jump` 走 `quire://block`；`apply_scene` 加 10 个场景 | 交互接线 |
| `src/services/export_service.rs` | `export_page_with(blocks, title_of)`（原 `export_page` 退化为 `|_| None`） | 导出的名字必须是**现在**的名字 |
| `src/services/import_service.rs` | 删掉本地 `PAGE_REF_PREFIX` / `is_iso_date`，改用 `core::reference` / `core::date` | 消除第二份格式定义 |
| `ui/Types.slint` | `TextRun` 加 `mention` / `mention-deleted` / `date`；新 `BacklinkRow`；`backlinks` / `backlink-total` / `backlinks-expanded` / 两个 callback | 数据契约 |
| `ui/components/BacklinkPanel.slint` **(新)** | 面板本体：分组头、行、折叠控件 | 无引用时高度 0，不画一条线 |
| `ui/components/Editor.slint` | 挂面板 | 页面尾部 |
| `ui/components/EditorBlock.slint` / `ColumnItemRow.slint` / `TableBlock.slint` | chip 是 **particle**：图标 + padding + min-width 修正 + `TouchArea` | 一个 mark = 一个不可断行的 cell |
| `ui/AppWindow.slint` | `slash-open` 关闭漏斗里清 `slash-pick-mention` | 四个模式互斥 |
| `src/bin/quire_shot.rs` | shot 走真实 repository | 面板是**读**，读必须看见写下去的东西 |
| `benchmarks/scripts/sweep.ps1` | 加 10 个场景 | 视觉门槛 |
| `tests/integration/{storage,markdown,workspace}_test.rs` | 迁移 16、往返、导出、投影 | 见 §5 |

> 上表的 diff 行数**包含其他 track 在同一文件里的改动**（共享工作树：`state.rs` /
> `controller.rs` / `repository.rs` / `migrations.rs` 都是接缝文件）。本 track 的净改动
> 约 +2 660 / −139，其中 4 个文件是新建。

---

## 4 · 像素读数（视觉门槛）

```powershell
powershell -ExecutionPolicy Bypass -File benchmarks\scripts\sweep.ps1 `
    -OutDir .scratch\sweep40 -Baseline .scratch\sweep34
```

| | 场景数 |
|---|---|
| 基线 `.scratch/sweep34` | 67 |
| `.scratch/sweep40` | 77 |
| **逐字节相同** | **67 / 67** |
| **变化** | **0**（没有任何既有场景动过一个像素） |
| 新增 | `mention`, `date`, `dangling`, `backlinks`, `backlinks-open`, `backlinks-small` + 4 个 `dark-` 臂 |

"0 个既有场景变化"是本 track 最重要的一条像素证据：chip 是 particle、面板挂在页面尾部、
`build_runs` 多了一个参数——这些全都在既有渲染路径上，而既有 67 个场景一个字节都没动。

`diffbbox.ps1` 因此**没有输出**（没有变化的场景可比较）；新增场景的目检结论：

- `mention` / `date`：chip 有底色、有图标、与正文基线对齐；
- `dangling`：chip 变灰，文字 `(deleted page)`，**没有**继续叫旧名字；
- `backlinks`：正文下方一条分隔线 + 组头（来源页名）+ 行（来源块自己的文字）；
- `backlinks-open`：展开到 50 行，折叠控件变成 "Show less"；
- `backlinks-small`（3 条）：**没有折叠控件**——折叠会藏掉 0 条，一个死控件比没有更糟。

---

## 5 · 验证（测试）

`cargo test --all-targets`（2026-09-22，共享工作树，HEAD `d65ad3e` @ `track/3-database`）：

| target | passed | failed | ignored | 备注 |
|---|---:|---:|---:|---|
| **lib** | 306 | **4** | 12 | 另 2 个用例**挂住**（>60 s）被 `--skip` 过滤掉了 |
| workspace | 14 | 0 | 0 | |
| storage | 37 | 0 | 2 | 含新增的 `the_v16_step_adds_reference_indexes_to_a_v15_database` |
| persistence | 5 | 0 | 0 | |
| markdown | 69 | 0 | 0 | 含新增的 4 条引用/日期往返 |
| search | 17 | 0 | 0 | |
| find | 9 | 0 | 0 | |
| backup | 13 | 0 | 1 | |

**4 个失败 + 2 个挂住，全部在 `storage::database_store::tests`（Track 3 的在途文件）**，
失败点例如 `src/storage/database_store.rs:2351` 的 `Option::unwrap()` on `None`——
与本 track 的改动无交集（本 track 没碰 `database_store.rs`）。本 track 自己的
**全部新增与既有测试通过**。
（`--all-targets` 因此会中途 abort；上面的数字是按 target 单独跑出来的。）

新增的本 track 测试（名字即断言）：

| 位置 | 测试 |
|---|---|
| `src/core/reference.rs` | `an_address_round_trips_and_only_this_kind_of_address_parses`、`the_trigger_is_the_last_at_that_starts_a_word`（覆盖 `@Pro` / `see @Pro` / `@one @two` / `@Pro Atlas` / `me@example.com` / `\@escaped`） |
| `src/core/date.rs` | `the_payload_format_is_exact`、`epoch_conversion_survives_the_usual_traps`（闰日 / 世纪 / 负天数）、`today_is_what_the_format_says_it_is` |
| `src/core/command.rs` | `an_inserted_reference_replaces_the_filter_and_undoes_as_one_step`、`a_second_reference_over_the_same_bytes_replaces_the_first` |
| `tests/integration/storage_test.rs` | `the_v16_step_adds_reference_indexes_to_a_v15_database`、`a_mention_and_a_date_survive_a_reopen_with_their_payloads` |
| `tests/integration/markdown_test.rs` | `a_mention_exports_as_a_link_to_its_page_and_reads_back_as_a_mention`、`a_date_exports_as_its_own_iso_text_and_reads_back_as_a_date`、`an_export_that_knows_the_workspace_names_each_page_as_it_is_called_now`、`a_mention_is_an_atom_so_a_bold_run_over_it_gives_way` |
| `src/app/state.rs` | `a_mention_run_reads_the_live_title_and_degrades_when_the_page_is_gone`、`a_date_run_carries_its_own_text_and_no_address`、`the_panel_says_how_many_and_how_many_it_is_not_showing`、`deleting_a_page_degrades_the_chip_that_named_it`、`renaming_moving_and_losing_the_page_a_reference_points_at`、`the_backlink_panel_groups_by_page_and_names_each_page_as_it_is_called_now` |
| `src/app/state.rs`（`#[ignore]`） | `cost_of_the_backlink_panel_on_a_page_open` |

### T2.4 的三种"丢失"，一条测试钉完

`renaming_moving_and_losing_the_page_a_reference_points_at` 一次跑过三个阶段：

1. **改名**——chip、侧栏行、Markdown 导出**同时**变成新名字，而
   `stored()` 断言那个块的 `text / marks / url` **逐字节没变**
   （"重命名一个被 3 000 处引用的页面只写一个页面"）。
2. **移到别的父页**——`children_of(archive)` 含它，chip 照旧解析，块照旧没动。
   （按 "标题 + 父路径" 存的方案会在这里断掉。）
3. **id 指向本库不存在的页**——chip 变 `(deleted page)`，`mention` 仍保留那个 id
   （一次修复想要的就是它）；再走一遍真删页，落到**同一个**状态——两种丢失是
   一条规则，不是两个 case。

---

## 6 · 性能（写进 `docs/PERFORMANCE.md`）

一句摘要：面板给一次开页带来 **+0.06 ms（折叠）/ +0.07 ms（展开）**；
参考读 **折叠 78 µs**，而**同一进程里把 `idx_marks_reference` drop 掉再跑同一条查询
是 6 068 µs（78×）** —— 这个反事实臂就是"不许全库扫"这句话的量化。
库：1 200 页 / 1 200 块 / 100 200 条 mark，其中 200 条是引用。
（对照构建要求：本臂与对照臂在同一进程、同一 sitting 交替跑，各自先自我标识。）

详细表见 `docs/PERFORMANCE.md` 的 "T2 · the backlink panel is one index seek"。

---

## 7 · 未完成 / 未验证边界（诚实清单）

### 7.1 T2.5 `synced block` —— **已做**（2026-09-22，ADR-0052）

用户「你可以开，后续碰到问题了以后我再改就行了」，四条语义由我按 ADR 定下，全文在
`docs/DECISIONS.md` 的 ADR-0052，**任何一条被推翻都是改一行的事**：

1. **删除语义**：删镜像只删这一行（**没有外键、没有级联**）；删源 → 镜像保留、可见退化成
   `(deleted source)`、并变只读（`content-of` 返回 -1，编辑绑定无处可落）。
2. **undo 语义**：**不需要合并** —— 只有源块持有内容，一次编辑真的只写一处，第二个位置的更新
   发生在下一次投影。这是"没有第二份东西"买到的最实在的一件东西。
3. **环检测**：照 §三十九，**在写入那一刻做，不在渲染时做**（`sync_would_cycle`，上界
   `SYNC_CHAIN_MAX`）；渲染侧的 `sync_target` 也带 `SYNC_RESOLVE_MAX`，所以旧备份里的环是
   "画错一句话"而不是"挂住"。
4. **Markdown 导出摊平**（ADR-0032 columns 的先例）；**导入有意不认新语法**，理由写在 ADR 里
   而不是藏在实现里。

**只同步一个块，不同步整棵子树。** 这一点值得单独说清楚：子树会让行数变成动态的，于是
§三十七 那两处附加改动（真删子树、row→model 换算）全都要跟上 —— 那是另一个量级的一刀。
本刀把这两处**判定为用不到**（镜像没有子节点、占一行、行数恒定），并把这个判定写进了 ADR，
免得后来者以为它是被漏掉的。

**未验证（T2.5 自己的）**：真键盘输入；源与镜像同时在屏时两个输入框的焦点争用；真点一次跳转。
性能上这一刀给每次投影加了"每个镜像行一次查表"—— O(1) 查表不是 O(n) 扫描，所以**没有欠量 RAM
闸的理由，但也没有数字**。

### 7.2 headless 证明不了的交互（必须真人过一遍）

- `@` 选择器的**键盘**操作（Up / Down / Return / Escape）——场景只画得出弹窗打开的静态样子；
- chip 的**悬停**（有没有 hover 态、指针形状）；
- **真点一次跳转**：代码路径是 `open-link` → `quire://page/<id>`，`quire://block/<id>`
  在 controller 里已接线，但"点下去视口有没有动"没被人眼看过；
- date 选择器的**日历/相对日期**（当前只写 `today_iso()`，没有日历面板）。

### 7.3 已知限制（与 TOC 共享）

`quire://block/<id>` 锚点**移动光标但不滚动视口**——Slint 1.18 的普通 `ListView` 没有
`bring-into-view`。TOC 与所有锚点共用这条限制，brief 允许"如实写下来"，本 track
**没有**做那条可选的"变高行 reveal"（它会动到 TOC 的行，属于另一个 track 的 territory）。

### 7.4 没量 / 没覆盖

- 面板的 `count(*)` 是**不分过滤条件**的单页计数，"带过滤的引用列表"没做也没量；
- 展开态上限是 50 行（`BACKLINK_EXPANDED`），超过 50 条仍然只显示一个数字 —— 这是
  **故意**的（一个被引用 200 次的页脚放不下 200 行），不是遗漏；
- date 是 ISO 纯展示，**没有** locale 化。

---

## 8 · 与 brief 的偏离（偏离本身不是错，隐瞒才是）

1. **ADR-0051 整篇重写**：brief 附的草案是 FTS5 `__backlink:` token 方案，已废弃，
   换成"两条索引 + 派生投影"。理由见 §2.3。草案原文保留在本报告的历史版本里。
2. **`Cargo.lock` / `Cargo.toml`**：本 track **没有**加任何依赖（chip 与面板都是自绘）。
3. **面板分组头**：brief 只说"列出所有引用本页的块"，我加了**按来源页分组**
   （第一行的组头写来源页名）。理由：不分组的话同一页的 12 条引用读起来是 12 条匿名行。
4. **shot 走真实 repository**：`src/bin/quire_shot.rs` 原来不带库。面板是读，
   读必须看见写下去的东西，所以 shot 现在建一个临时库。

---

## 9 · 别人 territory 里注意到的问题（只报不改）

- `src/storage/database_store.rs` 当前有 **4 条 release 警告**（unused imports、dead code
  `NOW` / `row_query` / `row_binds`）与 1 条 test 警告（unused `db`）。它们属于 Track 3
  的在途文件，共享工作树里 `cargo build --release` **因此不是零警告**。等 Track 3 收口后
  再过一次门槛才算干净——本 track 自己的文件是零警告。
- 共享 `target/` 目录在四 track 并发时会撞链接：`link.exe` 抢同一个
  `target/debug/deps/quire-<hash>.exe` 会报 **LNK1104**，且有 hung 住的测试二进制长期占着它。
  （绕不开：磁盘只剩 22 GB，一个 target 就 48 GB，开不起第二份。）**给整合者的建议**：
  四 track 合并前，先确认没有残留的 `quire-*.exe` 进程。

---

## 10 · 给整合者：要折进 PLAN / CHANGELOG / DECISIONS / PERFORMANCE 的内容

### CHANGELOG 条目（草稿）

```markdown
### 引用、提及与反向链接（SPEC §四十）

- **@page mention**：正文里 `@` 弹出页面选择器，插入一枚引用目标页的 chip。
  存的是目标页 **id**，不是标题——改名后所有引用自然显示新名字。
- **@date**：同一套 chip 的第二种载荷，ISO `YYYY-MM-DD`。
- **反向链接区**：页面底部列出所有引用本页的块，按来源页分组；折叠 5 行、展开 50 行，
  超出部分用 "and N more" 表示。派生数据，**不入库、不双写**（migration 16 加两条索引）。
- **悬空引用可见退化**：目标页被删 / 引用一个本库不存在的 id 时，chip 显示
  `(deleted page)` 并灰化，不崩、不静默显示旧名字。
- 点 chip 跳目标页，点反向链接行跳**来源块**。
```

### DECISIONS

- ADR-0050（两种原子共用 `marks.url`，Markdown 语法 `@[Title](quire://page/<id>)` /
  `@[YYYY-MM-DD]`）✅ 已落；
- ADR-0051（反向链接：两条索引 + 派生投影，**取代** FTS5 token 草案）✅ 已落。
- 号段 0052–0059 **空着未用**——T2.5 的 synced block ADR 建议从 0052 起。

### PLAN

- `PLAN.md` 允许各 track 末尾追加小节；本 track 的逐刀报告就是本文件。

### 提交形状（已做）

- 分支 **`track/2-references`**，提交 **`19201c3`**，父提交 `d65ad3e`。
- **共享 HEAD 没有动**：仍然在 `track/3-database` @ `d65ad3e`；工作树仍然脏，
  四个 track 的未提交内容原样保留。整个提交用 plumbing 建成
  （`read-tree` → `update-index --cacheinfo` → `write-tree` → `commit-tree` → `update-ref`），
  没有 `checkout` / `reset` / `stash`，也没有 `git add -A`。脚本留在
  `.scratch/t2_build_commit.py`（`.scratch` 被 gitignore，需要时可重建）。
- **四个接缝文件按 hunk 拆过**，别人未提交的行被摘掉了：
  - `src/core/mod.rs` —— 只留 `pub mod date/reference` 与三行 `pub use`
    （`pub mod database_property;` 是 Track 3 的，留在工作树里）；
  - `src/storage/repository.rs` —— 留 `references` / `reference_count` 与两处
    `stored_payload()`，**摘掉** `database_store::touch_edited_by_page(...)`；
  - `src/storage/migrations.rs` —— 只留 **v16 两条索引**，`CURRENT_VERSION`
    在这次提交里是 **16**；v17（record timestamps，ADR-0068）与
    `add_record_timestamp_columns` 是 Track 3 的，摘掉；
  - `tests/integration/storage_test.rs` —— 留 `date: None` 两处与 v16 那条测试，
    两处 `RowRequest::new(...)`（Track 3 未提交的重构）退回原来的结构字面量；
  - `docs/DECISIONS.md` / `docs/SPEC.md` —— 按 hunk 过滤，只进本 track 的
    ADR-0050 / ADR-0051 与 §四十 那一段（ADR-0066/67、ADR-0080/81、PDF /
    bookmark / embed 那些 hunk 全部留在工作树里）。
- **偏离一处**：纪律是"一个 slice 一个提交"，这里合成了一次提交。原因：共享工作树里
  按 hunk 再切 slice 会把 `state.rs` / `controller.rs` 同一个文件的相邻改动拆成三份
  互不完整的树，风险大于收益；每个 slice 的边界在上文 §2 / §5 里写清楚了。

---

## 11 · T2.5 收口（2026-09-22 08:00–09:00，第二次提交）

### 编译与测试（工作树 = T2.5 + Track 3 D3 的集成态）

- **修了三类静默破损**（都是上次会话批量补字段时留下的）：
  1. `src/services/import_service.rs` 与 `tests/integration/markdown_test.rs`
     共 **7 处** `ParsedBlock` 字面量被误补了 `db_ref: None,` —— `ParsedBlock`
     没有这个字段，lib + markdown test 共 7 个 E0560。判据（下次批量补字段用）：
     `db_ref: None,` 的**下一行不是 `sync_ref`** 的就是 `ParsedBlock` 字面量
     （`Block` 字面量的 `sync_ref` 紧跟在 `db_ref` 后面）。
  2. ADR-0052 正文写「UI int 23」，代码是 `Database => 23, Synced => 24`
     —— 已把 ADR 改成 24。
  3. `src/storage/mod.rs` 缺 `pub mod backlinks;`（19201c3 的树里 `backlinks.rs`
     是孤儿文件）—— 本 track 自己的遗漏，这次提交一并带上。
- **验证数字**：`cargo check --all-targets` 干净（3 条警告全部是 Track 3 领土的：
  state.rs 未用导入 TITLE_PROPERTY_NAME/ViewLayout、command.rs 未用 PageFont、
  db_definition/db_columns never used）；按 target 分开跑全绿 ——
  **lib 314 passed / 0 failed / 13 ignored**；backup 13+1ig；find 9；
  markdown 69；persistence 5；search 17；storage 39+2ig；workspace 14
  （日志 `.scratch/t25_tests_0812.log`）。`cargo build --release` 零警告
  （日志 `.scratch/t25_release_0830.log`）。
- **T2.5 的测试清单**（lib，`src/app/state.rs::tests`）：
  `a_mirror_draws_its_source_and_degrades_when_the_source_goes`（镜像画源、
  源删了退化只读）、`a_mirror_refuses_to_point_at_itself_or_close_a_cycle`
  （自指/环/非 Synced 块三个拒绝）；kind 字符串 `"synced"` 的往返由
  `types.rs` 的逐 kind 断言覆盖；存储列 `blocks.sync_ref` 的读写由
  repository 的逐列装配合着上面两条走通。
- **sweep 未跑**（诚实清单不变）：共享工作树没法给 synced 场景出干净数字 ——
  `synced` / `synced-source-gone`（含 dark 臂）的场景代码已进
  `apply_scene` 与 `sweep.ps1`，等整合者在干净的合并树上跑。

### 提交形状（与 19201c3 不同，这次是**合并提交**）

- 分支 **`track/2-references`**，**两个父**：`19201c3`（本 track 前四刀）与
  `93307ca`（Track 3 的 D3）。树 = 共享工作树的集成态（T2 四刀 + T2.5 + D3），
  **减去**不属于本 track 的未提交内容：T4 的 hayro 依赖（Cargo.toml/lock）、
  PDF 缩略图（`pdf_thumb.rs` / `attachment_store.rs` / `services/mod.rs` 一行）、
  verify 脚本改动、`docs/DECISIONS.md` 与 `docs/SPEC.md` 里 T1/T4 的 hunk。
- 为什么是合并提交：T2.5 的代码在共享工作树上**长在 D3 的代码中间**
  （state.rs / controller.rs / Types.slint 同文件交织），单父提交要么把 D3 整体
  算进 T2 的 diff、要么拆出一棵编译不过的树。合并提交如实记录
  「T2.5 是在 D3 之上集成的」，`git show` 的 combined diff 就是集成补丁本身。
- 构建照旧走 plumbing（临时索引 + `write-tree` + `commit-tree` +
  `update-ref`），共享 HEAD 与工作树不动。脚本 `.scratch/t25_build_merge.py`。
