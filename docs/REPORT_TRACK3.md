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
