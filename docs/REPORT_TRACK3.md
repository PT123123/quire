# Track 3 — Database（D0：决策与探针）

分支 `track/3-database`（从 `master` 的 `f580939` 起）。本刀（D0）**只做两件事**：证明 SPEC §三十九
性能红线第一条在选定形状下真的成立（给出数字），并把六个先决决策写成 ADR-0060…ADR-0065。
**没有交付任何功能**——没有 `.slint` 视图、没有表、`CURRENT_VERSION` 仍是 **11**。

范围与阶段划分见 `docs/AGENT_BRIEF_T3_DATABASE.md`；本文件的读者是整合者。

---

## 1 · 本刀改了哪些文件

| 文件 | 为什么 | 关键位置 |
|------|--------|----------|
| `src/core/database.rs`（**新**，565 行） | 纯「可见窗口」投影，证明通道存在。没有 SQL / Slint / 时钟 | `window()` L106、`RowWindow::fetch()` L99、`max_scroll_y()` L130、`RealizedRows::scroll_to()` L160、断言测试 L205–336、测量探针 L338–563 |
| `src/core/mod.rs` | **+1 行** `pub mod database;`（L5） | 只加这一行，没重排、没删 |
| `docs/DECISIONS.md` | 追加 ADR-0060…ADR-0065（文件末尾，共 377 行） | 见 §3 |
| `docs/SPEC.md` | §三十九 六个小节标注已交付 + ADR 号（照 §三十七 的写法） | 两处 hunk：L2488 附近与 L2500 附近 |
| `PLAN.md` | **只在末尾追加** `## Track 3 · D0 决策与探针`（57 行） | 新文件末尾小节 |
| `benchmarks/results/2026-09-22-track3-probe.jsonl`（**新**） | 探针的原始 JSON 行（两次运行） | 2 行 |
| `docs/REPORT_TRACK3.md`（**新**，本文件） | D0 的全部草稿与读数 | — |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`、`[profile.release]`、
任何 `ui/*.slint`、任何 `src/storage/**`、任何 Track 1/2/4 的文件。没有新增依赖。

---

## 2 · 探针：SPEC §三十九 性能红线第一条

> 10 000 行的库不得全量 realize；视图先算可见窗口再取行

### 2.1 形状

`src/core/database.rs` 是一个**纯函数 + 一个只能装窗口的容器**：

* `ViewGeometry { row_height, viewport_height, overscan }`（overscan 默认 `DEFAULT_OVERSCAN = 8`，单位是**行**，
  因为窗口的成本是「它持有几个行对象」）。
* `window(total, geometry, scroll_y) -> RowWindow { start, end }`：偏移先按内容夹紧（Slint 就是这么夹的，
  见 L106–127），再算 `first = offset / row_height`，`start = first - overscan`，
  `end = min(total, first + visible + overscan)`。**空表返回 0..0**；行高未量到（Slint 首帧报 0）时按
  1 px 兜底，宁可多 realize 一帧也不除零。
* `RowWindow::fetch() -> (limit, offset)`：**这就是那次查询的 `LIMIT`/`OFFSET`**，「先算窗口再取行」的
  「再取行」是一个数对，不是一句纪律。
* `RealizedRows::scroll_to(total, geometry, scroll_y, fetch)`：唯一构造器——先算窗口，再把窗口交给
  `fetch`；`rows.len() <= window.len()` 由它维持，没有任何路径能持有全表。

### 2.2 数字（10 000 行 / 32 px 行高 / 720 px 视口 / 8 行 overscan，**release**）

| 读数 | 值 |
|------|----|
| realize 行数（表顶） | **31**（窗口 0..31 = 可见 23 + 上 8 下 8） |
| realize 行数（`scroll_y = 4000`） | **39**（窗口 117..156） |
| realize 行数（滚到底 `scroll_y = 319 280`） | **31**（窗口 9 969..10 000） |
| 同样几何下 100 / 1 000 / 1 000 000 行的窗口 | **同一个 0..31**（界住它的是视口不是表） |
| 窗口 31 行的行对象占堆 | **6 806 B** |
| 同一张表 10 000 行全 realize 占堆 | **2 259 800 B**（**332×**） |
| 只要 id 的 `Vec<u64>`（SQL 侧索引的形状） | 80 000 B |
| 构造耗时：全表 / 只要 id / 窗口那次取行 | 6.317 ms / 0.061 ms / **0.0174 ms** |
| 进程读数（把 10 000 行真拿进内存） | working set 11.0 → 14.0 MB，**private 2.0 → 5.1 MB（Δ 3.1 MB）** |

**怎么得到的**：

* 断言：`cargo test --lib database::`（7 条，全绿；10 000 行的窗口与 31 这个数是 `assert_eq!` 钉住的，
  不是打印出来的）。
* 数字：`cargo test --release --lib -- --ignored --nocapture a_window_costs`（`#[ignore]` 的打印型探针，
  与仓库既有 `cost_of_a_...` 打印型测试同一个约定）。
* 堆字节用**只在测量线程计数**的全局分配器量（L338–400）：`System` 原样转发，计数开在
  `thread_local` + `const` 初始化上（分配器里不能再触发一次惰性 TLS 分配），别的测试线程并行跑但没 arm，
  所以互不污染。`realloc` 按 `new_size - old_size` 记账。
* 进程字节用测试内手写的 `K32GetProcessMemoryInfo` 声明（L516–563，与 ADR-0025 一个套路：一个
  `extern` 块换掉一个 crate），报的就是 `bench.ps1` 那两个数（working set / private）。
* 原始行：`benchmarks/results/2026-09-22-track3-probe.jsonl`（两次运行；堆数字**逐字节相同**，
  working set 差 0.1 MB）。

### 2.3 它证明 / 没证明什么

**证明了**：投影是窗口有界的——窗口由视口与行高算出、与总行数无关；取行计划由窗口给出
（`fetch()` 的 `LIMIT`/`OFFSET`）；行对象只存在于窗口里（构造器保证）；空表、短表、陈旧偏移、
未量到行高、NaN 行高都不会 panic 也不会越界。内存上，**10 000 行的行对象（2.16 MiB 堆 / ≈3.1 MB 私有）
在窗口形状下根本不发生**，发生的是 6.8 KB。

**没证明**（诚实清单，也写进了 SPEC §三十九 与 PLAN）：

1. **帧**：D0 没有任何 `.slint` 视图，所以 31 这个数是「投影里的行对象数」，不是「画出来的 delegate 数」。
   `bench.ps1` 一次都跑不了——它要等一个真窗口句柄才采样，而 D0 的代码不画东西。**没有任何 UI 臂**。
2. **SQL 侧**：`fetch()` 的 `LIMIT`/`OFFSET` 只是**计划**，一次都没执行过；`COUNT(*)` 也还没有（要 D1 的表）。
   所以「取回来的行数就是窗口长度」这条今天是算术，不是查询。
3. **行高与 overscan** 是 32 px / 8 行这两个假设；**行 payload 是代表性的**（`RowView { record, title,
   cells: Vec<String> }`，5 个文本 cell），D2 的类型化 cell 会换掉它——换掉之后窗口算术不变。
4. 进程读数来自**测试进程**内部的 FFI，与 `bench.ps1` 的 `ram_private_mb` 同源但不同二进制，
   **不能**与 `benchmarks/results/` 既有的行直接相除（那条方法学教训在 PLAN 的 columns 小节里写过）。

---

## 3 · 六个决策落在哪（brief §3 D0 的六问）

| # | 问题 | ADR | 一句话决策 | 代价（写进 ADR 的） |
|---|------|-----|------------|---------------------|
| 1 | database 是什么实体 | **ADR-0060** | 自己的 `databases` 行 + 一个新的 `Database` 块经 `blocks.db_ref` 指过去（同 ADR-0026 的 `page_ref`）；「整页数据库」就是首块是它的普通页；八个视图 = 八种 `db_views.layout` | 一个新块种类 = §三十七 的六个接点；D5 之前「+」菜单那六行占位仍在承诺不存在的东西 |
| 2 | schema 存哪 | **ADR-0061** | `db_properties` **行表**（`UNIQUE(db,name)` 是行表才有的不变量），只有 select/status 的选项列表是行内 JSON（选项带自己的 id） | `ord` 只能靠 app 保证；「每库恰好一个 title」不是约束 |
| 3 | 值怎么存 | **ADR-0062** | `db_values` 一行一 (record, property)，`text`/`num`/`flag` 三列 + `db_value_items` 给 multi-select / files；判据是「比较必须发生在 SQLite 自己的类型系统里」 | `created/last edited time` 的来源留 D2；排序空值位置要显式写 `IS NULL` |
| 4 | record ↔ page | **ADR-0063** | record **拥有**它的 page（`UNIQUE(page)` + `ON DELETE CASCADE`）；标题只有一个家（有页在 `pages.title`，无页在 `db_values`，读时 `COALESCE`）；**懒建页**；两个删除方向各一批 change、一次 Ctrl+Z | **侧边栏删页仍不进 undo**（既有缺口，明写不假装闭环）；title 列的过滤/排序编译特殊 |
| 5 | 视图定义怎么持久化 | **ADR-0064** | `db_views` 行（名字 / layout / 顺序是列）+ 一份 JSON 规则文档（过滤是递归树） | 删属性不能清理 JSON 里的引用（编译期忽略）；group 头那一行在窗口里的位置留 D4 |
| 6 | Markdown 通道 | **ADR-0065** | 导出**当前视图**的 GFM 表格；页-backed 标题写成 `[title](quire://page/<id>)`；**不写标记行**；导入侧不改（管道行回来仍是段落） | schema / 类型 / record 身份不过通道；`export_page` 要加一个「预渲染行」参数 |

三个存储判据是同一条：**SQL 有东西要在它上面过滤吗？** 列和值要在（→ 行 + 类型化列），视图规则不要
（→ 一份 JSON），选项列表也不要（→ 行内 JSON）。ADR-0065 是另一个通道的问题，答案必须明确，所以给了
一个明确的「表格、不写标记、导入不回」。

---

## 4 · 门槛结果

**1. 编译与测试**

```
cargo check --all-targets     → Finished，0 warning（touch src/lib.rs src/core/mod.rs src/core/database.rs 后复测，
                                输出里 warning 计数 = 0）
cargo test --all-targets      → 全绿，按 target 分开报：
  unittests src/lib.rs            247 passed / 0 failed / 10 ignored
  unittests src/main.rs             0 / 0 / 0
  unittests src/bin/quire_typing     0 / 0 / 0
  tests/integration/backup_test     13 / 0 / 1
  tests/integration/find_test        9 / 0 / 0
  tests/integration/markdown_test   62 / 0 / 0
  tests/integration/persistence_test 5 / 0 / 0
  tests/integration/search_test     17 / 0 / 0
  tests/integration/storage_test    27 / 0 / 2
  tests/integration/workspace_test  14 / 0 / 0
  ── 10 个 target 合计 394 passed / 0 failed / 13 ignored
cargo build --release         → Finished，**0 warning**（5m08s 的整库重编，输出里没有任何 warning 行）
                                过程说明：第一次尝试（01:2x）撞上 Track 4 正在写的 `src/services/pdf_thumb.rs`
                                （E0599），与 `src/core/database.rs` 无关；对方修好之后复跑即绿。见 §7 第 3 条。
```

**2. 视觉**：本刀不碰 UI（新模块没有被任何 UI 引用），预期 **changed 0**：

```
pwsh benchmarks/scripts/sweep.ps1 -OutDir .scratch/track3-sweep -Baseline .scratch/sweep33
  sweep at commit: 78ddf35
  vs baseline .scratch/sweep33 (67 scenes):
    changed    0:
    identical  67: default.png … dark-page-icon.png（全部 67 张逐字节相同）
```

**没有**用任务书里写的 `-OutDir .scratch/sweep34`：开工时那个目录正被另一条 track 的 sweep 占着
（01:01 刚写完、manifest 与 sweep33 逐字节相同），往别人正在写的目录里写会毁掉证据。基线仍是
`.scratch/sweep33`（`f580939`，67 scene），比值成立。注意本次 sweep 发生在 **78ddf35**（Track 1 已把
icon 那两刀提交），所以这一行同时说明：Track 1 的提交与 Track 3 的改动都没有移动任何像素。

**3. 性能**：见 §2.2 的表格与原始行。

---

## 5 · PLAN 段落草稿（已按 brief 追加到 `PLAN.md` 末尾，全文照录）

见 `PLAN.md` 的 `## Track 3 · D0 决策与探针（2026-09-22，on track/3-database，ADR-0060…ADR-0065）`
小节（57 行）。要点：缘起（先证明通道） → 通道形状 → 数字 → 证明/没证明 → 六个 ADR 一句话 →
验证（check/test 分 target/visual changed 0） → 未验证五条 → 下一步 D1。
整合者若要重排（例如并进 M14 小节），内容可直接搬运，标题那一行的日期/分支/ADR 号是照既有小节写的。

## 6 · CHANGELOG 条目草稿

**本轮不欠 CHANGELOG 行**：D0 没有任何用户可见变化。既有那句仍然为真、且**不要**改：
「The database views (Table view, Board, Gallery, List, Calendar, Timeline) appear as muted "later"
placeholders and cannot be picked yet」——它们今天仍然不可选。

如果整合者要为此刀的**状态**留一笔（可选，且属于 `Known limitations` 而不属于新功能）：

> - The database layer is decided but not built: §三十九's object model, property storage, views and
>   Markdown answer are written down (ADR-0060…ADR-0065), and the window projection that keeps a
>   10 000-row view to 31 realized rows is measured — but nothing in the UI opens a database yet, and
>   the six insert-menu placeholders are still muted.

## 7 · 给整合者的注意事项

1. **`docs/DECISIONS.md` 与 `PLAN.md` 里有别人的未提交内容，本刀用「只暂存自己那一段」的方式提交**。
   提交时树上还有 Track 2 未提交的 ADR-0050/0051（DECISIONS.md）与 `## M13` 小节（PLAN.md），
   直接 `git add` 会把它们夹带进本刀的 commit。做法：用 `git show HEAD:<file>` + 本刀追加段生成
   一份「HEAD + 只有本刀」的 blob，`git hash-object -w --path <file>` + `git update-index --cacheinfo`
   写进 index。因此**提交后** `docs/DECISIONS.md` 与 `PLAN.md` 在工作树里仍是 modified（那是 Track 2 的那两段），
   不是脏数据。Track 2 提交自己那段时，两段会自然接在一起。
2. **ADR 号占用**：本刀取 **ADR-0060…ADR-0065**（brief 给 Track 3 的号段是 0060…0079）。Track 4 的
   ADR 草稿里出现了 **ADR-0080**（PDF 缩略图）——它在本刀之后写，号段上不冲突，但如果 Track 4 也要用
   0060 段，请整合者仲裁。D1 会用 **0066…**。
3. **共享工作树会在中途变红：`cargo build --release` 第一次撞上 Track 4 的半成品**（01:21 起
   `src/services/pdf_thumb.rs`（新建，配合 `Cargo.toml` 新增 `hayro` 依赖）报
   `error[E0599]: ... which is required by 'LoadPdfError: ToString'`）。这不是本刀的文件，本刀的
   `cargo check --all-targets`、`cargo test --all-targets`（394 passed）在它落地之前跑过、是绿的；
   对方修好之后 `cargo build --release` 复跑**绿且零警告**。**给整合者**：本刀提交时的树是绿的，
   但四条 track 共用一棵树，合并前请再跑一次三条门槛——若红，先看红在谁的文件里。
4. **迁移号是串行接缝**：D1 的第一步取**提交那一刻**的 `CURRENT_VERSION + 1`（今天 = 12），并重新读一次
   `src/storage/migrations.rs`——Track 1/2 都可能先占号。本刀**没有**改 `migrations.rs`，
   `CURRENT_VERSION` 仍是 11。
5. **`bench.ps1` 没有数据库臂，这一条是 D3 的欠账**：SPEC §三十九 要的三个数字（10 000 行的 RAM、
   切换视图耗时、打开公式编辑器耗时）都进 `docs/PERFORMANCE.md`，本刀只给了探针的堆/私有字节
   （`benchmarks/results/2026-09-22-track3-probe.jsonl`）。整合者若要收口这一条，请在 D3 之后再收，
   现在的数字还不能与 bench.ps1 的既有行相除（不同二进制、不同采样点）。
6. **`docs/PERFORMANCE.md` 本刀没动**：探针数字先落在 `benchmarks/results/`，因为 PERFORMANCE.md 的
   §Method 要求的是「release 二进制 + 对照构建」，本刀两者都不满足（没有 UI 臂、没有前一刀的对照构建）。
7. **未提交的 `.scratch/`**：本刀的证据在 `.scratch/track3/`（探针日志、暂存用的 blob 源文件、
   `track3-sweep/` 的 67 张 PNG + manifest）与 `.scratch/track3-sweep/`。都不进提交。

## 8 · 我注意到、但没碰的别人 territory 的问题

1. **Track 4 半成品把树弄红了**（上面第 3 条）：`src/services/pdf_thumb.rs` + `Cargo.toml`/`Cargo.lock`
   的 `hayro`。我只读不写。
2. **侧边栏删页不进 undo**：`AppState::delete_page`（`src/app/state.rs:2394`）记录 `Change::PageDeleted`
   就直接删了，历史栈只有编辑器命令。§三十九 要求「删 record 与删页面都进 undo」，ADR-0063 因此把
   缺口写进明面；真正的修法（把那个确认框接到同一批 change 上）在 Track 1 的 territory（页面生命周期），
   本刀没动。
3. **`docs/SPEC.md` 与 `docs/ROADMAP.md` 里的 `\r`**：本机 `Get-Content` 会少算行数（SPEC.md 显示
   1964 / 实际 2553）。读这类文件请用 `[System.IO.File]::ReadAllLines(...)`。已知的坑，不是新发现，
   但这次又踩了一次，值得写进交接。
4. **`.scratch/sweep34` 被另一条 track 占着**：本刀因此换目录（§4 第 2 条）。整合者若要统一
   sweep 目录命名，注意四条 track 会同时写 `.scratch/`。

---

# Track 3 — Database（D1：数据层）

D0 证明了通道（投影先算窗口），D1 让通道**真的从 SQL 里取行**，并把 SPEC §三十九 的对象落成表。
本刀交付：六张表 + 四步迁移（v12–v15）、`core` 对象模型、`storage` 读写与窗口查询、
record↔page 的生命周期契约与测试、重建（关掉再打开逐字段一致），以及本刀欠的性能数字。
**没有 UI**：没有 `.slint`、没有块种类、六行插入菜单占位仍不可选（ADR-0060 的六个接点整批留待 D3）。

## 1 · 本刀改了哪些文件

| 文件 | 为什么 | 关键位置 |
|------|--------|----------|
| `src/storage/migrations.rs` | v12–v15 四步（一步一个语义单位）+ `check_schema` 的表清单 | `CURRENT_VERSION` L10（11 → **15**）、v12 L202、v13 L218、v14 L239、v15 L287、`TABLES` L494 |
| `src/core/database.rs` | D0 投影旁边加对象模型与值形状 | `Database` L268、`PropertyKind` L336、`CellValue` L589、`DatabaseCatalog` L659、`RowRequest` L694、`probe` 改 `pub(crate)` L1022、模型测试在 `mod tests` 末尾 |
| `src/core/persistence.rs` | `Change` 末尾追加 19 个变体（append-only） | L109 起（`DatabaseCreated` … `ViewDeleted`） |
| `src/storage/database_store.rs`（**新**，2188 行） | SQL：六张表的读写、窗口查询、批量路径的快照/恢复、测试、探针 | `load_databases` L62、`cell` L215、`window_rows` L267、`unwindowed_rows` L280、`realized_rows` L289、`row_query` L327、`set_cell` L705、`snapshot_tables` L871、`restore_tables` L957、`mod tests` L1025、`mod probe` L1870 |
| `src/storage/repository.rs` | 接缝：`apply_one` 的 19 个新臂、`replace_all` 的快照/恢复、`require_hit` 改 `pub(crate)` | 快照 L392、恢复 L436、`require_hit` L576、新臂 L936 起 |
| `src/storage/mod.rs` | 声明新模块 | L8 |
| `tests/integration/storage_test.rs` | 7 条集成测试（4 条迁移步 + 重建 + 两条删除路径 + 批量路径） | `mod database_layer` L1449 |
| `docs/DECISIONS.md` | ADR-0066 / ADR-0067 | 文件末尾 |
| `docs/SPEC.md` | §三十九 六处标注已交付 + 红线第一条补上 D1 的读数 | §三十九（对象模型 / record↔page / 属性类型 / 视图 / 操作 / 性能红线） |
| `PLAN.md` | 末尾追加 `## Track 3 · D1 数据层` | 文件末尾 |
| `benchmarks/results/2026-09-22-track3-d1-window.jsonl`（**新**） | 探针的原始 JSON 行（三次运行） | 3 行 |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`、`[profile.release]`、
任何 `ui/*.slint`、任何 Track 1/2/4 的功能文件。**零新依赖**。

## 2 · 迁移号（串行接缝）

* 动手前读 `src/storage/migrations.rs`：`CURRENT_VERSION = 11`（与 D0 报告一致）。
* 本刀采用 **v12 / v13 / v14 / v15**：`databases` → `db_properties` → `db_records` + `db_values` +
  `db_value_items` → `db_views`。**一次迁移 = 一个可独立回滚的语义单位**；v14 把 record 与它的值放在
  一步里，因为「有值没有 record」不是任何一个版本能产生的状态。
* 提交时再读一次：`CURRENT_VERSION` 已是 **16**（Track 2 在我之后加了一步 `reference lookup`，
  两个 `CREATE INDEX IF NOT EXISTS`）。**没有重号**：12–15 是本刀的，16 是他们的。
* 四步都是 `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`，所以「半途的库」收敛而不是报错
  ——这一点是被测试证明的：本刀的迁移测试会把版本降回 11/12/13/14 再升回来。

## 3 · 命令与门槛结果

```
cargo check --all-targets   → Finished，0 warning
cargo test --all-targets    → 全绿，按 target 分开：
  unittests src/lib.rs                277 passed / 0 failed / 11 ignored
  unittests src/main.rs                 0 / 0 / 0
  unittests src/bin/quire_typing        0 / 0 / 0
  tests/integration/backup_test        13 / 0 / 1
  tests/integration/find_test           9 / 0 / 0
  tests/integration/markdown_test      69 / 0 / 0
  tests/integration/persistence_test    5 / 0 / 0
  tests/integration/search_test        17 / 0 / 0
  tests/integration/storage_test       37 / 0 / 2
  tests/integration/workspace_test     14 / 0 / 0
  ── 10 个 target 合计 441 passed / 0 failed / 14 ignored
cargo build --release       → Finished，**0 warning**
```

这张表是**工作树**（HEAD + 四条 track 未提交改动的合集）的读数：本刀的 24 条测试在这 441 条里，
别人的测试也在里面。

**提交的树单独跑过一遍**，因为本刀提交时把别人的 hunk 排除在外，所以「提交树能不能编、测试过不过」
不是推论题。做法是一个独立 worktree（`git worktree add .scratch/track3-d1/verify <sha>`，共用
`target` 目录，不动共享工作树），在**提交树的原文**上跑：

```
cargo check --all-targets → Finished，0 warning
cargo test --all-targets  → 全绿：
  unittests src/lib.rs   264 passed / 0 failed / 11 ignored
  backup 13 / find 9 / markdown 62 / persistence 5 / search 17 / storage 34 / workspace 14
  ── 418 passed / 0 failed / 14 ignored（storage 34 = 原有 27 + 本刀的 7）
```

这一步**抓到过一个真错误，值得记下来**：第一次提交时 `src/storage/mod.rs` 与
`src/storage/repository.rs` 的「本刀 blob」里夹带了 Track 2 的两段（`pub mod backlinks;` 与
`references` / `reference_count`），而 `backlinks.rs` 是他们的**未提交新文件**——提交树因此编不过
（`E0583: file not found for module backlinks`）。同一类问题还有一处：Track 2 把他们的 v16 测试写在
**本刀的 `mod database_layer` 里面**（为了复用 `migrated` / `roll_back`），所以「从本刀的标记取到模块
结尾」会把他们的测试一起带走。修正办法是按名字排除（见 `.scratch/track3-d1/stage.py` 的头部清单），
commit 已 amend（`20b4507`），**上面这张表与 §4 的数字都是修正后重跑的**。
教训：共享文件里的「追加」可以只暂存自己那段，但**逐行排除别人的改动**必须用「提交树单独跑一遍」来证明，
不能靠读 diff。

本刀新增测试 **24 条 + 1 条打印型探针**：

* `core::database::tests` 6 条（kind/layout 的字符串往返与 fold、`Database::title_property` /
  `first_view`、`CellValue` 的位模式等价与显示、catalog 的按库过滤）；
* `storage::database_store::tests` 11 条（schema 往返 + 未知 kind/layout 折回、每种 cell 形状往返 +
  「空 = 没有行」+ 一次写入只留一个形状、窗口真的是窗口、空库、标题的两个家、可见的 title 列、
  删列/删库的级联、`UNIQUE(page)`、改 kind 不迁移值、id 不存在时报错并整批回滚）；
* `database_layer`（`storage_test.rs`）7 条：v12/v13/v14/v15 四条迁移步（fixture 走应用自己的写路径，
  降版本 → 迁移 → 断言旧行逐字段未变 + 新表可用）、`a_database_comes_back_field_for_field_after_a_reopen`、
  `a_row_dies_with_the_page_it_owns_and_both_delete_paths_end_there`、
  `the_bulk_path_keeps_the_database_layer_except_what_hangs_off_a_dropped_page`。

**视觉**：本刀是纯数据层，预期 changed 0，实测 changed 0：

```
pwsh benchmarks/scripts/sweep.ps1 -OutDir .scratch/track3-sweep-d1 -Baseline .scratch/track3-sweep
  sweep at commit: 0786336
  vs baseline .scratch/track3-sweep (67 scenes):
    changed    0:
    identical  67: default.png … dark-page-icon.png（全部逐字节相同）
```

（基线用本 track 自己 D0 的 `.scratch/track3-sweep`，不是任务书里的 `.scratch/sweep33`：两者都是 67
场景，但自己的基线是同一套采集方式采的。`.scratch/sweep34` 正被别的 track 占着。）

## 4 · 性能数字（本刀欠的）

10 000 条 record × 5 列 = 60 000 个 change，`cargo test --release --lib -- --ignored --nocapture
database_store::probe`，原始行 `benchmarks/results/2026-09-22-track3-d1-window.jsonl`（三次运行）：

| 读数 | 第一次 | 第二次 | 第三次（含诊断） |
|------|--------|--------|------------------|
| 落库 10 000 行 | 635.8 ms（63.6 µs/行） | 939.8 ms（94.0 µs/行） | **848.0 ms（84.8 µs/行，53 205 change/s）** |
| 窗口读（顶，`LIMIT 31 OFFSET 0`） | 247.2 µs | 658.7 µs | **463.5 µs** |
| 窗口读（中，`LIMIT 39 OFFSET 4992`） | 5.1 ms | 8.8 ms | **7.5 ms** |
| 窗口读（底，`LIMIT 31 OFFSET 9969`） | 8.5 ms | 13.3 ms | **12.9 ms** |
| `realized_rows`（`COUNT(*)` + 投影 + 那次查询） | 0.45 ms | 0.68 ms | **0.70 ms** |
| 对照：一次取回全部 10 000 行 | 46.8 ms / 1 842 780 B | 69.9 ms / 1 842 780 B | **56.0 ms / 1 842 780 B** |
| 窗口那 31 行占堆 | 5 576 B | 5 576 B | **5 576 B（全表的 1/330）** |
| 进程 private（建库前后） | 3.1 → 5.0 MB | 2.9 → 4.9 MB | **2.9 → 4.5 MB** |

**证据（第三次运行）**：查询被交给 SQLite 的原文、绑定的值（最后两个就是 `LIMIT`/`OFFSET`）与它选的计划：

```
SELECT r.id, COALESCE(p.title, t.text), v0.text, v0.num, v0.flag, … (每个可见属性一组)
FROM db_records r
  LEFT JOIN pages p ON p.id = r.page
  LEFT JOIN db_values t ON t.record = r.id AND t.property = ?1
  LEFT JOIN db_values v0 ON v0.record = r.id AND v0.property = ?2   … 
WHERE r.db = ?6 ORDER BY r.ord, r.id LIMIT ?7 OFFSET ?8
binds: [1, 2, 3, 4, 5, 1, 31, 0]
plan:
  SEARCH r USING INDEX idx_db_records_db_ord (db=?)
  SEARCH p USING INTEGER PRIMARY KEY (rowid=?) LEFT-JOIN
  SEARCH t USING INDEX sqlite_autoindex_db_values_1 (record=? AND property=?) LEFT-JOIN
  SEARCH v0/v1/v2/v3 USING INDEX sqlite_autoindex_db_values_1 (record=? AND property=?) LEFT-JOIN
```

**这三次的比值稳定（330–332×），绝对值漂 1.4–2.7×**：同一台机器上还有另外三条 track 在编、在跑测试，
这是本项目记录过的「会话漂移」。数字要当成数量级读，比值才是结论。

### 4.1 量出来的真问题：窗口界住的是**行**，不是**工作量**

上表里「底部一次窗口读 12.9 ms」不是笔误。三次追加的读数把它拆开了（第三次运行）：

| 同一窗口的三种问法 | 耗时 |
|--------------------|------|
| 裸索引走位（`SELECT id … LIMIT 31 OFFSET 9969`，没有 JOIN） | **55.3 µs** |
| 本刀的实现（`LIMIT 31 OFFSET 9969` + 6 个 LEFT JOIN） | **12 874 µs** |
| 同一个窗口改用游标（`(r.ord, r.id) > (?, ?)` + 同样的 JOIN） | **251.1 µs** |

读法：`OFFSET` 本身很便宜，贵的是 **`LEFT JOIN` 在跳过的 9 969 行上照样执行**（计划里 JOIN 在
外层循环体内）。所以红线那句「先算窗口再取行」在**行与字节**上成立（31 行 / 5 576 B / 330×），
在**时间**上并不成立：滚到 10 000 行的底部，一次读要 12.9 ms，而按游标问只要 251 µs（**51×**）。
修法是把读契约从 `(limit, offset)` 换成一行一个 key 的游标，这是 D0 定的 `RowWindow::fetch()` 要改，
**属于 D3/D4**（要有真帧才谈得上「够不够快」）；本刀把数字钉在这里，D4 那一刀要打败它。

## 5 · record 与 page 的生命周期（ADR-0063 落地）

* **owner**：`db_records.page` 的 `UNIQUE (page)` —— 一个 page 至多是一个 record 的脸；
  批量 NULL 合法（懒建页的前提）。
* **CASCADE**：`ON DELETE CASCADE` 是 SQL 兜底，`a_row_dies_with_the_page_it_owns_and_both_delete_paths_end_there`
  两条路径（视图的行菜单 `[RecordDeleted, PageDeleted]` 与侧边栏只发 `PageDeleted`）**结束在同一个状态**，
  并且逆批次能把 record、page 与它的值一起放回来。
* **标题只有一个家**：有 page 读 `pages.title`，无 page 读 `db_values`，查询里一个 `COALESCE`；
  「打开」（建页 + 搬家）与它的逆（搬回 + 清指针、页面留在树里）都有测试。
* **懒建页**：`Record::bare` 是唯一构造器，`RecordPageSet` 是唯一的指针写入。

## 6 · 偏离与原因

1. **`CellValue::Number` 用位模式相等**（手写 `PartialEq` + `impl Eq`）：`Change` 必须 `Eq`
   （`core::document::Entry` 是 `Eq`，那是 undo 的单位），而 `f64` 不是。位相等在这里是**真**的等价关系
   （NaN 等于自己、`0.0` 不等于 `-0.0`），所以 `Eq` 不是谎报。
2. **写入按「形状」落列，读取按「kind」落列**：一个 `CellSet` 只写一行 / 一张表，所以写入不需要为
   验证 kind 多一次 `SELECT`（D6 要量「改一个 cell」的成本）。代价是「把 `Text` 写进 multi-select 列」
   会读到空——这是调用方的契约（D2/D3 拿着 kind 写），**有一条测试把它钉住而不是假装没有**。
3. **`replace_all` 加了快照/恢复**（ADR-0066）：这是 `DELETE FROM pages` 会级联掉 page-backed record
   造成的，属于本刀引入的 CASCADE 的必然后果；不修的话「检查点/修复」会静默吃掉数据库的行。
4. **没有把 record/值放进 `PersistedState`**（那是更「自然」的修法）：那会让检查点与 LAN 拉取把每个
   数据库的每一行都装进内存并放上传输格式，正是 ADR-0067 要避免的。理由写在 ADR-0066 里。
5. **`unwindowed_rows` 是公开 API**：它是测量对照臂，也是 ADR-0065 导出那条路的未来接缝，文档里写明
   「不是读取路径」。没有它就没有本刀的对照数字。
6. **没有实现游标读**（§4.1）：D0 的读契约就是 `LIMIT`/`OFFSET`，而换成游标是滚动模型的变化，
   属于画视图的那一刀；本刀只把「差 51×」量出来。

## 7 · 未验证（诚实清单）

1. **没有任何帧**：没有 `.slint` 视图，没有 `bench.ps1` 臂。31 行 / 5 576 B 是**查询与模型**的读数，
   不是画出来的 delegate 数；行高 32 px、overscan 8 仍是 D0 的假设，D3 要重测。
2. **没有键**：database 块、`blocks.db_ref`、六行插入菜单一个都没点亮（ADR-0060 的六个接点整批留待
   D3）。所以本刀的所有代码路径今天只有测试在调。
3. **`OFFSET` 的时间**只在本机与本刀自己的对照比过（§4.1），没有跨机器、没有跨版本对照构建。
4. **批量路径没有端到端跑过 LAN pull**（ADR-0066 的未验证条）。
5. **改 kind 不迁移值**：`PropertyKindSet` 只改类型，值的列不动（D2 的逐类型转换）。
6. **`config` 只被原样存取**：没有 JSON 解析器，选项名、日期格式、rollup 目标都还是 D2 的事。
7. **ADR-0065 仍是规格**：没有代码路径把 database 渲染成 Markdown 表格，`export_page` 的签名也还没改。
8. **性能数字里没有 filter/sort**（D4）、没有公式（D6）、没有视图切换（D3/D5）。

## 8 · 给整合者的注意事项

1. **共享工作树里，本刀的 commit 是用「只暂存自己那一段」的方式做的**（D0 的方法，这次多两个文件）：
   `docs/DECISIONS.md`（排除 Track 2 的 ADR-0050/0051 与 Track 4 的 ADR-0080/0081）、`PLAN.md`
   （排除 Track 2 的 `## M13`）、`docs/SPEC.md`（排除 Track 2 的 §三十七/§四十 hunk）、
   `src/storage/repository.rs`（排除 Track 2 的 `Mark::from_stored` / `stored_payload`）、
   `src/storage/migrations.rs`（排除 Track 2 的 v16）、`tests/integration/storage_test.rs`
   （排除 Track 2 的 mention 往返测试与两处 Mark 字面量）。做法是
   `git show HEAD:<file>` + 本刀改动 → `git hash-object -w --path <file>` + `git update-index --cacheinfo`。
   **提交后这些文件在工作树里仍是 modified**（那是别人的改动），不是脏数据。
2. **`CURRENT_VERSION` 现在是 16（工作树）**：本刀的提交里是 **15**（12–15 是本刀的四步）。
   合并时 v12–v16 会自然接在一起。
3. **ADR-0066 / ADR-0067 需要进 `docs/DECISIONS.md` 的收口**（本文已给全文，已在文件末尾）。
4. **CHANGELOG**：本刀**没有用户可见变化**（没有 UI、没有点亮任何占位），既有那句仍然为真、不要改。
   若要为状态留一笔，属于 Known limitations：
   > - The database layer's storage is in place (six tables, schema v12–v15, a windowed row read that
   >   really runs `LIMIT`/`OFFSET`), but nothing in the UI opens a database yet and the six
   >   insert-menu placeholders are still muted.
5. **`docs/PERFORMANCE.md` 本刀没动**：§Method 要的是「release 二进制 + 对照构建」，本刀两者都没有
   （没有 UI 臂）。数字进 `benchmarks/results/2026-09-22-track3-d1-window.jsonl`，D3/D8 收口。
6. **下一刀（D3）欠的第一件事是键**：把 `.scratch/track3-sweep-d1` 的 67 张相同像素当基线正好说明
   ——本刀一个像素都没动，所以 D3 点亮插入菜单时 `menu` / `plus` / `slash` 一定会动，那正是要看的数字。
7. **游标读（§4.1）**：如果 D3 的滚动帧出现 10 ms 级的 hitch，第一嫌疑就是它，不要先怀疑 Slint。

## 9 · 我注意到、但没碰的别人 territory 的问题

1. **共享树中途变红两次，两次都不是本刀的文件**：第一次是 `MarkKind::Mention`/`Date` 的消费者未跟上
   （`import_service` / `export_service` / `command.rs` / `state.rs`）；第二次是 `AppState::apply_mention`
   的返回类型在半改状态（`bool` vs `Option<Vec<Change>>`）。两次都在我重试等待期间被对方修好，
   本刀的三条门槛**在它们修好之后**跑的。
2. **Track 2 的 marks 没有新列**：mention/date 的载荷被他们塞进 `marks.url` 这一列
   （`Mark::stored_payload` / `from_stored`），所以 `CURRENT_VERSION` 只被 v16 的两个索引推高一次。
   这是他们的设计，我**不改也不评论对错**，只是记录：那个编码意味着 `url` 不再是「一个 URL」，
   而 ADR-0050 里应该写了这件事。
3. **`.scratch/` 是四条 track 共用的**：本刀的 sweep 用 `.scratch/track3-sweep-d1` 而不是
   `.scratch/sweep34`（后者被别的 track 占着），同理 `.scratch/track3-d1/` 是本刀的日志目录。
4. **`docs/SPEC.md` 的行数**：本机 `Get-Content` 少算（`\r`），python 读是 2595 行。老坑，再记一次。
# Track 3 — Database（D2：property 系统）

D1 让窗口真的从 SQL 里取行，D2 让窗口里的每一格有意义：SPEC §三十九 的 **14 种属性类型**各自的
输入与渲染规则、**两个派生时间列**（不许双写）、以及「**排序必须在 SQL 侧**」这条红线。`person` 按
ADR-0061 的形态降级。仍然**没有 UI**：没有 `.slint`、没有块种类、插入菜单那六行占位仍不可选（D3）。

## 1 · 本刀改了哪些文件

| 文件 | 为什么 | 关键位置 |
|------|--------|----------|
| `src/core/database_property.rs`（**新**，1732 行） | 属性的全部语义：选项列表、格式、输入规则、渲染规则、全仓库唯一的 JSON 阅读器 | `PropertyOptions` L100、`to_config` L148、`option_named` L204、`NumberFormat` L258、`DateFormat` L298、`parse_one` L365、`parse_many` L459、`iso_date` L516、`looks_valid` L557、`AttachmentNames` L610、`paint` L639、`mod json` L725（深度上限、转义、代理对）、测试 L1121 起 20 条 |
| `src/core/database.rs` | 「两个时间 kind 不是文本值」与「排序是编译好的词」 | `is_derived` L459、`sort_column` L476、`ValueKind::Derived` L508/L527、`SortColumn` L533、`SortSpec` L556、`RecordTimestamps` L583、`RowRequest::sort` L821（+`RowRequest::new`）、`CellValue::display` 的注释改成「值自己的形态」 |
| `src/storage/migrations.rs` | v17：`db_records` 的两个时间列（一步一个语义单位） | `CURRENT_VERSION` L14（**16 → 17**）、v17 步 L344、`add_record_timestamp_columns` L402（「缺哪列补哪列」的范式） |
| `src/storage/database_store.rs` | SQL 侧的三件事：派生列、排序、渲染 | `NOW` L67、`read_record_timestamps` L405、`derived_cell` L426、`cell` 的派生分支 L265、`workspace_people` L334、`row_query_plan` L510、`sort_expression` L591、`read_attachment_names` L767、`insert_record` 盖章 L990、`touch_record` L1019、`touch_edited_by_page` L1034、`set_cell` 末尾的 bump L1087、快照/恢复带上两列 L1252 起、测试 L1425 起 10 条、探针 L3555 |
| `src/storage/repository.rs` | 页标题改名要动 record 的 `edited`（跨模块的唯一一处） | `PageTitleSet` 臂里的 `database_store::touch_edited_by_page` L608 |
| `tests/integration/storage_test.rs` | v17 迁移测试 + 批量路径的生日 + D1 那两处 `RowRequest` 字面量 | `mod database_property_layer` L2341（**末尾新增模块**，不碰 D1 与 Track 2 的 `database_layer`）；两处 `RowRequest::new(...)` |
| `docs/DECISIONS.md` | ADR-0068…ADR-0071（文件末尾，217 行） | 时间列 / 输入与渲染 / 排序 / person |
| `docs/SPEC.md` | §三十九「属性类型」小节标注已交付 + ADR 号 | 三处 hunk（属性类型段、person 降级行、created/last edited 的来源那句） |
| `PLAN.md` | 末尾追加 `## Track 3 · D2 属性系统` | 文件末尾 |
| `benchmarks/results/2026-09-22-track3-d2-sort.jsonl`（**新**） | 探针的原始 JSON 行（三次运行） | 3 行 |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`、`[profile.release]`、
任何 `ui/*.slint`、任何 Track 1/2/4 的功能文件。**零新依赖**（`Cargo.toml` 一个字没改）。

## 2 · 迁移号（串行接缝）

* 动手前读 `src/storage/migrations.rs`：`CURRENT_VERSION = 16`（Track 2 的 `reference lookup` 步）。
* 本刀采用 **v17**（`db_records.created` / `.edited`）。
* **提交树里这一步仍是 v17，而数组里 16 是缺的**：本刀的提交 blob 建在 HEAD（v15 收尾）之上，Track 2 的
  v16 还没提交，所以 blob 的数组是 `[…12…15, 17]` 且 `CURRENT_VERSION = 17`。这不是重号——迁移 runner
  （`ensure_current`）只按数组顺序应用「version > 文件当前版本」的步，**跳号是允许的**，全新库走完
  1…15 再到 17 即 `CURRENT_VERSION`，v16 库（Track 2 写的）打开时 `16 > 17` 为假、只会补应用 v17。两个
  分支合并后就是连续的 12…17。**没有把 v17 改成 v16**：那会让 master 上出现两个 v16 步，第二个会被
  runner 的 `version <= from` 静默跳过——那才是真的数据损坏。
* 本步是 `sql: ""` + `backfill`（`add_record_timestamp_columns`），照 v10/v11 立的范式「缺哪列补哪列」，
  所以半途的库收敛而不是报错；v17 的库**不发明生日**（旧行两列是 `''`，读出来是空格子）。

## 3 · 命令与门槛结果（**工作树**）

```
cargo check --all-targets   → Finished，0 warning
cargo test --all-targets    → 全绿，按 target 分开：
  unittests src/lib.rs                312 passed / 0 failed / 13 ignored
  unittests src/main.rs                 0 / 0 / 0
  unittests src/bin/quire_typing        0 / 0 / 0
  tests/integration/backup_test        13 / 0 / 1
  tests/integration/find_test           9 / 0 / 0
  tests/integration/markdown_test      69 / 0 / 0
  tests/integration/persistence_test    5 / 0 / 0
  tests/integration/search_test        17 / 0 / 0
  tests/integration/storage_test       39 / 0 / 2
  tests/integration/workspace_test     14 / 0 / 0
  ── 10 个 target 合计 478 passed / 0 failed / 16 ignored
cargo build --release       → Finished，**0 warning**
```

**提交的树单独跑过一遍**（本刀提交时把别人的 hunk 排除在外，所以「提交树能不能编、测试过不过」不是
推论题）：独立 worktree（`git worktree add .scratch/track3-d2/verify HEAD --detach`，共用 `target` 目录，
不动共享工作树），把 `.scratch/track3-d2/stage/` 里**将要提交的那 12 个 blob** 覆盖进去再跑：

```
cargo check --all-targets → Finished，0 warning
cargo test --all-targets  → 全绿，按 target 分开：
  unittests src/lib.rs   295 passed / 0 failed / 12 ignored
  backup 13 / find 9 / markdown 62 / persistence 5 / search 17 / storage 36 / workspace 14
  ── 10 个 target 合计 451 passed / 0 failed / 15 ignored
cargo build --release     → Finished，**0 warning**
```

（比工作树少是因为那棵树里没有 Track 2 / Track 4 的未提交测试：478 − 451 = 27。lib 由 D1 提交树的 264
涨到 295 = **+31**，正好是本刀新增的 31 条；storage 由 34 涨到 36 = 本刀的 2 条集成测试。）

**这一步又抓到一个真错误，值得记下来**（D1 那次抓到的是 `E0583`，这次是另一类，方向相反）：第一次提交树
检查在 `tests/integration/storage_test.rs:1902 / 1993` 报 `E0063: missing field sort` —— 那两处
`RowRequest { … }` 字面量是 **D1 自己的两条测试**写的，在 HEAD 的文本里是旧形状；本刀给 `RowRequest`
加了 `sort` 字段，而我的 blob 是「HEAD 的文本 + 我新加的模块」，于是**旧字面量留在了提交树里**。
修法是把那两行（`RowRequest::new(db.id, PropertyId(1), &columns)` 与
`&RowRequest::new(db.id, PropertyId(1), &[])`）也算进本刀的 diff。教训与 D1 那条同源：**排除了别人的
hunk 之后，HEAD 的旧代码要一起面对新 API**——「提交树单独跑一遍」是唯一能发现它的地方。给整合者：
任何在 `RowRequest { … }` 字面量上构造请求的测试（Track 2 若在自己的测试里也写了）合并后都要改这一行。

本刀新增 **33 条测试 + 1 条打印型探针**（31 条在 lib 里，2 条在 storage 集成里）：

* `core::database_property::tests` **20 条**：JSON 阅读器（写出-读回、转义与代理对、拒绝非文档、深度上限、
  id 语义）、选项列表（ADR-0061 的文档往返、按 id 改名不动值、get-or-add 不复用 id、坏文档折成空表、
  重复 id 取第一个）、两种格式（整数/百分比/未知格式折默认、日期的两个默认值、不发明时间）、输入规则
  （空输入 = 无值、文本逐字、数字的接受与拒绝名单、日期的形状 vs 日历、勾选词表、三个字符串 kind 的
  **不改写**、select 的 id 与名字、列表两种入口、存储型 kind 拒绝一切写入）、渲染（选项名/未知 id 显示
  自己/文件名字与回退/格式）。
* `core::database::tests` **+1**：`sort_column` 每种 kind 落哪一列 + 哪五种没有 `SortSpec`。
* `storage::database_store::tests` **+10**：每型往返（含 paint）、边界（负数/小数/0/空串/5 万字符/非法
  email·url·phone/日期形状/列表重复项）、**数字序 vs 字节序的对照**（同一对 `2`/`10` 排两种序）、
  **日期序依赖定宽**（写一个不合形的 `2026-9-2` 进去看顺序坏掉）、隐藏列排序（多一个 join + 计划里有
  `TEMP B-TREE`、没有 `SCAN db_values`）、派生列是 record 自己的（含「偷写的值没人读」）、files 渲染名字与
  无名回退、改名选项不动值、person 的折叠与派生名单（并检查没有成员表）、批量路径保住生日。
* `tests/integration/storage_test.rs` 的 `mod database_property_layer` **2 条**：
  `the_v17_step_adds_the_record_timestamps_to_a_v16_database`（真 v16 文件：DROP COLUMN + 版本回退，
  升上来后旧行逐字段没变、新行有章、旧行空格子）、
  `a_bulk_replace_keeps_the_birthday_of_the_records_it_keeps`（端到端 `replace_all`）。

**视觉**：本刀是纯数据层，预期 changed 0，实测 changed 0：

```
pwsh benchmarks/scripts/sweep.ps1 -OutDir .scratch/track3-sweep-d2 -Baseline .scratch/sweep33
  sweep at commit: d65ad3e
  vs baseline .scratch/sweep33 (67 scenes):
    changed    0:
    identical  67: default.png … dark-page-icon.png（全部 67 张逐字节相同）
    new        10: mention.png date.png backlinks.png backlinks-open.png backlinks-small.png
                  dangling.png dark-mention.png dark-date.png dark-backlinks.png dark-dangling.png
```

那 10 个 `new` **不是本刀的场景**：它们是 Track 2 未提交的 mention/date/backlinks 场景，出现在我的
sweep 目录里是因为我编的 `quire-shot` 里带着他们的代码（基线 `.scratch/sweep33` 早于它们）。67 个既有
场景逐字节相同 = 本刀一个像素都没动。

## 4 · 性能数字（本刀欠的）

10 000 条 record × 2 列（一个 number、一个 date，数值是 `(i × 7919) mod 10007`，插入序与值序都不是答案），
`cargo test --release --lib -- --ignored --nocapture a_sorted_window_costs`，原始行
`benchmarks/results/2026-09-22-track3-d2-sort.jsonl`（三次运行）：

| 读数 | 三次 |
|------|------|
| 落库 10 000 行 + 30 000 格 | 422.0 / 494.5 / 492.6 ms |
| **SQL 排序窗口（顶，`LIMIT 31 OFFSET 0`）** | **6360 / 6161 / 6656 µs** |
| SQL 排序窗口（底，`LIMIT 31 OFFSET 9969`） | 13 209 / 13 148 / 13 586 µs |
| 同一个底部窗口**不排序** | 4944 / 4501 / 4766 µs（**排序给一次滚动加 ~8 ms**） |
| 排序但取全部 10 000 行（顺序本身的价钱） | 19.1 / 18.9 / 19.8 ms |
| 日期降序窗口（顶） | 7796 / 6757 / 6445 µs |
| **对照：取回全部再在内存里排** | 13.3 / 15.5 / 13.5 ms，堆 **1 220 000 B** |
| 排序窗口那 31 行的堆 | **3 782 B**（三次逐字节相同，全表的 **323×**） |
| 一次单元格写入（自己一个事务 / 批量） | 5489 / 7395 / 6931 µs · **10.7 / 14.9 / 10.6 µs** |

**证据（每次运行都打印）**：被交给 SQLite 的原文、绑定的值、以及它选的计划——

```
SELECT r.id, COALESCE(p.title, t.text), v0.text, v0.num, v0.flag, v1.text, v1.num, v1.flag
FROM db_records r
  LEFT JOIN pages p ON p.id = r.page
  LEFT JOIN db_values t ON t.record = r.id AND t.property = ?1
  LEFT JOIN db_values v0 ON v0.record = r.id AND v0.property = ?2
  LEFT JOIN db_values v1 ON v1.record = r.id AND v1.property = ?3
WHERE r.db = ?4 ORDER BY (v0.num IS NULL) ASC, v0.num ASC, r.ord, r.id LIMIT ?5 OFFSET ?6
binds: [1, 2, 3, 1, 31, 0]
plan:
  SEARCH r USING INDEX idx_db_records_db_ord (db=?)
  SEARCH p USING INTEGER PRIMARY KEY (rowid=?) LEFT-JOIN
  SEARCH t / v0 / v1 USING INDEX sqlite_autoindex_db_values_1 (record=? AND property=?) LEFT-JOIN
  USE TEMP B-TREE FOR ORDER BY
```

**读法（诚实的一面）**：排序在 SQL 侧赢得的是**内存**（323×）与**红线**（顺序是数据库的，不是副本的），
但**时间上只赢 2×**（6.4 ms vs 14.0 ms）：因为 `db_values.num` 上没有索引，SQL 必须为 10 000 行建临时
B 树（`USE TEMP B-TREE FOR ORDER BY` 就是它），而 Rust 排 10 000 个 f64 只要 0.36 ms——贵的是**取回那
10 000 行**（13 ms）。D1 量出的「底部 `OFFSET` 12.9 ms」在本刀里变成「排序 + 底部 = 13.2 ms」（同会话
的不排序底部才 4.9 ms），这两个数一起说明：**红利在行数与内存上，不在 CPU 上**；D4 若要更快，第一嫌疑
是给 `db_values(property, num)` 加索引与换游标读，而不是把排序挪回 Rust。

**一次单元格写入**（D6 的欠账先还一半）：自己一个事务是 **5.5–7.4 ms**，批量（一个事务包 10 000 次）
**10.6–14.9 µs** —— 差值几乎全是 commit/fsync，而 ADR-0068 给每次写入多加的那一条
`UPDATE db_records SET edited = …` 就含在 10.7 µs 里。

## 5 · 15 种类型各自的落库形态（一句话一个）

| kind | 存哪 | 读回来是什么 |
|------|------|--------------|
| `title` | `db_values.text`；record 有页时读的是 `pages.title`（ADR-0063 的 `COALESCE`，两个家只有一个真值） | 字符串，原样 |
| `text` | `db_values.text` | 原样：不裁剪、不折叠大小写、没有长度上限（5 万字符的测试） |
| `number` | `db_values.num`（REAL，可索引、可按数值比较） | 有限浮点；无行 = 无值，**不是 0** |
| `select` | `db_values.text` = **选项 id** | 按 `config` 的选项表渲染成选项**名字**；名单里没有这个 id 就显示 id 自己 |
| `multi-select` | `db_value_items` 每项一行 = 选项 id（显示顺序 = 行序） | 选项名字用 `", "` 拼起来；重复项保留 |
| `status` | 与 select 同形（三组语义是 D5 的 board 的事） | 同上 |
| `date` | `db_values.text` = `YYYY-MM-DD` 或 `YYYY-MM-DDTHH:MM`（定宽，字节序即时间序） | 按 `DateFormat` 决定显示几天/几分；存的是日期就绝不发明 `00:00` |
| `checkbox` | `db_values.flag`（`Flag(false)` 是一行真值，`Empty` 才是没勾过） | `Yes` / `No`（ADR-0065 的词，导出表格同词） |
| `url` / `email` / `phone` | `db_values.text`，**逐字** | 原样；`looks_valid` 只给编辑器一个提示，从不拒绝也从不改写 |
| `files` | `db_value_items` = `attachments.id`（ADR-0029/0030 的**同一套**附件通道，没有第二套落盘） | `attachments.name`；附件行没了就显示 id 自己 |
| `created time` | `db_records.created`（ADR-0068，v17），**不进 `db_values`** | 记录出生那一刻（`YYYY-MM-DDTHH:MM`），改名/编辑都不动它 |
| `last edited time` | `db_records.edited`，**不进 `db_values`** | 内容变更时随写路径走到「现在」（格与页标题算内容，拖行/挂页签不算） |
| `person`（降级） | `db_values.text` = 一个名字（kind 在装载时折成 `text`） | 字符串；成员名单是 `workspace_people()` 从值里现算的去重名字，**没有成员表、没有 id、没有账号** |

## 6 · 偏离与原因

1. **`parse_many` 为 multi-select 拒绝了「列表里没有的名字」**（不自动新建选项）：自动新建需要同时改
   `config`，那是同一批里的第二个 change，而 `Change` 里**还没有**写 config 的臂。`PropertyOptions::option_named`
   已经给了 get-or-add 的形状，臂跟着 D3 的选项编辑器一起加（已写进 ADR-0070 的未验证条）。
2. **`Record` 结构体没有加 `created` / `edited` 字段**：加了就意味着一份 `Change` 能带一个「生日」进库
   （ADR-0068 明确禁止），而且会把 Track 2 写在 `database_layer` 里的 `Record { … }` 字面量全部弄红。
   两个时间戳只能经 `record_timestamps()` 读、只能由写路径盖。
3. **`ValueKind::Derived` 是新变体**，把 D1 的 `each_kind_names_the_column_or_table_its_value_lives_in`
   里那两条断言（created/last edited 曾是 `Text`）改成 `Derived`：ADR-0062 原话就是「这两个 §三十九 类型
   本 ADR **不落**、由 D2 带自己的 ADR 落」，所以这是落地不是改口径。
4. **`cell()` 不再自己调 `self.record_timestamps()`**：那会在持有连接锁时再锁一次（本项目用的是非可重入
   `Mutex`）。SQL 移到自由函数 `read_record_timestamps(&Connection, …)`，方法只是「加锁 + 调用」。
   这个坑是**跑测试跑出来的**（两条测试挂死 > 60 s），不是读出来的——写进 ADR-0068 的注释里了。
5. **`sort` 进了 `RowRequest`**（而不是另开一个 `window_rows_sorted`）：一次读 = 一个完整的问题，SQL
   也只有一处能长出 `ORDER BY`；`RowRequest::new()` 让「不排序」的调用点比原来短。代价见 §3 的提交树
   教训：字段一旦加上，所有 `RowRequest { … }` 字面量都要改。
6. **`sql: ""` + backfill 而不是一步 `ALTER TABLE`**：照 v10/v11 的收敛范式（半途的库不报错）；代价是
   v17 的库升级时多一次 `pragma_table_info`。
7. **批量路径的快照/恢复带上两列**（`DatabaseSnapshot` 的 record 元组从 4 个元素变 6 个）：不加的话
   检查点/修复/LAN pull 会把每个 record 的生日重置——D1 的 ADR-0066 就是为这类「顺手弄丢」写的。

## 7 · 未验证（诚实清单）

1. **没有任何帧**：本刀没有 `.slint`，没有任何格子被画出来；`looks_valid` 的阈值（几个数字算电话、
   域名要不要点）是作者定的，没有一个用户量过；`bench.ps1` 没有数据库臂（D3 的欠账）。
2. **视图文档还没被编译成 `SortSpec`**：ADR-0064 的 JSON → `Option<SortSpec>` 的编译（多列排序、分组头
   在窗口里的位置）是 D4 的；本刀只给了**编译结果**的形状与 SQL。
3. **没有过滤的数字**（D4 的）；**没有 `DISTINCT` 名单的开销**（`workspace_people()` 没量）。
4. **排序只在本机、与本刀自己的对照臂比过**：三次运行的绝对数漂 1.08×（同一棵树、同一台机器，另有别的
   track 在编），比值稳定（323× 逐字节相同）。
5. **改 kind 不迁移值**仍是 D1 的状态（`PropertyKindSet` 只改类型），本刀没有加逐类型转换——它需要每对
   类型的规则，D6/D7 之前没有用户能触发。
6. **选项列表的写入还没有 `Change` 臂**（见偏离 1）：`PropertyOptions::to_config()` 今天只有测试在调。
7. **`created` / `last edited` 的「刷新时机」表全部由测试断言**，但没有一条路径来自 UI 写入（没有 UI）。
8. **时区**：两个时间戳是**本机本地墙钟**（ADR-0062 给日期定的规矩），而 Track 2 的日期 atom 用的是 UTC
   （`core::date::today_iso`）。两处口径不同，本刀**不改他们的文件**，写进 ADR-0068 的未验证条请整合者
   仲裁。
9. **本报告与 PLAN / DECISIONS / SPEC 的最后几个 blob 是在提交树验证之后改的**（只有文档变，代码与测试
   的 blob 一个字节没动）——§3 里那套读数仍然对应提交的代码；文档改完又跑了一次
   `cargo check --all-targets` 确认干净。

## 8 · 给整合者的注意事项

1. **共享工作树里，本刀的 commit 仍然用「只暂存自己那一段」的做法**（D0/D1 的方法，这次靠
   `.scratch/track3-d2/base/` 里开工前对共享文件的快照 + 按名字排除）：
   `src/core/mod.rs`（排除 Track 2 的 `pub mod date;` / `pub mod reference;`）、
   `src/storage/migrations.rs`（排除 Track 2 的 v16；**本刀 blob 里 `CURRENT_VERSION = 17`、数组里缺 16**）、
   `src/storage/repository.rs`（排除 Track 2 的 `references` 等）、`docs/DECISIONS.md`（排除 Track 2 的
   ADR-0050/0051 与 Track 4 的 ADR-0080/0081）、`docs/SPEC.md`（排除 Track 2 的 §三十七/§四十 hunk）、
   `PLAN.md`（排除 Track 2 的 `## M13`）、`tests/integration/storage_test.rs`（本刀**追加**一个新模块 +
   改 D1 那两处 `RowRequest` 字面量，Track 2 与 D1 的 `database_layer` 其余一字不动）。**提交后这些文件
   在工作树里仍是 modified**（那是别人的改动），不是脏数据。
2. **迁移号**：合并后 master 应当是连续的 v12–v17（12–15 D1、16 Track 2、17 本刀）。**不要在合并时把
   v17 改成 v16**，理由见 §2。
3. **ADR-0068…ADR-0071 需要进 `docs/DECISIONS.md` 的收口**（全文已按文件既有风格追加在末尾）。
4. **CHANGELOG**：本刀**没有用户可见变化**（没有 UI、没有点亮任何占位），既有那句仍然为真、不要改。
   若要为状态留一笔，属于 Known limitations：
   > - The property system is stored but not drawn: all fourteen kinds round-trip through the store
   >   (with `created time` / `last edited time` derived from the record's own two columns, and sorts
   >   compiled into SQL), but no view exists yet and the six insert-menu placeholders are still muted.
5. **`docs/PERFORMANCE.md` 本刀没动**：§Method 要的是「release 二进制 + 对照构建」，本刀只有 release 探针
   与本刀自己的两个臂（有对照，但不是「前一刀的提交编出来」那种）。数字落在
   `benchmarks/results/2026-09-22-track3-d2-sort.jsonl`，D3/D8 收口。
6. **Track 4 的 `Cargo.toml` 仍然只有他们的 `hayro`**：本刀零新依赖，所以 `Cargo.toml` 本刀没改
   （提交里也不含它）。
7. **`docs/SPEC.md` 的行数与 `\r`**：老坑，SPEC.md 在树里是 CRLF，追加/替换时按文件原有行尾写。

## 9 · 我注意到、但没碰的别人 territory 的问题

1. **Track 2 的测试写在 D1 的 `mod database_layer` 里面**（为了复用 `migrated` / `roll_back`）：
   D1 的报告已经记过一次（那次导致 E0583）。本刀因此把集成测试放进**自己的新模块**，代价是
   `roll_back` 那套 helper 没有复用（我自己写了 `migrated` / `roll_back_timestamps` 十来行）。若整合者要
   统一，建议把 `roll_back` 挪成一个共享 helper 模块，而不是继续往 `database_layer` 里塞。
2. **`CURRENT_VERSION` 在四条 track 之间是纯串行接缝**，而本刀的提交树必然带一处跳号（§2）。这不是
   本刀的选择，是「谁的提交先落地」决定的；合并顺序若先合 Track 2，本刀那步会自动接在 v16 之后。
3. **`quire-shot`（debug）与 `quire`（release）用同一棵源码树**：本刀的 sweep 因此采到了 Track 2 的 10 个
   新场景（见 §3 结尾），读数要按「67 个既有场景逐字节相同」来看，而不是「10 个 new」。

# Track 3 — Database（D3：table view，代码刀）

D0 决策、D1 存储层、D2 属性系统之后，D3 把 §三十九 **画出来**并点亮 ADR-0060 的六个接点。
本刀遵守任务书铁律：**只写代码，一行 cargo 都没跑**（不 check / 不 build / 不 test / 不 run，
不跑 sweep）——编译、测试、视觉对照全部留给全部代码生成完之后的总测试。本节行号是**工作树**
（四条 track 未提交改动的合集）的行号。

## 1 · 开工时的现状（对任务书五个缺口的核对结论）

接手时上一轮 D3 的 WIP 比任务书记的多：`state.rs` 的投影层（`DbWindow` / `db_watch` /
`db_refresh` / `db_fill_row` / 12 个 `db_*` 写入 helper / `db_absorb`）、`command.rs` 的六个
Database 命令、`persistence.rs` 的 `BlockDbRefSet`、`document.rs` 的应用臂、`migrations.rs` 的
v18、`export_service.rs` 的 `DatabaseTable` + `render_database` + 导出臂**都已写好**。逐条核对
任务书的缺口：

| 缺口 | 核对结论 | 本刀动作 |
|------|----------|----------|
| 1. AppWindow 没注册三个组件 | **部分成立**。`DatabaseCell`/`DatabaseSwitcher` 由 `DatabaseView.slint` import（无需 AppWindow）；`DatabaseView` 已被 `EditorBlock.slint` import（L13）但**从未实例化**，`body-height` 也没有 kind 23 的臂；AppWindow 缺的是 `db-columns` 的**窗口级 popup** | `EditorBlock.slint` 实例化 `dbv := DatabaseView`（L1240）+ `db-height` 属性（L45）+ body-height 臂（L188）+ `ta` 点击排除（L211）；AppWindow 新增 `DatabaseColumnsPopup`（L188）+ 实例化（L675）+ show/close 接线（L386、L521） |
| 2. controller 没有 dispatch | **成立**（`on_db_*` 0 处） | `controller.rs` L1322–1520 新增 12 个 callback 接线 + 4 个 helper（`db_debounce_arm` L2557、`db_commit_cell` L2593、`db_refill_row` L2612、`db_push_columns` L2628） |
| 3. apply_scene 没有 database-table | **成立** | `apply_scene` 加 `database-table`（L3619）与 `dark-database-table`（L3487）两臂 + `seed_database_table`（L3213）；`quire_shot.rs` 的 `needs_db` 加 `contains("database")`（L176） |
| 4. Markdown 导出是否完整 | **导出器完整、调用侧断了**：`export_page_full` 的 `database` 参数两处调用都传 `&|_| None` | 两处调用点接上 `&|id| s.db_markdown_table(id.as_u64() as i32)`（L2949 clipboard、L2984 .md 导出）；`db_markdown_table` 开头补 `force_flush`（导出的是用户**正在看的**表，写队列必须先落） |
| 5. 菜单行是否真接上 | **一半**：`SLASH_ITEMS`/`INSERT_ITEMS`/`TURN_INTO` 的 `Table view` 行、`kind_from_int`/`kind_to_int` 的 23 都在（WIP）；但 `make_database` **0 个调用者**——slash-apply、Turn-into 都没有 Database 分支，菜单行点了会掉进 `SetBlockType` 的拒绝臂 | slash-apply 加分支（L1800）、Turn-into 加分支（L2105），都走 `state.make_database` |

## 2 · 本刀改了哪些文件

| 文件 | 为什么 | 关键位置（工作树行号） |
|------|--------|------------------------|
| `ui/components/EditorBlock.slint` | 实例化 DatabaseView；块的行高 = 视图自己的高度（页面的滚动 = 视图的滚动） | `db-height` L45、body-height 臂 L188、`ta` 排除 L211、`dbv :=` L1240 |
| `ui/AppWindow.slint` | 隐藏列的窗口级 popup（一行一属性，title 锁定），照 BlockMenuPopup 的三件套（component / 实例化 / is-open 镜像） | L188、L386、L521、L675 |
| `ui/Types.slint` | `db-editing-block`（提交路径要三件套才能定位一格）与 `db-cell-closed`（同步提交，见 §4.2） | L352、L542 |
| `ui/components/DatabaseCell.slint` | 关闭路径从「本地改两个属性」改为调 `db-cell-closed`——debounce 会在编辑器关闭后读回已清零的 id，最后 300 ms 的输入会被吞 | accepted/Escape/Tab 三处 |
| `ui/components/DatabaseView.slint` | 列 popup 的锚点从「两个 absolute-position 相加」修成只用 TouchArea 自己的（`absolute-position` 是**窗口相对**，相加是双重计数） | L218–231 |
| `ui/components/Editor.slint` | `editor-viewport-h` 此前**没有任何 .slint 写它**（投影只能吃 720 的默认值）——Editor 根部的 `changed height` 报告一次 | L13–20 |
| `src/app/controller.rs` | 12 个 db callback + 4 个 helper + 两个菜单分支 + 两处导出调用点 | 见 §1 表 |
| `src/app/state.rs` | `db_fill_row` 补上 `db_row_height`/`db_header_height`（此前从不赋值，Slint 默认 0px——**行高会塌成 0** 的真 bug）；`db_refresh` 的 offset 公式修正（见 §4.1）；`make_database` 同批清空行的字；`db_markdown_table` 先 flush | L4955、L4793、L5049 起、L5495 起 |
| `src/bin/quire_shot.rs` | `needs_db` 加 `contains("database")`（表画的是 record，record 在 SQL 里，无库的会话只能拍到空格子） | L176 |
| `docs/DECISIONS.md` | ADR-0072…0075（文件末尾） | 见 §6 |
| `docs/SPEC.md` | §三十九「视图」「操作」标注已交付 + ADR 号 | 两处 hunk |
| `PLAN.md` | 末尾追加 `## Track 3 · D3 table view` | 文件末尾 |
| `docs/REPORT_TRACK3.md` | 本节 | — |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`Cargo.toml`（本轮零新依赖）、`[profile.release]`、
任何 Track 1/2/4 的功能文件。

## 3 · 六个接点各落在哪（ADR-0060 的清单）

1. **types**：`src/core/types.rs` `BlockKind::Database`（L152）+ `ALL` 追加 + `db_ref` 字段
   （L400 附近）——WIP 已有，核对无误；
2. **kind 字符串**：`as_str` 的 `"database"`（types.rs L231）+ `state.rs` 的
   `kind_to_int`/`kind_from_int`（23 ↔ `BlockKind::Database`）——WIP 已有（重复臂已被修）；
3. **Markdown**：`export_service.rs` 的 `DatabaseTable`/`render_database`/导出臂（WIP）+
   本刀接上的两处调用点 + `state::db_markdown_table` 的 flush；
4. **Turn into**：controller L2105 分支 → `make_database`（`SetBlockType` 对 Database 的拒绝臂在
   `command.rs` L1131，WIP 已有）；
5. **slash / insert 菜单**：菜单行（WIP）+ 本刀的 slash-apply 分支（L1800）；
6. **截图场景**：`database-table` / `dark-database-table` + `seed_database_table` +
   quire_shot 的 `needs_db`。

## 4 · 写码时抓到的三个真 bug（都是「接线才看得见」的）

1. **窗口算术的双重计数**：delegate 报的锚是 body 顶**相对视口**的 px（`row-y - editor-scroll-y`），
   而 `db_refresh` 用 `offset = scroll - top` 把 scroll 又加了一次——任何非零滚动位置上取的窗口都
   不盖住视口。修正：`offset = (-top).max(0.0)`（body 顶在视口上方多少 px，就有多少 px 已经滚过去）。
   两半的约定都在注释里钉死了（state.rs L4780 起、DatabaseView.slint 的 `top-in-view`）。
2. **行高塌 0**：`Types.slint` 的 `db-row-height`/`db-header-height` 没有 Slint 默认值（0px），而
   `db_fill_row` 从不赋值——块的 `db-height` 因此是 header 0 + count×0，整张表画成一条线。修正：
   `db_fill_row` 开头写 `TableView::ROW_HEIGHT`/`HEADER_HEIGHT`（常数只有一个来源，投影的窗口算术
   与委托的摆放不可能再 disagree）。
3. **最后 300 ms 的输入会丢**：cell 的 Enter/Escape/Tab 原地清零 `db-editing-record/-property`，
   而 debounce 提交靠的就是这两个 id——先清零再等 300 ms 的提交，等于吞掉最后一次输入。修正：
   新增 `db-cell-closed` callback，关闭时同步提交再清零（`db_commit_cell`）。

## 5 · 已接 / 未接的行内编辑（D3 的诚实边界）

| 类型 | 状态 | 说明 |
|------|------|------|
| title / text | **已接** | 行内 `TextInput`，debounce 提交，`parse_one` 按列的 kind 解析（ADR-0069） |
| number | **已接** | 同上；编辑器里是 `0.5`，画出来是 `50%`（`db_cell_text` 给的是**存储形态**不是绘制形态） |
| checkbox | **已接** | 整格点击写 `Flag(!shown)`（16px 的方块是精度任务，Notion 同款） |
| select / status | **已接（读侧）** | 格内选项列表（一个 popup per realized row 不是 popup 是列表）；**写侧只能选已有选项**——「加一个选项」的 `Change` 臂仍欠着（D2 的偏离 1，D5 的选项编辑器） |
| date / url / email / phone | 未接输入控件 | 画值（ISO 文本原样）；date 的日历选择器是 D5 的。文本输入臂**其实能编辑它们**（parse_one 接受对应形态），但没有专门控件就不算交付 |
| multi-select / files | 未接 | 列表型的输入是 D5/D6 |
| created time / last edited time | 不可编辑（设计如此） | 派生列，`editable = false`，点击不开编辑器 |
| formula / rollup / relation | 不可编辑（设计如此） | 计算列，D6 |

## 6 · 四个新决策（ADR-0072…0075，全文在 DECISIONS.md 末尾）

| # | 一句话决策 | 代价（写进 ADR 的） |
|---|------------|---------------------|
| 0072 | 六张表的 id 是**会话水位**：启动时 `MAX(id)` 播种一次，只有 plan 成功才推进；`MakeDatabase` 等把 id 带**进**命令 | 被拒绝的命令不烧 id；没写库的 id 重启后遗忘（无人引用，无可观察的空洞） |
| 0073 | 「这个块正在看哪个视图」是**会话态**不是列：`db_active_view` 是 AppState 的一张 map，重启回到第一个视图 | 视图切换不进 undo、不写库；记忆跨重启需要等 D5 的 schema 问题 |
| 0074 | 视图文档**按文本读改写**：本刀只写 `columns`/`widths` 两个键，`filter`/`sorts`/`v` 原样透传 | 隐藏一列不会顺带清掉后一刀的过滤规则；undo 恢复的是整段文本 |
| 0075 | 内存 catalog 从 change 批次学习（apply/undo/redo 共用 `record()` 这一个漏斗），record 永不进 catalog | fold 方向与 storage 一致（change 说发生了什么，不说往哪个方向跑）；`DatabaseDeleted` 在内存里级联 |

## 7 · 迁移号（串行接缝）

* 动手前读 `src/storage/migrations.rs`：工作树 `CURRENT_VERSION = 19`（D2 的 v17 之后，
  Track 4 的 v19 `block sync pointer` 在我之后追加）。
* 本刀采用 **v18**（`blocks.db_ref`，`add_db_ref_column`，「缺哪列补哪列」的收敛范式，
  与 v5 的 `page_ref` 同形：可空、无外键、无索引）。**本刀的提交 blob 里 `CURRENT_VERSION = 18`**
  （HEAD 是 17 + 本刀的 18）；工作树里的 19 是 Track 4 的，合并后 12…19 连续。
* 未重号：12–15 是 D1 的，16 是 Track 2 的，17 是 D2 的，**18 是本刀的**，19 是 Track 4 的。

## 8 · 共享工作树的提交方式与三个已知交叠

1. **方法**：D0/D1/D2 的「只暂存自己那段」（`git show HEAD:<file>` + 本刀文本 →
   `git hash-object -w --path` + `git update-index --cacheinfo`），脚本在 `.scratch/track3-d3/stage.py`。
   提交后这些共享文件在工作树里仍是 modified（那是别人的改动），不是脏数据。
2. **导出调用点与 Track 2 交叠**：工作树里那两处 `export_page_full(...)` 调用是 Track 2 未提交的
   hunk（title_of 与 sync_of 两个闭包是他们的）。本刀**只把他们传的 `&|_| None`（database 位）换成
   db_markdown_table**，他们的行一字未动。**给整合者**：若 Track 2 先提交并重写这两处调用，
   请把 database 位的闭包带过去——那一行丢了，数据库导出就静默退回「什么都不写」。
3. **quire_shot 的 `needs_db` 与 Track 2 交叠**：同一道理——那块（含注释）是 Track 2 为 backlinks
   场景写的未提交 hunk，本刀在其条件上追加 `|| contains("database")` 并加了一段注释。
4. **合并痕迹三处**（两条 track 的未提交改动在同一文件尾相撞造成的复制）：`Types.slint` 的
   `db-layout`/`db-layout-ok` 重复字段、`state.rs` 的 `kind_from_int` 重复 `23 =>` 臂、
   `controller.rs` 的重复 `"backlinks-small"` 臂——本刀开工时都在，**等待期间被并行修掉了**
   （前两处在我的段落上，后一处是 Track 2 的段落），本刀没有再动。

## 9 · 未验证（诚实清单——因为一行 cargo 都没跑）

1. **编译**：本刀全部代码没有过 `cargo check`。静态自查做过（生成类型的字段名、callback 签名、
   借用次序、`Model`/`Rc` 的 import、`DbColumnToggle` 生成类型与 state 自建类型的双路径并存），
   但那不是编译器。
2. **测试**：没有跑任何已有测试；铁律禁止新增 `#[test]`，本刀没有写。
3. **视觉**：`database-table` / `dark-database-table` 两个场景从未渲染过；`seed_database_table`
   走真写路径（in-memory 库 + force_flush），但「5 行 4 列的表长什么样」没有像素证据。
4. **`absolute-position` 的语义**：按 Slint 文档（窗口相对、逻辑 px）写的锚点与 clamp；若 1.18 的
   实际语义不同（例如父相对），popup 会错位但不会崩——统一测试的第一张 shot 就能看出来。
5. **布局期回调的重入**：`changed top-in-view` 在布局期调进 Rust（`db_watch` 可能触发一次 SQL 读），
   窗口移动时 `db_refill_row` 会 `set_row_data`——高度不变（total 不变），预期无循环，但没有跑过。
6. **`exec_all` 里 `ReplaceText`+`MakeDatabase` 的一步 undo**：`exec_all` 跳过 plan 为 None 的命令、
   所有 plan 针对同一 pre-state（读的是 `command.rs` 源码），行为没有测试钉住。
7. **性能**：D1 量出的「底部窗口读 12.9 ms / 排序 +8 ms」原样有效（本刀没有换游标读，那是 D4 的）；
   D3 欠的「切换视图耗时」「行内编辑到重绘」两个数字见下面测试计划。

## 10 · 测试计划（留给最终统一测试——每项：验什么 / 建议测试名 / 量哪个数 / 怎么量）

**A. 编译与门槛**

1. 全树可编译、零警告：`cargo check --all-targets` / `cargo build --release`（看 warning 计数 = 0）；
2. 全测试不回归：`cargo test --all-targets`（D2 时 478 条的基线上，本刀不新增测试，允许别人新增）。

**B. 功能（headless 可证的）**

3. 数据库块在页面上画出一张 5 行 4 列的表：`quire-shot --scene database-table`，
   建议场景名 `database-table` / `dark-database-table`（已写）；**人工核对** header 64px、行高 32px、
   列对齐（表头与行的列来自同一次投影）；
4. 「行是动态的」：同一场景加 `--scroll-y 4000`，**数 delegate 数不变**（窗口 31 行左右）、
   行的内容随滚动换——量法：`--probe-blocks` 之外在 `db_refresh` 临时打印 `window.start`（不要提交），
   或对比 shot 像素；
5. 悬垂 ref：`DatabaseDeleted` 后块回退渲染 `(deleted database)`——建议测试名
   `a_dangling_db_ref_renders_one_muted_line`（headless：造块→undo 掉创建批→shot）；
6. 单元格写入路径：checkbox 点击 300 ms 后 `db_values.flag` 翻转、number 输入 `abc` 被拒且存储值
   不变——建议测试名 `a_checkbox_toggles_as_one_undo_step` /
   `a_number_cell_refuses_non_numbers_and_keeps_the_stored_value`；
7. 导出：带一个数据库的页 `Copy page as Markdown`，文件含当前视图的 GFM 表、页-backed 行标题是
   `quire://page/<id>`——建议测试名 `a_database_exports_as_the_view_it_is_showing`
   （`export_page_full` 纯函数级，直接测，不用 UI）；
8. 视图文档透传（ADR-0074）：带 `filter`/`sorts` 键的文档经一次「隐藏列」后两键原样保留——
   建议测试名 `hiding_a_column_keeps_the_keys_this_build_does_not_own`。

**C. 视觉与交互（人工 / sweep）**

9. sweep 对照 D2 基线：67 个既有场景应当逐字节相同，`database-table`、`dark-database-table` 为 new——
   `pwsh benchmarks/scripts/sweep.ps1 -OutDir .scratch/track3-sweep-d3 -Baseline .scratch/track3-sweep-d2`；
10. Columns popup 落点：点 `database-table` 场景里 Columns 按钮的坐标（`--click x,y`），
    popup 应出现在按钮正下方且不出窗——这一条同时验证 §9.4 的 `absolute-position` 语义；
11. slash 输入 `/table` → 选中 `Table view` → 空行变数据库（`--probe-blocks` 应看到 kind 23）、
    一次 Ctrl+Z 回到原段落（验 §9.6）。

**D. 性能（SPEC §三十九 欠的三个数字，本刀能收口的两个）**

12. **切换视图耗时**：今天每库只有一个视图，真正的切换数字在 D5 有第二个视图后才有——本刀能量的是
    「切换路径本身」：`db_pick_view` + `db_refresh` 的重读。量法：release 下在 `db_pick_view` 前后
    打印 `Instant::elapsed`（10 000 行库，参照 D1 的探针写法 `#[ignore]` 打印型测试），对照 D1 的
    窗口读数（顶 463 µs / 底 12.9 ms），数字进 `benchmarks/results/`；
13. **行内编辑到重绘**：一次 cell 写入的全路径 = flush（D2 量过：5.5–7.4 ms 单发 / 10.6–14.9 µs 批内）
    + `SetDatabaseCell` 计划 + 窗口重读。量法：同上探针，10 000 行库上量 `db_set_cell_text` 端到端，
    期望 ≈ D1 的窗口读 + D2 的单发写；**10 000 行的 RAM** 仍以 D1 的 31 行 / 5 576 B 为准
    （本刀没有改变「行只活在窗口里」的形状，bench.ps1 的数据库臂仍欠——D8 收口时一起）。

## 11 · 给整合者的注意事项

1. §8 的三条交叠（导出调用点、quire_shot 的 needs_db、并行修掉的三处合并痕迹）；
2. 迁移号：合并后 12…19 应连续（§7）；本刀提交 blob 里 `CURRENT_VERSION = 18`、数组缺 16
   （Track 2 的 v16 未提交）——runner 允许跳号，D2 的报告 §2 已论证过这是安全的形状；
3. 本刀的提交树**没有单独跑过门槛**（铁律禁止跑 cargo）——D1/D2 的「提交树单独验证」这一步
   请整合者在统一测试时补做，重点看 §9.1 的静态自查清单；
4. `DbColumnToggle` 有两个同名类型：`crate::DbColumnToggle`（slint 生成，popup 的模型行）与
   `crate::app::state::DbColumnToggle`（state 的构建形状）——并存是有意的（生成的类型不该从
   state 导出），控制器里 `db_push_columns` 是两者唯一的转换点。

# Track 3 — Database（D4：filter / sort / group，代码刀）

D3 画出了 table，D4 让视图**有规则**：过滤树、多键排序、分组，全部在 SQL 侧编译，全部持久化在
视图自己的 JSON 文档里。本刀遵守任务书铁律：**只写代码，一行 cargo 都没跑**（不 check / 不 build /
不 test / 不 run，不跑 sweep）——编译、测试、视觉对照、性能数字全部留给总测试。行号是**工作树**
（四条 track 未提交改动的合集）的行号。

## 1 · 核心立场：红线是模块边界，不是纪律

「filter / sort 在 SQL 侧完成，不在 UI 侧过滤」最容易违反，因为「取回再过滤」写起来最短。本刀把它
做成**结构**：

```
视图文档（db_views.definition 的 JSON，ADR-0064）
    │  core::database_view::ViewRules —— 按 schema 解析成类型化规则（纯，无 SQL）
    ▼
RowRequest { sorts: &[SortSpec], filter: Option<&FilterNode> }   ← 规则本身随请求走
    │  storage::database_query —— 唯一把规则变成 SQL 文本+绑定的模块
    ▼
一条语句：SELECT … FROM … WHERE db ∧ (树) ORDER BY (各键, 空值置后) …, r.ord, r.id LIMIT/OFFSET
    │  core::database::window —— 窗口算术，开在过滤后的计数上
    ▼
realize 的就是那一窗
```

三个契约（注释里都钉死了，测试要验）：

1. **计数先算、窗口后开**：过滤视图的总数是 `filtered_count`——`COUNT(*)` 跑在与行读**同样的
   `FROM`/`WHERE`** 上（`database_store.rs:437`）；过滤 10 000 行剩 3 行，窗口就 realize 3 行。
2. **tie-break 稳定**：每个排序键各一段（空值置后项永远升序，值项跟方向），最后统一 `r.ord, r.id`
   ——同一窗口的重读是同样的行（`database_query.rs:604` 的 `order_clause`）。
3. **规则从不单独成为一次内存过滤**：`RowRequest` 只借出规则，读路径上没有任何函数同时拿到
   「全部行」和「规则」（state.rs 的 `db_refresh` 注释里写成「作为借用的红线」）。

## 2 · 改了 / 新增了哪些文件（为什么 + 关键行号）

| 文件 | 为什么 | 关键位置（工作树行号） |
|------|--------|------------------------|
| `src/storage/database_query.rs`（**新**，655 行） | 本刀的核心：规则 → SQL 的编译器。`Sql`（带 `Value` 绑定与隐藏 join 别名分配 `s0,s1…`）、`build_from`（共用的 FROM+槽位）、`column_expr`（ADR-0070 的「每种 kind 比较在哪一列」决策，排序与过滤共用一处）、`row_query_plan`（窗口读）、`row_query_in_group`（组内切片读）、`count_query`、`group_query`、`node_predicate`/`clause_predicate`（WHERE 生成器）、`order_clause`（多键排序）、`group_predicate` | `Sql` L67、`hidden_join` L130、`build_from` L164、`column_expr` L213、`row_query_plan` L268、`row_query_in_group` L298、`count_query` L337、`group_query` L357、`node_predicate` L396、`clause_predicate` L469、`order_clause` L604、`group_predicate` L628 |
| `src/core/database_view.rs` | 规则的**纯**一半：`FilterOp`（10 种比较符 + 每种 kind 的可用名单 `ops_for` + 面板用词 `label`）、`FilterValue`（含 `Missing`=没填完的规则）、`FilterClause`/`FilterNode`（树）、`FlatFilter`（面板可表示的扁平子集）、`GroupSpec`/`GroupKey`、`group_window`（分组的条目窗口算术）、`ViewRules`（解析 + 降级 + note）、`ViewDefinition::set_filter/set_sorts/set_group`（写回三把键）、`is_stored_date`（日期值形状检查） | D4 段 L576 起；`FilterOp` L638、`FILTER_MAX_DEPTH` L603、`FilterClause` L880、`FilterNode` L892、`FlatFilter` L980、`GroupSpec` L1080、`GroupKey` L1102、`group_window` L1167、`ViewRules` L1227、`rules()` L1250、`set_*` L1333/1344/1366、`is_stored_date` L1543 |
| `src/core/database.rs` | `RowRequest` 长出 `sorts: &[SortSpec]`（替换 D2 的 `sort: Option<SortSpec>`，ADR-0070 预言的那一步）与 `filter: Option<&FilterNode>`；`SortSpec` 文档更新 | L868 起（字段 L876/L880） |
| `src/storage/database_store.rs` | 执行侧：`row_query_plan`/`sort_expression`/`Sql` 移去 database_query（store 只执行）；`read_rows` 拆成薄壳 + `run_row_query`（三种读形共用一个上色管道）；新增 `filtered_count` / `group_counts`（`GROUP BY` + `GroupKey` 归一化）/ `window_rows_in_group`；`realized_rows` 对带 filter 的请求改用 filtered 计数；测试/探针的 `req.sort` 六处、`RowRequest` 字面量两处机械补 `sorts`/`filter`（D2 先例）；`row_binds` 返回 `Vec<Value>`（过滤绑定异构：id / REAL / 文本） | `filtered_count` L437、`group_counts` L463、`window_rows_in_group` L498、`row_binds` L745、`read_rows` L751、`run_row_query` L765 |
| `src/storage/mod.rs` | 声明新模块 | `pub mod database_query;` L9 |
| `src/app/state.rs` | 接线：`db_refresh` 规则化（规则解析→计数→开窗→分组条目装配；缓存键加入 definition 文本）；`DbWindow` 加 `definition` 字段；`db_fill_row` 填规则头部状态（filter 计数/排序箭头/分组列/可见提示）；写路径 9 个 `db_filter_*` + `db_sort_cycle` + `db_group_pick`（全部经 `db_edit_definition` → `SetDatabaseViewDefinition`，一个 change 一步 undo）；面板数据 `db_filter_panel`/`db_ops_for_kind`/`db_property_options`/`db_group_choices`/`db_group_current`；`db_markdown_table` 带上规则（补上 ADR-0065「过滤排序照做」的欠账） | `db_rows_of` L4592、`DbFilterPanelRow` L4559、`db_rules` L4666、`db_refresh` L4834、`db_order_groups` L5071、`db_group_label` L5100、`db_fill_row` 规则段 L5194 起、`db_edit_filter` L5775、写 helper L5823–6044、`db_filter_panel` L6045、`db_markdown_table` L6190 |
| `src/app/controller.rs` | 面板模型推送 4 个 helper + 19 个 callback 接线 + 两个场景 + 种子函数 | `db_push_filter` L2873、`db_push_filter_ops` L2909、`db_push_filter_options` L2929、`db_push_group` L2950、dispatch 段 L1523–1746、`dark-database-filter` L3866、`database-filter` L3868、`seed_database_filter` L3606 |
| `ui/Types.slint` | `DbRow.header`（组头条目）；`BlockRow` 的规则头部状态 5 字段（`db-filter-note/-count/-sort-property/-sort-desc/-group-property`）；`DbFilterRow`/`DbFilterOp` 结构；UIState 的面板属性（filter 11 个 + group 5 个）与 19 个 callback | `header` L127、BlockRow 字段 L204–212、`DbFilterRow` L232、`DbFilterOp` L245、UIState 属性 L419–437、callback L623–647 |
| `ui/components/DatabaseFilterPopup.slint`（**新**，530 行） | 过滤面板：一个 popup 四个状态（0 规则 / 1 选列 / 2 选比较符 / 3 选值）——一个会变列表的 popup，而不是 popup 开 popup | 全文件；高度公式 L29–34；规则行 L160 起；选列 L436 起；选比较符 L468 起；选值 L492 起 |
| `ui/components/DatabaseGroupPopup.slint`（**新**，134 行） | 分组 picker：No group + option-bounded 列，行用 `db-group-current` 自标 | 全文件 |
| `ui/components/DatabaseView.slint` | 头部 `Filter`/`Group` 按钮（含激活色与计数 chip）；`db-filter-note` 占据行数文本位置（危险色，可见降级）；列头排序箭头 + 点击循环；行委托的组头臂（`row.header != ""` 画一条 muted 行、无格子无删除） | note L139、filter 按钮 L259、group 按钮 L299、排序 L352/L364、组头 L467、数据行 L489 |
| `ui/AppWindow.slint` | 两个 popup 的注册（import / 实例 / is-open 镜像 / changed 开合），照 DatabaseColumnsPopup 的三件套 | import L20、open 属性 L392–393、changed L474 起、实例 L718/L723 |
| `docs/DECISIONS.md` | ADR-0076（规则编译进语句 + 降级策略）/ ADR-0077（分组是条目投影，组头永远不是行） | 文件末尾（0075 之后） |
| `docs/SPEC.md` | §三十九「操作」标注 D4 交付 + 「性能红线」第二条标注形状已交付、数字留总测试 | 两处 hunk |
| `PLAN.md` | 末尾追加 `## Track 3 · D4 filter / sort / group` | 文件末尾 |
| `docs/REPORT_TRACK3.md` | 本节 | — |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`（**零新依赖**——
JSON 用的是 `core::database_property::json` 那个全仓库唯一的阅读器，SQL 是手拼的，比较符菜单与解析
共用一张 `ops_for` 表）、`[profile.release]`、`src/storage/migrations.rs`（**没有新迁移步**）、
Track 1/2/4 的功能文件。

## 3 · WHERE 生成器：比较符 × 属性类型 → SQL

比较符 10 种（`FILTER_OPS`，序即 int：contains, eq, ne, gt, gte, lt, lte, any-of, is-empty, is-not-empty），
每种 kind 只接受它有意义的子集（`FilterOp::ops_for` 一张表同时管解析接受与面板菜单，所以菜单永远
开不出编译器要拒绝的东西）：

| kind | 比较列（ADR-0070 的决策复用） | 映射 |
|------|------|------|
| title / text / url / email / phone（含 person 折叠后的 text） | `text`（title 走 `COALESCE(p.title, t.text)`） | contains = `INSTR(LOWER(expr), LOWER(?)) > 0`（不用 LIKE，值里的 `%` 是它自己；LOWER 只折 ASCII——全仓库文本搜索同一边界）；eq/ne 文本原样绑定；空值判定 `IS NULL OR = ''` |
| date / created time / last edited time | `text` / `r.created` / `r.edited` | before/after 绑**定宽 ISO 文本**——字节序即时间序（ADR-0062 的形状在这里兑现）；值必须过 `is_stored_date` 两种形状之一 |
| number | `num`（REAL） | eq/ne/gt/gte/lt/lte 绑 `Value::Real`——`2` 排在 `10` 前、`10 > 9` 是数值比较 |
| checkbox | `flag` | `is checked` = `expr = 1`；`is unchecked` = `(expr = 0 OR expr IS NULL)`（没碰过就是没勾）；`is not checked` 是其否定（排除没碰过的行——「不是」断言有一个值） |
| select / status | `text` = 选项 **id** | `is` = `expr = ?`（id）；`is any of` = `expr IN (ids)`；不存在 contains——按标签匹配要读 config JSON，SQL 做不到，菜单也不提供 |
| multi-select / files | `db_value_items` | `has` = 一次 `EXISTS (… WHERE record = r.id AND property = ? AND value = ?)`；`has any of` = `value IN (…)`——都是 PK 前缀上的索引探针（ADR-0062 预言的形状） |
| formula / rollup / relation | —（不存值） | `ops_for` 为空：子句必然被解析层丢弃并计数提示（D6 才有计算值可比） |

另两条语义决定：**`ne` = `NOT (eq 形)`**——三值逻辑让「没有值的行」两边都不匹配（「有一个不是这个
的值」）；**没填完的规则（`FilterValue::Missing`，文档里是 `value: null`）编译成 `1`**——加一条规则
不会在用户说出它之前藏掉任何行，半成品规则重启后还是半成品。

## 4 · 解析失败的降级策略（全部写在 ADR-0076，代码在 `ViewRules::rules`）

| 失败 | 动作 | 可见性 |
|------|------|--------|
| 文档整篇不是 JSON / 不是对象 | `ViewDefinition::parse` 折成空文档（ADR-0064 的既有折法） | 视图打开，无规则 |
| `filter` 树骨架读不开（`and`/`or` 子女不是数组、嵌套 > 8 层） | **整棵树丢弃**，`note` = "This view's filter could not be read and was ignored." | 视图上永久可见（行数文本的位置、危险色）——不静默、不崩、不是一闪而过的 toast |
| 单条子句不可读（属性 id 不存在 / 比较符不适用于该 kind / 值不是该列存的形状） | **丢那一条**（`not` 包着它就一起丢——消失的规则不能变成自己的否定把所有行藏掉），其余树照常编译；`note` 计数（"N filter rule(s) were dropped …"） | 可见（这是 ADR-0064 的删列规则，扩大到子句的其它不可读方式） |
| 排序项编不出来（列没了 / kind 无序）/ 分组编不出来（列没了 / kind 不可分组） | **静默丢弃** | 顺序与分组只改变「怎么看」，不藏行——诚实的失败在第一帧就看得见 |
| 空组 `{"and":[]}`（面板删空规则的正常状态） | 无过滤、无提示 | — |
| 面板遇到嵌套树（它能过滤、不能表示） | **拒绝编辑** + 一次性 notice 说明；表继续按树过滤 | 面板拒绝改写，绝不把用户的树重塑成自己画得出的形状 |

## 5 · group by：分组头怎么避开「全量 realize」

`core::database_view::group_window(counts, window)`（L1167）是答案。三个要点：

1. **条目列表是虚拟的，组列表是小的。** 视图滚动面上是「组头条目 + 该组行」的交错列表，
   `total_entries = Σ(count + 1)`。组列表来自**一次** `GROUP BY`（`group_counts`，`database_store.rs:463`），
   而且只对 checkbox / select / status 这三种 option-bounded kind 开放——一张几行的表，不是全表。
   按 text / date 分组会让组列表和表一样长，正是「分组头不能变成 10 000 行」禁止的事，picker 干脆
   不提供（按 number 分组需要分桶，那是另一个问题，不猜）。
2. **窗口跑在条目数上，映射到每组的行切片。** `window(total_entries, geometry, scroll)` 照旧先开，
   `group_window` 把 `[start, end)` 映射成「窗口内的组头（通常一两个：一个组头就是一个条目高）+
   每个触及窗口的组的 `(skip, len)` 切片」；每片用 `row_query_in_group` 取——组谓词进 `WHERE`、
   视图的 filter/sort 照常编译、`LIMIT len OFFSET skip` 是**组内**偏移。装 10 000 行的一组和不分组的
   整表一样只 realize 31 行；**一个组的成本是一个条目，永远不是一行/组**。走组列表累加条目位置是
   O(组数)，不是 O(条目数)——组头因此不可能被 realize 成全量。
3. **组头是条目不是记录。** `DbRow.header` 非空 = 组头（带计数文案，Rust 拼好），委托画一条 muted
   行、无格子无删除动作；数据行的窗口算术、`db-row-start` 摆放、块高全部与不分组共用。

组头的**顺序**是 Rust 排的（config 的选项表 SQL 看不见）：已知选项按 config 序、config 忘了的 id
按字节序跟在后面（ADR-0069 的折叠用到组头上）、checkbox 未勾在先、「No value」最后。排序的是一小把
**组头**，行还是 SQL 的——红线没有弯。

## 6 · 持久化：落在哪个表哪个列

全部落在 **`db_views.definition`**（ADR-0064 的 JSON 文档，一个视图一行）：

| 键 | 形状 | 写入方 |
|----|------|--------|
| `filter` | 递归树：`{"and":[…]}` / `{"or":[…]}` / `{"not":{…}}` / `{"property":7,"op":"eq","value":…}`；子句级非用 `not` 包裹；未填完的值是 `null` | `ViewDefinition::set_filter`（database_view.rs L1333），面板的每个编辑经 `db_edit_filter` → `db_edit_definition` → `Change::SetDatabaseViewDefinition` |
| `sorts` | `[{"property":7,"descending":false}, …]`，首位最显著 | `set_sorts`（L1344），列头点击循环 |
| `groups` | `[7]`——只读第一项；`[]` = 无分组 | `set_group`（L1366），picker |
| `columns` / `widths` | D3 原样 | 不动 |

写法仍是 ADR-0074 的**文本读改写**：`db_edit_definition`（state.rs）读出整段文档文本 → 编辑函数替换
自己拥有的键 → 整段文本作为 `SetDatabaseViewDefinition` 的 `from`/`to`——一次编辑一个 change、
一步 Ctrl+Z 恢复全部五把键，`db_absorb`（ADR-0075）让 catalog 从 change 批次学会新文档，缓存键里的
definition 文本让下一次投影重读窗口。**没有新表、没有新列、没有新迁移。**

## 7 · UI 接线点

* **面板**：`DatabaseFilterPopup`（AppWindow import L20、实例 L718、`changed db-filter-open` L474、
  is-open 镜像）；一个 popup 四个状态（`db-filter-panel`：0 规则 / 1 选列 / 2 选比较符 / 3 选值），
  比较符与选项两个选择器由 Rust 推送（`db_push_filter_ops` controller.rs:2909 / `db_push_filter_options`
  L2929）。值的文本框 **Enter 提交**（一次规则编辑 = 一个 undo 步；每键提交会把「输入 2026」变成
  五个 undo 步），离散控件（比较符 / ¬ / 删除 / 选项）即时生效。
* **回调**：19 个（`Types.slint` L623–647 全部声明、controller L1523–1746 全部绑定、popup/DatabaseView
  全部使用，三件套齐）：filter 16 个 + `db-sort-cycled` + group 3 个 − 重复计数。每次被接受的编辑：
  写文档（Rust 校验，值形状不对就拒写并让文本留着）→ `db_refill_row`（行数/内容变了）→
  `db_push_filter`（面板还开着，行模型就地换）。
* **列头**：点击循环排序（DatabaseView.slint L382 起 `head-ta.clicked`），排序中的列画 chevron 箭头
  （L364，升/降），无可排序 kind 的列在 Rust 里静默拒绝（点击不撒谎）。
* **分组**：`Group` 按钮（L299）开 `DatabaseGroupPopup`（AppWindow L723），行内勾标当前组；
  `db-group-picked(-1)` = No group。
* **可见降级**：`db-filter-note` 非空时占据行数文本的位置（L139，危险色）——「过滤器没在起作用」是
  每一帧的事实，不是某一刻的事件，所以不用 toast。
* **场景**：`database-filter`（controller.rs L3868）+ `dark-database-filter`（L3866）+
  `seed_database_filter`（L3606，**走面板同一套写路径**：`db_filter_add_clause` → `db_filter_set_op(0,3)`
  （`gt` 是 `FILTER_OPS[3]`）→ `db_filter_set_text(0,"5")`；5 行里 `Points > 5` 留 3 行）。值是字面量，
  明天的 sweep 拍到同一张表。`quire_shot` 的 `needs_db` 已含 `contains("database")`（D3 的），新场景
  自动被覆盖。

## 8 · 迁移号

**没有用新号。** 动手前读 `src/storage/migrations.rs`：工作树 `CURRENT_VERSION = 19`（12–15 D1、
16 Track 2、17 D2、18 本 track D3、19 Track 4 未提交）。本刀的规则全部落进 `db_views.definition`
这份已存在的 JSON 文档（ADR-0064 当初把规则放进文档，就是因为 SQL 不需要在规则上过滤），所以
**零迁移步**；本刀的提交 blob 不含 migrations.rs，提交树里仍是 18（D3 的现状）。

## 9 · 未验证（诚实清单——因为一行 cargo 都没跑）

1. **编译**：本刀约 2 900 行新代码 + 多处重接没有过 `cargo check`。静态自查做了：六个文件的
   括号/圆括号/方括号平衡（带生命周期与字符字面量处理的检查器）全部归零；按名字核对过每个
   `use` 的使用次数（`SortSpec`/`Value` 这两个只剩测试用的 import 已加 `#[cfg(test)]`）；Slint 侧
   19 个 callback 的「声明 / 使用 / 绑定」三件套逐个 grep 核对；`BlockRow`/`DbRow` 字面量补齐新字段
   （state.rs 两处 BlockRow、一处 `DbRow` 工厂 + db_refresh 里两处构造）。但那不是编译器。
2. **测试**：没有跑任何已有测试；铁律禁止新增 `#[test]`，本刀没有写。**已知风险点**（统一测试
   先看这里）：`column_read` 测试 helper 的 `sort.as_slice()` 借用、`RowRequest` 新字面量的临时值
   生存期（已用局部变量绕开 `Option::as_slice` 对临时值的借用）、Slint 的 `if + for` 嵌套作用域、
   `TextInput.accepted(text)` 的签名。
3. **视觉**：`database-filter` / `dark-database-filter` 从未渲染；filter 面板的四个状态、排序箭头、
   组头行全部没有像素证据；按钮重排后 `menu`/`plus`/`slash` 场景不涉及（头部按钮在 database 块内），
   预期只有两个新场景为 new。
4. **分组与排序的交互**：组内行按视图的 sorts 排、组间按 config 序——组合行为只存在于代码与
   注释里，没有运行证据。
5. **性能**：D2 量出的「排序 +8 ms（TEMP B-TREE）」与 D1 量出的「底部 OFFSET 12.9 ms」原样有效，
   本刀没有换游标读；过滤会给语句再加谓词成本，对照数字见下面测试计划（D8 收口）。

## 10 · 测试计划（追加到「留给最终统一测试」清单）

**B. 功能（headless 可证的，接着 D3 的 §10 编号）**

14. 过滤后窗口只 realize 过滤后的行：10 000 行的库、过滤到 3 行，`realized_rows` 的 `realized()`
    == 3 且 `window()` 与 3 一致——建议测试名 `a_filtered_window_realizes_the_filtered_count`（钉住
    「计数先 `COUNT(*)` 后开窗」这条契约）；
15. 每种比较符一句 SQL、语义各有一个对照：contains 的大小写折叠（`INSTR(LOWER,LOWER)`）、
    `any-of` 的 `IN` 绑定 id 数、`ne` 排除空值行而 `is unchecked` 包含它们、number 的 `2 < 10`（数值序）
    与同对值的字节序对照、日期 before/after 依赖定宽（写一个 `2026-9-2` 进去要么被形状检查拒、要么
    顺序可见地坏）——建议测试名 `a_filter_clause_compiles_to_the_comparison_it_names`；
16. 降级三态：整棵读不开 → 行数不变 + `note` 非空；单条子句指向已删列 → 该子句丢弃、其余生效、
    note 计数；空 `{"and":[]}` → 无过滤、无 note——建议测试名
    `an_unreadable_filter_is_dropped_with_a_visible_note` /
    `a_clause_naming_a_deleted_column_is_dropped_and_counted`；
17. 多键排序：两个 `sorts` 项的 `ORDER BY` 文本与实际顺序（首键并列时次键定序、`r.ord, r.id` 收尾）
    ——建议测试名 `a_second_sort_key_orders_the_ties_the_first_leaves`；
18. 分组：组列表 = `GROUP BY` 的归一化键（NULL 与 `''` 同组、flag 0 与缺失同组）、条目数
    Σ(count+1)、跨组滚动只取触及窗口的组、被删选项的值以其 id 成组——建议测试名
    `a_group_header_is_one_entry_and_never_one_row_per_group`；
19. 规则持久化与透传：带嵌套 filter + `sorts` + `groups` + 外来键的文档经一次 D4 编辑后，外来键
    原样保留（ADR-0074 的往返，现在两边都有规则）；undo 一步恢复整段文档——建议测试名
    `a_rule_edit_keeps_the_keys_this_build_does_not_own`；
20. 导出跟随视图：带 filter + sorts 的视图 `Copy page as Markdown`，文件行数 = 过滤后行数、顺序 =
    sorts（ADR-0065 的「过滤排序照做」）——建议测试名 `a_database_exports_the_view_it_is_showing`；
21. 面板拒绝嵌套树：手写 `{"or":[{"and":[…]}]}` 后 `db_filter_editable` 为 false、编辑调用全部返回
    false、表仍按树过滤——建议测试名 `the_panel_refuses_to_reshape_a_tree_it_cannot_draw`。

**C. 视觉（接着 §10 的编号）**

22. sweep 对照 D2 基线：67 个既有场景逐字节相同；`database-filter`、`dark-database-filter` 为 new
    （人工核对：计数 chip "Filter 1"、行数 3、值列仍是全表 5 列）；filter 面板四状态的人工截图
    （打开面板 → 加规则 → 选比较符 → 选值 → 表随 Enter 变化）；
23. 排序箭头与分组头的人工核对：点列头两次（升→降）箭头换向、点第三次消失；分组后组头行是
    muted 满宽行、无删除叉、计数在标签里。

**D. 性能（SPEC 要求的证据，本刀欠的对照）**

24. **「10 000 行的库加一个过滤条件」的耗时 vs 「取回 10 000 行再在内存里过滤」**——SPEC 红线第二
    条要的证据，量法与要量的两个数：
    * **数 A（SQL 侧）**：release 下（D1 的探针写法，`#[ignore]` 打印型），10 000 行 × 5 列的库上，
      构造一个含 text-contains + number-gt 的过滤树，跑 `realized_rows`（= `filtered_count` 的
      `COUNT(*)` + 窗口读）端到端，报 `Instant::elapsed` 与窗口 realize 行数（应 ≈31，不是 10 000）；
      同时打印 `EXPLAIN QUERY PLAN`（预期 `SEARCH r USING INDEX idx_db_records_db_ord` + 每列一个
      索引探针，没有 `SCAN db_values`）。
    * **数 B（对照臂，明知违规才存在）**：同一库 `unwindowed_rows` 取回全部 10 000 行（D1 量过
      ≈56 ms / 1.84 MB 堆），在 Rust 里对同一谓词 `retain`，报耗时与堆字节数。
    * **结论读法**：A 的总耗时（两次查询）对 B 的总耗时（一次大查询 + 内存过滤），以及
      **行对象数与堆字节**（A ≈ 31 行 / ≈6 KB，B = 10 000 行 / ≈1.8 MB）——红线赢在行与内存上，
      时间上若 A 更慢，第一嫌疑是谓词没吃到索引（探针的计划能直接看见），修法是加
      `db_values(property, num)` 类索引或换游标读，**不是把过滤挪回 Rust**。
    * 原始行落 `benchmarks/results/2026-09-22-track3-d4-filter.jsonl`（三次运行），进
      `docs/PERFORMANCE.md` 的收口随 D8。
25. 顺带可量的：一次规则编辑的端到端（写文档 change + force_flush + `filtered_count` + 窗口重读），
    期望 ≈ D2 的单发 cell 写 + 一次 COUNT + D1 的窗口读；数字进同一 jsonl。

## 11 · 给整合者的注意事项

1. **共享文件的提交方式照 D0–D3**：`docs/DECISIONS.md`（排除 Track 4 的 ADR-0052 与其他 track 的
   未提交 ADR；本刀只有末尾 ADR-0076/0077 两段）、`PLAN.md`（只追加 D4 节）、`docs/SPEC.md`
   （「操作」与「性能红线」两处 hunk，排除 Track 2/4 的段落）、`src/storage/mod.rs`（只加
   `database_query` 声明）、`src/app/state.rs` / `src/app/controller.rs` / `ui/Types.slint` /
   `ui/AppWindow.slint`（只加 D4 段落；state.rs 与 controller.rs 里有大量别人的未提交内容，逐段
   提取）。脚本在 `.scratch/track3-d4/stage.py`。提交后这些共享文件在工作树里仍是 modified——
   那是别人的改动，不是脏数据。
2. **本刀提交树带四处对 D3 的补漏**（都是「D3 的 hunk 留在了工作树、提交树缺失」这一类，D1 的
   E0583 / D2 的 E0063 教训的实例；不补则 D4 的提交树单独编译必然是红的）：
   a. HEAD 的 `src/core/command.rs` 引用 `DatabaseDraft`，而 HEAD 的 `src/core/database.rs` 没有它
      （39 行，本就属于 D3）；
   b. HEAD 的 `src/app/state.rs` 两处 `BlockRow` 字面量（页面投影的大字面量与 `block()` 测试工厂）
      缺 D3 的 db 字段（`db_ref`…`db_layout_ok`）；
   c. HEAD 的 `ui/Types.slint` **缺少 `DbCell` / `DbRow` / `DbOption` / `DbColumn` / `DbViewTab`
      五个 struct 定义**（`BlockRow` 引用着它们，D3 的 blob 只带上了 `DbColumnToggle`）；
   d. HEAD 的 `src/app/controller.rs` 把 `table_refocus` / `columns_refocus` 这一对 helper **冻结成
      两份**（E0428 重复定义；工作树当时已并行修成一份，本刀的 blob 带上同一修复）。
   给整合者：**D3 的提交树（93307ca）单独编译是红的**，以上四处随本刀提交树（c0734d2）闭合；统一
   测试若要对照「D3 树 vs D4 树」，请以合并后的树为准。
3. **`RowRequest` 的 API 变化会碰别人的字面量**：`sort: Option<SortSpec>` → `sorts: &[SortSpec]` +
   `filter`。工作树里使用 `RowRequest` 的只有本 track 的文件与 `tests/integration/storage_test.rs`
   的两处 `RowRequest::new(...)`（签名未变，不用改）；若 Track 2/4 在合并前也写了 `RowRequest { … }`
   字面量或 `req.sort = …`，合并时要机械改字段名。
4. **迁移号**不变（见 §8）；**ADR 号** 0076/0077 接在 0075 后，与 Track 4 的 0080+ 不冲突。
5. **`INSTR(LOWER…)` 与 FTS**：contains 是 `INSTR` 上的全表达式，不吃索引——10 000 行的 contains
   过滤是全扫（计划能看见）。SPEC 没有要求 contains 的性能红线，红线要的对照是「过滤在 SQL 侧」
   的形状与行数；若统一测试觉得 contains 慢到影响体验，解法是 FTS5 影子列或 SQLite 的
   `lower()` 函数索引，出 ADR 再做（§三十九 的视图内搜索 D7 也要经过这个决定）。
6. **D3 报告 §8 的三条交叠**（导出调用点、quire_shot needs_db、合并痕迹）继续有效；本刀新增的
   交叠只有一处：`db_markdown_table` 现在构造 `RowRequest` 的结构体字面量（原来用 `new`），如果
   Track 2/4 也改了这个函数，合并时以「带规则的那份」为准。
7. **提交**：`c0734d2`（feat(m14): a filter compiles into the statement, and a group header is
   never a row）——共享文件照旧「只暂存自己那段」（脚本 `.scratch/track3-d4/stage.py`），提交后
   这些文件在工作树里仍是 modified（那是别人的改动），不是脏数据。
