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

# Track 3 — Database（D5：视图族，代码刀）

D0 决策、D1 存储、D2 属性、D3 table、D4 规则之后，D5 把 SPEC §三十九「视图」剩下的六种
**一次做齐**：**board / list / calendar / gallery / timeline / form 六种全部交付**（chart 按
SPEC 的顺序留 D7），每一种都有虚拟化、规则持久化、切换器入口、`apply_scene` 臂与 `dark-` 臂。
本刀遵守任务书铁律：**只写代码，一行 cargo 都没跑**（不 check / 不 build / 不 test / 不 run，
不跑 sweep）——编译、测试、视觉、性能全部留给总测试。本节行号是**工作树**（四条 track 未提交
改动的合集）的行号。

## 1 · 六个视图各做到什么（第一段先回答「做了哪几个、没做哪几个」）

| 视图 | 状态 | 文件与关键位置 | 虚拟化契约（窗口开在什么单位上） |
|------|------|----------------|-----------------------------------|
| **board** | **做了** | `core/database_view.rs` 的 `board_window`/`board_slots`（L~640/L~672）；state 的 Board 分支 L5180–5263；`DatabaseView.slint` 的 board 臂（`is-board`） | 窗口开在**卡片槽位**上：一个槽位是一道横贯所有列的带，板面高 = 各列计数的 max。列=**组列表**（一次 `GROUP BY`，≤ 选项数，全部 realize 成小矩形），每列只取它落到槽位窗口里的那一段（`window_rows_in_group` + 组内 `LIMIT/OFFSET`）。10 000 张卡片的一组 realize 的还是那一窗。**拖拽换组不做**（边界，见 §6） |
| **list** | **做了** | `DatabaseView.slint` 的 list 行臂（`is-list`，行高 44 px = `TableView::LIST_ROW_HEIGHT`）；state 的默认分支（与 table 同路） | 与 table 同一套行窗口（`db-row-start`/`db-row-height`），只是行高 44、一行画「标题 + 首两列预览」，行点击 = 打开 record |
| **calendar** | **做了** | state 的 Calendar 分支 L5266–5395；`core/database_view.rs` 的日历算术（`days_in_month`/`first_weekday_monday0`/`month_cells`/`CALENDAR_PEEK`）；`DatabaseView.slint` 的 calendar 臂 | 格子固定 6×7（表面高是**常数**），窗口开在**天**里：整月一次 `GROUP BY` 取每日计数（≤ 31 键，月界由 `>= 1 号` / `< 下月 1 号` 两个子句编译进语句），每天至多 realize `CALENDAR_PEEK = 3` 条 + 折叠计数（"and N more" 的 N 来自 `GROUP BY`，那 N 条从不变成对象）。**无日期的 record 不显示**（月范围子句 + 三值逻辑天然排除） |
| **gallery** | **做了** | state 的 Gallery 分支 L5398–5441；`DatabaseView.slint` 的 gallery 臂（`is-gallery`，含 delegate 报告的 `per-row`） | 窗口开在**卡片行**上：一次取 `per_row × 行窗` 的切片，delegate 按它报告的 `per_row` 切块摆放。`per_row` 由 delegate 量出（它知道网格宽）后回调给 Rust，**进了缓存键**（`DbWindow::stamp`），所以形状变了就重读。卡片 = 首字母占位（首图见 §6 边界） |
| **timeline** | **做了** | state 的 Timeline 分支 L5444–5570；`core/database_view.rs` 的 `day_number`/`day_number_of`；`database_query.rs` 的 `range_query` + `database_store.rs` 的 `column_bounds`；`DatabaseView.slint` 的 timeline 臂 | 窗口开在**泳道**上（一行一条有日期的 record，行高 36）；轴是**一次 `min`/`max`**（同一 `WHERE`，不取行），「无日期不显示」是 AND 进请求的 `is_not_empty` 子句——不取回来再 `retain`。天的数字在 Rust 从**已绘制的日期格**读出（画出来的日期永远以存储的那天开头），所以一条泳道不花第二次查询。起=止=同一天画点（宽 6 px 起） |
| **form** | **做了** | state 的 Form 分支 L5573–5593 + `db_form_*` 三方法 + `db_form_submit`；`DatabaseView.slint` 的 form 臂 | **不读行**：表面 = 字段表（schema 的可见列 × 40 px）+ 动作行，窗口只用来算计数文案。草稿在会话里（`db_form`），**提交才建行**：每个填了的字段过 `parse_one`，一个 record + N 个 cell 在**一批**里（一次 Ctrl+Z）。**分享链接不做**（brief 明确） |

七个布局里 `chart` 仍 `LayoutSupport::Missing`（D7）：它以名字拒绝，切换器「+」菜单里那行
可见但 inert（"Chart - not in this build yet"）。

## 2 · 本刀改了 / 新增了哪些文件

| 文件 | 为什么 | 关键位置（工作树行号） |
|------|--------|------------------------|
| `src/core/database_view.rs` | D5 的**纯**一半：`LayoutSupport` 七种已交付；`LayoutMetrics`/`layout_metrics`；每布局的表面高公式（`rows_surface_height`/`gallery_surface_height`/`calendar_surface_height`/`form_surface_height`）与常量（LIST 44 / BOARD 76 / GALLERY 132 / TIMELINE 36 / 周 96 / 字段 40）；`board_window`/`board_slots`；日历算术（`days_in_month`/`day_number`/`day_number_of`/`first_weekday_monday0`/`date_key`/`month_key`/`month_of`/`day_of`/`shift_month`/`month_label`/`month_cells`/`CALENDAR_PEEK`）；文档新键 `date`/`end` 的读写 | D5 段 L1560 起（文件末尾）；`layout_metrics` L1590、`board_window` L1720、日历算术 L1740–1900、`date_column`/`set_date`/`end_column` L1905 起 |
| `src/storage/database_query.rs` | `range_query`：`min(expr), max(expr)` over 同一 `FROM`/`WHERE`（timeline 的轴） | L374 起 |
| `src/storage/database_store.rs` | `column_bounds`（执行 range_query，返回存储文本两端）与 `local_month`（日历默认月，用与 record 时间戳**同一个时钟**：`strftime('now','localtime')`） | `column_bounds` L513、`local_month` L540 起 |
| `src/core/command.rs` | `Command::AddDatabaseView { block, view }`（+1 变体、+1 plan 臂；apply `[ViewAdded]` / revert `[ViewDeleted]`），`View` 进 import | 变体 L218、plan 臂 L1455 起 |
| `src/app/state.rs` | 会话态三张表（`db_cal_month`/`db_gallery_per_row`/`db_form`）；`DbWindow` +7 字段（layout/stamp/body/board/cal/tl_start/tl_days）与缓存键扩两维；`db_refresh` 六分支（Layout 决定窗口单位）；`db_fill_row` 填布局载荷（含每布局 geometry）；`db_time_axis`/`db_calendar_month(_set)`/`db_cal_shift`/`db_gallery_set_per_row`/`db_form_*`/`db_add_view`/`db_open_record`；`db_rows_of` 补 `tl_from/tl_to/letter`；`DbRow::header()/header_with()` 两个构造器；`layout_total`/`date_bound`/`month_clauses`/`group_day`/`push_column` 五个自由函数 | `DbWindow` L4513+、`db_refresh` L5025、`db_fill_row` L5845、D5 方法段 L6600 起、辅助函数 L4660 起 |
| `src/app/controller.rs` | 7 个新回调接线（`db-view-added`/`db-open-record`/`db-cal-month`/`db-gallery-shaped`/`db-form-text`/`db-form-submitted`/`db-form-cleared`）；`seed_database_view`（六场景共用，走 `db_add_view` 同一写路径）；6 个场景臂 + 6 个 `dark-` 臂 | dispatch L1752 起、`seed_database_view` L3640、dark 臂 L3985 起、场景臂 L4130 起 |
| `ui/Types.slint` | `DbBoardColumn`/`DbCalendarDay`/`DbFormField` 三个新 struct；`DbRow` +`tl-from`/`tl-to`/`letter`；`BlockRow` +8 字段（`db-body-height`/`db-board-columns`/`db-cal-days`/`db-cal-label`/`db-gallery-per-row`/`db-tl-start`/`db-tl-days`/`db-form`）；7 个新 callback | struct L131/L161 起、BlockRow L204 起、callback L716 起 |
| `ui/components/DatabaseView.slint` | 六个布局臂（行矩形里的 table/list/timeline 分支；board/calendar/gallery/form 四个新臂）；高度改为 Rust 的 `db-body-height`（布局自己的表面）；空态只对行/卡类布局显示 | `is-*` 判定 L47 起、`height` L76、board L780、calendar L880、gallery L1030、form L1130 |
| `ui/components/DatabaseSwitcher.slint` | 「+」点亮：布局菜单（8 行，chart 那行 inert），行点击 → `db-view-added(block, index)` | 全文件（菜单在 `if root.menu-open`） |
| `ui/components/EditorBlock.slint` | `db-height` 改用 `db-header-height + max(db-body-height, 34px)`（布局自己的形状） | L41–53 |
| `docs/DECISIONS.md` | ADR-0078（每布局的窗口单位与日历折叠）/ ADR-0079（视图创建、懒建页触发、chart 拒绝） | 文件末尾 |
| `docs/SPEC.md` | §三十九「视图」段标注 D5 交付 + ADR 号 | 一处（视图段末尾） |
| `PLAN.md` | 末尾追加 `## Track 3 · D5 视图族` | 文件末尾 |
| `docs/REPORT_TRACK3.md` | 本节 | — |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`（**零新依赖**
——日历/时间轴的日子算术是 Hinnant 的 civil-date 两段手写，没有日期库；SQL 仍手拼；JSON 仍
用 D2 那个唯一阅读器）、`[profile.release]`、`src/storage/migrations.rs`（**没有新迁移步**）、
Track 1/2/4 的功能文件。

## 3 · 迁移号（串行接缝）

**没有用新号。** 动手前读 `src/storage/migrations.rs`：工作树 `CURRENT_VERSION = 19`
（12–15 D1、16 Track 2、17 D2、18 D3、19 Track 4 未提交）。本刀的视图族**零迁移**：
board 的列 = D4 的 `groups` 键，时间轴 = 视图文档的新键 `date`/`end`（ADR-0064 的同一份
`db_views.definition` JSON），layout = `db_views.layout` 的**已有行**（`AddDatabaseView` 只是
插一行）。所以 **D5 的提交 blob 不含 migrations.rs**，提交树里仍是 18（D3 的现状）。

## 4 · 顺手闭合的 D4 提交缺口（两条，都是「工作树有、提交树缺」）

D4 的提交（HEAD `009a629`）漏了两处**本 track 自己**的改动，本刀的 blob 把它们带上：

1. **`ui/components/DatabaseView.slint` 整个没进 D4 的提交**（HEAD 的该文件是 D3 版 446 行，
   没有 filter/group 按钮、没有组头行臂）。工作树里那 177 行正是 D4 的 UI 半刀。本刀按
   「工作树版 + D5 段」提交，缺口闭合。
2. **`src/app/state.rs` 的 `record()` 少了 `self.db_absorb(&changes);`**（ADR-0075 的漏斗）。
   HEAD 的 `record()` 只把它交给 persistence，内存 catalog 因此不会学到新建的 library——一个
   刚建的数据库在自己的块上会画成 "(deleted database)" 直到重启。工作树里有这一行，本刀带上。

另外两处工作树里的 D4/合并痕迹（`next_view_id: Cell<u64>,}` 的换行、`column_items, column_boxes,`
的重复）**不**归本刀，是别的 track 的清理，不带上。

## 5 · 各视图特有规则存哪（JSON 键名）

全部落在 **`db_views.definition`**（ADR-0064 的文档，ADR-0074 的文本读改写）：

| 视图 | 键 | 形状 | 写入方 |
|------|----|------|--------|
| board | `groups`（**复用 D4**） | `[7]`（选项有界的那一列） | 同一个 Group picker（`db_group_pick` → `SetDatabaseViewDefinition`） |
| calendar / timeline | `date`（新） | `7` 或 `null` | `ViewDefinition::set_date`；本轮**没有**UI 编辑器（边界见 §6）——默认按 schema 序解析，且**不回写** |
| calendar / timeline | `end`（新，可选） | `7` 或 `null` | `ViewDefinition::set_end`（同边界：本轮无编辑器） |
| list / gallery / form | — | 无自有键（列与顺序用 D3 的 `columns`/`widths`） | — |

时间轴列的解析（`AppState::db_time_axis`）：文档的 `date` 键（合法日期类列）→ schema 第一个
`date` 列 → 第一个派生时间列（created/last edited）→ 无（calendar 画空白月 + nav，timeline
画空态）。**gallery 的 per-row 不进文档**：它是会话态（窗口宽度的事实），delegate 报告、
`DbWindow::stamp` 进缓存键。

## 6 · 已接 / 未接（D5 的诚实边界）

| 项 | 状态 | 说明 |
|----|------|------|
| board 拖拽换组 | **不做** | brief 明说「拖拽换组不做」。换组今天只能改分组列的值（表格视图里改），board 只是它的横向读法 |
| board 卡片的「列内新建」 | 未接 | 卡片列表下方没有 per-column "+ new" 行（表头有全局 New row）。Notion 的 per-column new 是 D7 的模板/预填一起 |
| 新建 select 选项 | 仍未接（D2 的欠账） | `PropertyOptions::to_config()` 没有 `Change` 臂，所以 select/status 列**没有选项编辑器**：board 的分组只能用已有选项的列（场景因此用 checkbox 的 Done 列） |
| `date`/`end` 的 UI 编辑器 | 未接 | 解析按 schema 序默认；「选哪一列做时间轴」的下拉是 D5 之后的东西（文档键已就位、值可手写） |
| gallery 封面图 | 首字母占位 | brief 允许的降级：`files` 属性首图需要逐卡缩略图解码（Track 4 的 `pdf_thumb` / D8 的解码预算），本轮画字母头像 + 标题 + 首列预览 |
| calendar 日格里点一条 record 打开 | 未接 | 日格画标题（elided），点击不开页；打开走 board 卡 / list 行 / gallery 卡三处 + 表格行菜单（D5 新加的 `db-open-record`） |
| timeline 双日期「吸」排序 | 未接 | 只有「一个日期列 + 可选 end 列」。没有 Notion 的 start/end 双属性选择器 |
| 时间轴/日历的空状态 | 已接 | timeline 无任何有日期的行 → 空态文案；calendar 无日期列 → 空白月 + nav（「这个 schema 没有时间轴」是事实，不是错误） |
| 插入菜单四行 database 占位 | **仍 muted** | `Board`/`Gallery`/`List view`/`Calendar`/`Timeline` 五行 id 仍 -1。点亮它们要教插入路径「首视图是 X 的数据库」，而本轮唯一的诚实路径是切换器「+」（ADR-0079）。**没做，明说** |
| 视图内搜索 | 未做（D7） | SPEC 把它排在 D7 |
| 导出 | 未改 | ADR-0065 的「导出当前视图的 GFM 表」在 D4 已跟随 filter/sort；本轮六个布局的导出**仍是那条底层行表**（calendar 的月份/board 的列不改变导出的行集，只改变看法）。边界写在这里 |

## 7 · UI 接线点

* **组件**：`DatabaseSwitcher`（`+` → 布局菜单 → `db-view-added`）、`DatabaseView` 的六个布局
  臂。新增/点亮回调 7 个，全部三件套齐（`Types.slint` 声明 / `.slint` 使用 / `controller.rs`
  绑定，逐个 grep 核对过）。
* **场景**（每个视图一对，共 12 个臂）：`database-board` / `database-list` / `database-calendar`
  / `database-gallery` / `database-timeline` / `database-form` + 各自 `dark-`。种子走**真写
  路径**：`seed_database_table`（D3 的 5 行 4 列）→ `db_add_view`（切换器同一条路）→
  board 用 `db_group_pick(Done)`、calendar 用 `db_calendar_month_set(2026, 9)`（**钉死月份**，
  明天的 sweep 拍同一张月历）。`quire_shot` 的 `needs_db` 已含 `contains("database")`（D3 的），
  新场景自动被覆盖。
* **打开 record**：board 卡（标题区 TouchArea）、list 行（整行）、gallery 卡（整张）三处 →
  `db-open-record` → 有页去页；无页**现在建页**（父 = 数据库所在页、标题 = record 的标题、
  `[PageCreated, RecordPageSet]` 一批 = 一次 Ctrl+Z）→ 导航（controller 的 `open`）。
* **表单**：`db-form-text`（草稿，不写库不重填——正在输入的输入框不能被重建）、
  `db-form-submitted`（一批建行）、`db-form-cleared`（丢草稿）。
* **日历**：`db-cal-month(±1)` → `db_cal_shift`。**画廊**：`db-gallery-shaped(per_row)` →
  `db_gallery_set_per_row`（clamp 1..=8，变了才重读）。

## 8 · 未验证（诚实清单——因为一行 cargo 都没跑）

1. **编译**：本刀约 2 300 行新代码 + 多处重接没有过 `cargo check`。静态自查做了：九个文件的
   括号平衡（剥注释/字符串的检查器；`.slint` 四个文件全平衡，Rust 文件的差值与本刀前一致——
   噪声来自剥串粗粒度与别人未提交的 hunk）；我手写的 Rust 段逐段数过 `{}`（`AddDatabaseView`
   的变体与 plan 臂）；7 个新回调的三件套逐个 grep；`DbRow`/`BlockRow` 的全部字面量构造点
   （`db_rows_of` + `DbRow::header()` + `project_blocks` 大写字面量 + 测试工厂 `block()`）补齐
   新字段；`RowRequest`/`RowWindow`/`Change::RecordPageSet` 的形状对着源码核过。但那不是编译器。
2. **测试**：没跑任何已有测试；铁律禁止新增 `#[test]`，本刀没有写。
3. **视觉**：12 个新场景从未渲染。**已知风险点**（统一测试先看这里）：
   * `for cell[ci] in card.cells : if cell.kind == 7 : Rectangle`（board 卡的 checkbox）——
     「`for` 里套 `if`」在 Slint 1.18 的接受度是解析里最含糊的一处，若不接受，改成
     `for … : Rectangle { if … }` 加宽度 0 的壳；
   * `db-cal-days[week * 7 + day]` 与 gallery 的 `index % per-row`：模型/数学表达式下标；
   * `Math.floor` 返回 int 并赋给 `int` 属性（画廊 `per-row`）；
   * `changed per-row =>` 在首帧布局时触发一次 `db-gallery-shaped`（预期：初始计算值与
     默认 4 不同就触发，触发即重读——没有循环的风险，重读不改宽度）；
   * `drop-shadow-*` 在测试环境（无合成器）的表现。
4. **分组与排序在 board/timeline 上的交互**：board 的组内行按视图 sorts 排、组间按 config 序
   （照 D4）；timeline 的泳道按视图 sorts 排——组合行为只在代码与注释里。
5. **性能**：本刀**没有量任何数字**（D5 欠「切换视图耗时」，见测试计划）。已知的形状：board
   一次刷新 = 1 次 `GROUP BY` + 每个触及窗口的列 1 次切片查询；calendar = 1 次月 `GROUP BY`
   + 每天 1 次 `LIMIT 3`（最多 31 次小查询，**这是本轮最可疑的性能点**，若总测试量出 hitch，
   第一嫌疑是它——修法是把 31 次合成一次「整天范围 + 窗口函数」或一次取回一个月的前 3 行/天，
   不是把过滤挪回 Rust）；gallery = 1 次计数 + 1 次切片；timeline = 1 次计数 + 1 次 min/max +
   1 次窗口读。
6. **`date`/`end` 键与 `columns` 的透传**：ADR-0074 的读改写只在代码里（没有往返测试钉住）。
7. **`db_open_record` 的父页与侧边栏**：建出的页会出现在侧边栏当前页之下（走 workspace 的
   `create`），本刀没有跑过侧边栏的投影；页的 search text 没有写（`create_page` 也不写，只有
   `rename_page` 会写——所以新建的 record 页在搜索索引里只有标题的既有路径，未验证）。
8. **`CURRENT_VERSION`**：工作树 19 / HEAD 18，本刀**没有**动迁移，所以提交树里 18 仍是 D3 的
   现状，合并后 12…19 连续。

## 9 · 测试计划（追加到「留给最终统一测试」清单，编号接 D4 的 25）

**B. 功能（headless 可证的）**

26. board 只 realize 一窗卡片：10 000 行、3 列的分组库，`db_table` 的 board 模型里每列
    `cards.len()` 之和 ≤ `列数 × (可见槽位 + overscan)`，且组的 `count` 之和 = 真实行数——
    建议测试名 `a_board_realizes_one_slot_window_per_column`；
27. calendar 折叠：某天 500 条、`CALENDAR_PEEK = 3`，该日格 `records.len() == 3`、
    `count == 500`；整月计数之和 = 月范围过滤后的 `COUNT(*)`；同一天两种存储形状
    （`2026-09-22` 与 `2026-09-22T10:00`）折到同一格——建议测试名
    `a_calendar_day_folds_five_hundred_records_into_three_and_a_count`；
28. timeline 的「无日期不显示」在 SQL 里：引擎捕到的语句含 `IS NOT NULL AND <> ''`（或
    kind 对应形态），且无日期的行不在结果里；轴 = `min`/`max`；`end < start` 折成点——
    建议测试名 `an_undated_row_never_enters_the_timeline_statement`；
29. gallery 的窗口与报告：`per_row = 4`、viewport 两行，取回的卡片 = `4 × (2 + overscan×2)`；
    `db_gallery_set_per_row(0)` clamp 成 1 且 `per_row = 4` 不变时返回 false（不重读）——建议
    测试名 `a_gallery_window_is_card_rows_times_per_row`；
30. form 提交一批：填两列 + 空一列，一次 `db_form_submit` → 1 个 record + 2 个 cell，**一次
    Ctrl+Z 全没**；一列填了非法值（number 里写 `abc`）→ 提交被拒、`db_notice` 提到列名、**没有
    建任何行**——建议测试名 `a_form_submit_is_one_record_and_one_undo`；
31. `AddDatabaseView` 的往返：新增视图后 `db_views` 多一行、id/ord 正确、**第一个视图的文档
    一字未动**；undo 删掉它、redo 放回来（同 id）——建议测试名
    `a_second_view_does_not_disturb_the_first`；
32. 懒建页：`db_open_record` 一个裸 record → 页存在、标题 = record 标题、父 = 数据库所在页、
    `RecordPageSet` 已写；undo → record 回到裸（无页）；对已有页的 record 再调 → 返回同一页
    （不建第二个）——建议测试名 `opening_a_record_mints_its_page_once`；
33. 文档键透传（ADR-0074/0078）：手写 `{"date":7,"end":8,"columns":[…]}` 后隐藏一列，
    `date`/`end` 原样保留；`date` 指向非日期列 → 解析回退到 schema 第一个日期列且**不写回**
    ——建议测试名 `a_time_axis_naming_a_text_column_falls_back_without_writing`；
34. 布局几何一致：每个布局 `layout_metrics(layout).row_height` == `db_fill_row` 写进
    `db-row-height` 的值，且 `db-body-height` == 该布局表面高公式——建议测试名
    `every_layout_lays_itself_out_at_its_own_unit`（钉住 D3 那个 row-height=0 的 bug 类）。

**C. 视觉（接着 §10 编号）**

35. sweep 对照 D4 基线：既有的 67 个场景 + D3/D4 两个 database 场景的像素**逐字节相同**
    （本刀没动 table 的像素？**会动**：`db-height` 的公式换了实现但等值——table 的 body 仍是
    `total × 32`，所以 `database-table` 与 `database-filter` 应当逐字节相同，**这是要看的信号**：
    若它们动了，第一嫌疑是 body 高度公式不等值）；`database-*` 六个新场景为 new（人工核对
    每张：board 两列 `Done · 3` / `Done · 2` 与卡片、list 两行预览、calendar 2026-09 的格子与
    "and N more"、gallery 4 列卡片与字母、timeline 五天轴上的条与点、form 四字段 + Submit）。
36. 切换器「+」菜单的人工截图：点开 → 8 行、chart 行灰、点 Board → tab 多一个 "Board" 且内容
    换成看板（**这也是「切换视图耗时」要量的那一刻**）。

**D. 性能（D5 欠 SPEC 的第二个数字）**

37. **切换视图耗时**：release 探针（`#[ignore]` 打印型，D1/D2 的写法），10 000 行的库上量
    `db_add_view` + `db_pick_view`（含一次窗口重读）的端到端；再分三种布局各一次（board 的
    `GROUP BY` 成本、calendar 的月 `GROUP BY`、timeline 的 min/max）。原始行落
    `benchmarks/results/2026-09-22-track3-d5-views.jsonl`，进 `docs/PERFORMANCE.md` 随 D8 收口。
38. **calendar 的 31 次日查询**（§8.5 的最可疑点）：量整月刷新（1 次 `GROUP BY` + N 次
    `LIMIT 3`）对一个 10 000 行库的耗时，与「取回整月再在 Rust 里截前 3」的对照臂比；若日
    查询占大头，修法是合查询而不是挪过滤。

## 10 · 给整合者的注意事项

1. **共享文件的提交方式照 D0–D4**：`docs/DECISIONS.md`（只在末尾追加 ADR-0078/0079，排除
   Track 4 前插的 ADR-0080）、`PLAN.md`（只追加 D5 节）、`docs/SPEC.md`（视图段一处的追加）、
   `src/core/command.rs`（+1 变体 +1 plan 臂 + `View` 进 import）、`src/app/state.rs`
   （D5 段 + 两个 D4 缺口）、`src/app/controller.rs`（7 回调 + 12 场景臂 + seed）、
   `ui/Types.slint`（D5 的 struct/字段/callback）、`ui/components/EditorBlock.slint`
   （一行 binding + 其注释）。脚本在 `.scratch/track3-d5/stage.py`。提交后这些共享文件在工作
   树里仍是 modified——那是别人的改动（Track 2 的 mention/backlinks/synced），不是脏数据。
2. **本刀顺带闭合 D4 的两个提交缺口**（§4）：`ui/components/DatabaseView.slint`（D4 整刀没提
   交它）与 `src/app/state.rs` 的 `db_absorb` 漏斗行。所以 D5 的提交树里这两个文件带着 D4 的
   内容——**D4 的提交树（009a629）单独编译/运行是缺这两处的**，统一测试若要对照「D4 树 vs D5
   树」，请以合并后的树为准。
3. **`db_open_record` 会建页**：它是第一个从 UI 触发懒建页的路径（ADR-0063 的 UI 半边）。若
   Track 1 正在改页面生命周期/侧边栏，合并时注意这条路径建出的页是一个普通子页（父 = 打开
   页），没有新的页属性。
4. **迁移号不动**（§3）；**ADR 号** 0078/0079 接在 0077 后，与 Track 4 的 0080+ 不冲突。
5. **`RowRequest` 未变**（本刀没加字段），但 `DbWindow` 的字段和缓存键加了 layout/stamp——
   只影响本 track 的 state.rs。
6. **`CHANGELOG.md` 不要为 D5 写功能行吗？** 六个视图首次可画是**用户可见**的（切换器「+」
   与六个布局）。本刀没碰 CHANGELOG（铁律：不改它）；整合者若要收口，条目草稿：
   > - The view switcher's `+` now adds a view, and six of §三十九's layouts are drawn: Board
   >   (grouped columns of cards), List, Calendar (a month grid with per-day fold counts),
   >   Gallery, Timeline (bars on an aggregate `min`/`max` axis) and Form (a new-record form that
   >   creates one record per submit, one Ctrl+Z). Chart is still refused by name (D7), and the
   >   insert menu's database placeholders beyond "Table view" are still muted.

# Track 3 — Database（D6：计算属性，formula 交付 / rollup·relation 留待）

D0 决策、D1 存储、D2 属性、D3 表格、D4 规则、D5 视图族之后，D6 处理 SPEC §三十九
「需计算：formula / rollup / relation」——**这一刀只交付 formula**，另外两件按 brief
原话留待 Track 2 的引用基础设施（§1 是核查过程与结论）。本刀遵守任务书铁律：**只写代码，
一行 cargo 都没跑**（不 check / 不 build / 不 test / 不 run，不跑 sweep）——编译、测试、
视觉、性能全部留给总测试。本节行号是**工作树**（四条 track 未提交改动的合集）的行号。

## 1 · rollup / relation 为什么没做（先查现状的结论）

任务书要求先查 Track 2 的引用基础设施落地没有，查到的是**没有**：

* `git log --oneline -15` 里没有任何 T2 的引用提交（最近 6 个提交都是本 track 的 D0–D5）；
* 工作树里 `src/core/reference.rs` 与 `src/storage/backlinks.rs` 都是 **`??` 未跟踪**（git
  status 原文）；
* 它们的消费者（`core/mod.rs` 的 `pub mod date;` / `pub mod reference;`、SPEC §四十 写的
  mention `marks` 载荷、迁移 16 的两个索引）也都在**未提交的 hunk** 里，`HEAD` 上没有。

brief 的原话是「relation 用 §四十 的基础设施（Track 2），不自己写一套 id 表」，SPEC 的
排期前提也是「本阶段在 §四十 的引用基础设施之后开始，否则简单表格和 relation 会各造一遍
轮子」。两条路都不能走：把他们的未跟踪文件拖进本刀提交（禁区），或另造一套引用机制
（brief 明禁）。所以这两件**留一刀**（等 Track 2 落地），形态在 **ADR-0084** 里写死——
relation 存 **id 不存标题**、双向关系是**一批 change**（一次 Ctrl+Z）、环检测在**保存时**
做、rollup 是对 relation 目标 record 集合在目标列上的聚合（`sum / count / min / max /
average / none` 六种起步）、配置存列自己的 `config`、值投影时现算（继承 ADR-0083 的契约）。

**D6 只做 formula；报告在下面把 formula 的每一处都写清，rollup/relation 不假装。**

## 2 · 本刀改了 / 新增了哪些文件

| 文件 | 为什么 | 关键位置（工作树行号） |
|------|--------|------------------------|
| `src/core/database_formula.rs`（**新**，约 1 400 行含注释） | 本刀核心：**纯词法 + 递归下降 + 树遍历解释器**（SPEC 原文，零新依赖）+ 依赖集 + 保存时环检测 + config 存取 | 四个预算常量 L119–148、`Val` L157、`display` L177、`val_of` L197、`FUNCTIONS` L319、`Program::parse` L354、`Program::eval(eval_at)` L395/L405、`node` L426、`arith` L570、`compare` L600、`extreme` L638、词法 `lex` L776、解析器 `impl Parser` L979、`build_call` L1221、`would_cycle` L1282、`config_formula` L1316、`config_set_formula` L1335、末尾是「将来测试该验什么」的清单 |
| `src/core/mod.rs` | 声明新模块（**只加 `pub mod database_formula;` 一行**） | L7–9 |
| `src/core/persistence.rs` | **`Change::PropertyConfigSet { id, config }`**（追加在枚举末尾，append-only）——列 `config` 整文档替换，ADR-0074 的读改写纪律用于列 | L201–215 |
| `src/core/command.rs` | **`Command::SetDatabaseFormula { block, property, from, to }`** + plan 臂（block 必须有 db_ref、from==to 不产生 undo 步） | 变体 L248、plan 臂 L1495–1516 |
| `src/storage/database_store.rs` | `set_property_config`（一条 `UPDATE db_properties SET config`）+ `column_values`（导出预加载：一列一次索引扫，按 record 归并的存储值） | `column_values` L350、`set_property_config` L1207 |
| `src/storage/repository.rs` | `apply_one` 的 `PropertyConfigSet` 臂 | L1020 |
| `src/app/state.rs` | 本刀的投影与编辑面：求值适配器 `FormulaSource`、六布局共用的 `db_table_rows`、`db_paint_formulas`、公式方法五件、导出预加载、`db_absorb` 臂、四个会话字段 | `FormulaSource` L4685（`value` L4770 附近）、`db_absorb` 的 `PropertyConfigSet` L5134、`db_table_rows` L7254、`db_paint_formulas` L7271、`db_formula_map` L7339、`db_formula_current` L7364、`db_formula_preview` L7380、`db_formula_accept` L7442、`db_formula_column_add` L7516、计数字段 L185/629、六处 realize 调用点（5413/5551/5615/5711/5832/5862）、导出预加载与整视图现算（`db_markdown_table` 内） |
| `src/app/controller.rs` | 5 个回调接线 + 公式场景种子 + 两个场景臂 | `on_db_formula_opened` L1850、`-text_changed` L1877、`-accepted` L1892、`-closed` L1912、`-added` L1922；`seed_database_formula` L3846；`dark-database-formula` L4151、`database-formula` L4329 |
| `ui/Types.slint` | D6 的 5 个 callback + 8 个 UIState 属性（草稿/预览/错误/列名/三个 id） | L736–758 |
| `ui/AppWindow.slint` | `DatabaseFormulaPopup` 的三件套（import L24 / `changed db-formula-open` L543 + is-open 镜像 L550 / 实例 L786，居中）；Columns popup 加「+ New formula column」一行（L280–305，高度公式同步 +30px） | 见左 |
| `ui/components/DatabaseFormulaPopup.slint`（**新**） | 公式编辑器：多行输入 + 每键现算预览 + 错误行（危险色）+ 一行语法提示 + Enter 保存 / Escape 关闭 | 全文件 |
| `ui/components/DatabaseCell.slint` | 公式格（kind 14）的点击开编辑器：`is-formula` / `clickable` 两个派生属性和 `ta` 的第四个分支 | L45–52、L153 |
| `docs/DECISIONS.md` | **ADR-0082 / ADR-0083 / ADR-0084**（文件末尾；**借号声明见 §8**） | 末尾 |
| `docs/SPEC.md` | §三十九「属性类型」的公式引擎句标注 D6 交付 + 「性能红线」第三条标注 formula 半边 | 两处 hunk |
| `PLAN.md` | 末尾追加 `## Track 3 · D6 计算属性 formula` | 文件末尾 |
| `docs/REPORT_TRACK3.md` | 本节 | — |

**没碰**：`CHANGELOG.md`、`docs/ROADMAP.md`、`docs/PERFORMANCE.md`、`Cargo.toml`（**零新依赖**
——词法/解析/求值全部手写，JSON 仍用 D2 那个唯一阅读器）、`[profile.release]`、
`src/storage/migrations.rs`（**没有新迁移步**）、任何 Track 1/2/4 的功能文件。

## 3 · 引擎：词法 / 解释器在哪、支持什么、边界在哪

**文件**：`src/core/database_formula.rs`（唯一新模块，纯函数 + 一个值类型，无 SQL / 无
Slint / 无时钟 / 无 I/O）。

**支持的语法与函数（全清单，写在一处 `FUNCTIONS` L319）**：

| 类别 | 清单 |
|------|------|
| 字面量 | 数字（`2`、`0.5`，无指数记法）、`"文本"`（仅 `\"` 与 `\\` 两个转义）、`true` / `false` |
| 引用 | `[列名]`——**本行**的列，解析期按列名**精确匹配** schema，未知名字是**语法错误**（保存被拒，不是空格子） |
| 算术 | `+ - * /`、一元 `-`；`+` 在两个 text 上是拼接 |
| 比较 | `==`（`=` 同义）、`!=`、`<`、`<=`、`>`、`>=`（一篇表达式只允许一次比较，`a<b<c` 被拒并提示用 `and`） |
| 逻辑 | `and` / `or` / `not` |
| 函数（7 个） | `if(c,a,b)`、`length(text)`、`round(number)`（半值远离零）、`abs(number)`、`min(…)`、`max(…)`（数值族或文本/日期族，不混）、`text(x)`（**唯一显式转换**） |

**类型系统（四类 + 一空缺）**：`Val`（L157）= `Num` / `Str` / `Flag` / `Date` / `Empty`。
不支持隐式转换，具体写死在这些地方（ADR-0082 同样列了）：`"Total: " + [Points]` 是**类型
错误**；`length([Points])` 是类型错误；`min("a", 1)` 是类型错误；`1/0` 是**错误**（不是
`inf`/`NaN`）；`Empty` **传染**（空操作数 → 空结果），例外只有两处：`if` 短路（只算被选
分支）与 `text(Empty)` = `""`。text 与 date 可以互相比较——两边都是字节，定宽 ISO 的字节序
就是时间序（ADR-0062 的存储形状，不是转换）。select/status 与 multi-select/files 的值**读作
`Empty`**（存的是选项 id / 附件 id，id 进算术比空格子更坏）——这是本刀的诚实边界。

**有限求值（SPEC 原文「表达式必须有限求值」）**：四个常量（`database_formula.rs` L119–148）

* `FORMULA_MAX_TOKENS = 2_048`（词法期，超了直接拒）；
* `FORMULA_MAX_DEPTH = 32`（解析嵌套 + **运行期属性链深度**：公式引用公式列会递归求值，
  旧文档里的环在这一层折成 `Error`，不挂）；
* `FORMULA_MAX_STEPS = 10_000`（求值节点预算，整个表达式共享）；
* `FORMULA_RESULT_MAX = 65_536`（结果文本上限，拼接的膨胀在这一层被拒）。

文法是**无循环、无自定义函数**的（上面那张表就是全部），加上「无时钟无 I/O」（`today()`
刻意缺席——读时钟的公式对同一份文档每帧画不同的值），所以这四个数就是全部工作量上界。

**环检测在保存时做**（SPEC 原文）：`would_cycle`（L1282）在 `db_formula_accept`
（state.rs L7442）里、**记录 change 之前**跑：公式可以引用另一个公式列（组合），自指的链
当场被拒并在 notice 里说明；渲染期只有上面的 depth 上限兜「从别的门进来的」文档。

**表达式存哪、值为什么不入库**：表达式存 `db_properties.config` 的 `"formula"` 键
（ADR-0061 的一列一文档；`config_formula` L1316 读、`config_set_formula` L1335 读改写，
**不拥有的键原样保留**、空表达式**删键**），写入是新 change `PropertyConfigSet`（整文档
替换）与新命令 `SetDatabaseFormula`——一次编辑一个 change、一步 Ctrl+Z。**值不入库**：
SPEC 的「不存值，投影时现算」（ADR-0062 记下、ADR-0039 的纪律），所以任何写路径都不写
公式格——「存的是表达式，值是现算的」这句话在代码里是两个不同的地方。

## 4 · 增量重算：机制、边界、将来的测试量哪两个数（ADR-0083）

**机制（三段，都是形状不是纪律）**：

1. **求值只发生在投影窗口**：六个布局分支的 realize 统一走 `db_table_rows`
   （state.rs L7254，`table_rows` + `db_paint_formulas` L7271），所以求值集 = **被 realize
   的行 × 可见公式列**（表格/列表是 31 行那一窗，board 是槽位切片的卡、calendar 是每天
   ≤3 条、timeline 是泳道窗口）——**没有任何路径遍历全表求值**。
2. **依赖是本行的，且按构造成立**：`FormulaSource`（L4685）是**为一个 record 造的**，
   引擎的取格回调（`Program::eval` 的 `cell`）**没有 record 参数**——公式在 API 上就引用
   不到别的行（跨行是 rollup/relation 的事，ADR-0084）。依赖集 `Program::deps()`
   （直接引用）+ 调用方展开闭包（保存时环检测与导出预加载都走它）。于是「改格
   (r,q)」后**值可能变化的格 ⊆ {r} × {P | q ∈ deps*(P)}**，窗口里其余格的重算结果逐位不变
   （纯函数、输入只有本行的格）。
3. **计数器说话**：`db_formula_evals`（state.rs L185）每次投影求值 +1，**没有任何逻辑读
   它**——它是 ADR-0083 契约的单位。

**为什么没有跨刷新的值缓存**（写进 ADR-0083 的代价）：跳过「重算结果不变」的那些格需要
缓存 + 失效，而失效必须覆盖每条写路径（单元格、undo、redo、表单提交、LAN 批量替换）；
漏一条就画**陈旧值**，比确定性的微秒级重复求值更糟。窗口重算与其它列的 repaint 同价，
所以就按窗口算；将来 D8 若要省这一份，干净的位置是 `record()` 漏斗上的 dirty 集。

**量法（写进下面的测试计划，第 39 条）**：两个数——

* **数 A**：`db_formula_evals` 在「改一个单元格」前后的差值。预期 = 窗口行数 × 可见公式列
  数（≤ 39 × F），**在同一棵树上的 10 000 行库与 5 行库上相同**（与 `COUNT(*)` 无关）。
* **数 B**：该次编辑前后窗口里**绘制文本发生变化的 (row, property) 集合**。预期
  **⊆ {被改的行} × {依赖闭包含该列的公式列}**——其余格逐字节不变。

两个数一起才是红线：**窗口有界的工作 + 依赖精确的效果**，且两者都不随总行数增长。

**导出边界**（同 ADR-0083）：`db_markdown_table` 渲染**整个视图**（ADR-0065 的边界），所以
公式对整视图按行现算——这是**显式产物的固有成本**，不属「输入触发的重算」红线；它的依赖
值经 `column_values`（database_store.rs L350）**每列一次索引扫**（按 record 归并），
而不是每行一次点读。

## 5 · UI 接线点

* **单元格**：公式格（kind 14）保持只读展示（`editable=false`，`TableCellView::editable`
  的既有规则不变），但 `ta` 的 `enabled` 与点击分支加了一种（DatabaseCell.slint L45–52、
  L153）：点击 → `db-formula-opened(block, record, property)`，record 是**样例行**。
* **弹窗三件套**：`Types.slint` 声明（L736–758）→ `AppWindow.slint` 的
  `changed db-formula-open` + is-open 镜像 + 实例（L543/L550/L786，居中：它编辑的是**列**
  这个 schema 事实，不是格子，格子会滚走）→ `controller.rs` 五个回调（L1850–1935）。
* **实时预览**：每键 `db-formula-text-changed` → `db_formula_preview(record, property, text)`
  （state.rs L7380）解析 + 在**样例行**上求值，返回 `(preview, error)`；错误行显示解析或
  求值的**一句消息**，预览显示 `= 值`（空值显示 `= empty`）。样例行 `-1` = 空行
  （表里还没有行时预览的就是「这公式在空行上是什么」）。
* **保存**：Enter（或 Save 按钮）→ `db_formula_accept`（L7442）：校验（是公式列 / 解析与
  列名 / 不产生环 / 空草稿 = 清空）→ `SetDatabaseFormula` 一个 change → `db_refresh`；
  被拒时 notice 说明、旧表达式不动。一次保存一步 Ctrl+Z（undo 恢复的是**整段 config**）。
* **新建列**：Columns popup 底部一行「+ New formula column」→ `db-formula-added` →
  `db_formula_column_add`（L7516，名字 `Formula` / `Formula 2`… 直到唯一，因为
  `UNIQUE (db, name)`）→ 直接打开编辑器。**这是本刀的一处范围增加**（brief 只要求编辑器）：
  没有它，编辑器只能靠场景种出来的列到达，整条路径对用户不可达。kind picker 仍是 D7 的。

## 6 · 场景

`database-formula`（controller.rs L4329）+ `dark-database-formula`（L4151）。种子
`seed_database_formula`（L3846）走**真写路径**：`seed_database_table`（D3 的 5 行 4 列，
字面量值）→ `db_add_column` 两个公式列 → `db_formula_accept` 写表达式（**含保存时的全部
检查**，场景不可能种出编辑器会拒绝的公式）→ `db_refill_row`。两个表达式覆盖两种最常见形状：

* `Double` = `[Points] * 2` → 62 / 20 / 14 / 78 / 6（算术 + 数字列）；
* `Label` = `if([Done], "done", "open")` → done / open / done / open / done（布尔列 + `if` +
  字符串字面量）。

`quire_shot` 的 `needs_db` 已含 `contains("database")`（D3 的），新场景自动被覆盖——**没有
改 quire_shot.rs**。编辑器本身不在场景里（场景钉的是带计算列的表）。

## 7 · 迁移号（串行接缝）

**没有用新号。** 动手前读 `src/storage/migrations.rs`：工作树 `CURRENT_VERSION = 19`
（12–15 D1、16 Track 2、17 D2、18 D3、19 Track 4 未提交）。本刀**零迁移**：表达式落进
`db_properties.config` 这个**已经存在的列**（ADR-0061 从第一天就有它），`Change` 只加变体、
表结构一字不动。所以 **D6 的提交 blob 不含 migrations.rs**，提交树里仍是 18（D3 的现状）。

## 8 · 决策号：**号段用尽，借了 0082–0084**（请整合者确认）

本 track 的号段是 **0060…0079**（ADR-0060 到 ADR-0079 已在本 track 各刀用尽）。工作树
`docs/DECISIONS.md` 里 **ADR-0080 / ADR-0081 是 Track 4 的**（`hayro` PDF 缩略图与
`bookmark`，未提交、插在文件中部），所以本刀**借 0082 / 0083 / 0084**：

* **ADR-0082** — formula 的表达式存 config、值现算、引擎是纯词法 + 手写解释器（类型 /
  函数 / 预算 / 无隐式转换的清单与环检测的落点）；
* **ADR-0083** — 增量重算的契约（窗口为求值单位、本行依赖、计数器说法、导出的边界、
  为什么没有值缓存）；
* **ADR-0084** — rollup / relation 留待 §四十（Track 2 未落地的证据、形态写死）。

**给整合者**：这是越界借号（brief 允许但要明说），若整合时要重编号（例如把 Track 4 与本
track 的第二批放在同一段），请以**内容**为准搬迁；ADR 之间的互相引用在正文里都是按内容
写的（「ADR-0082」「ADR-0083」这两个词不构成需要改的交叉引用之外的东西）。

## 9 · 未验证（诚实清单——因为一行 cargo 都没跑）

1. **编译**：本刀约 2 000 行新代码（引擎 1 400 + 接线 600）没有过 `cargo check`。静态自查
   做了：**括号平衡**（写了一个处理 `'a` lifetime 与 `b'"'` 字面量的词法级检查器，四个
   独立文件与八个共享文件的 delta 全部归零——第一版检查器把 `Formatter<'_>` 当未闭合字符
   字面量，报过假阳性，已在报告里记下方法）、五个新回调的三件套 grep 核对、`Change` 的
   全部 match 点（`document.rs` 有 `_` 兜底臂所以不用改；`repository.rs::apply_one` 是穷尽
   match，已加臂）、`PropertyKind::ALL` 的 kind 号（公式 = 14，与 Types.slint 的 legend 一致）
   核过。但那不是编译器。
2. **测试**：没有跑任何已有测试；铁律禁止新增 `#[test]`，本刀没有写（未来测试清单在 §11 与
   `database_formula.rs` 文件尾）。
3. **视觉**：`database-formula` / `dark-database-formula` 从未渲染；公式编辑器的四个状态
   （空草稿 / 有值预览 / 解析错误 / 求值错误）没有像素证据；`DataabaseCell` 的 `clickable`
   改动对既有场景应当零像素差（没有公式列的旧场景照旧 inert）。
4. **求值的每一行**：引擎的每个函数、每种类型错误、`Empty` 传染、四个预算、`would_cycle`、
   `config_set_formula` 的键保留——**全部没有运行证据**，只有代码与注释。
5. **增量重算的两个数**：`db_formula_evals` 计数器就位，但**没有量过**（D8 收口）。
6. **滚动的求值成本**：每帧滚动若窗口移动会重读并重算一窗公式（31 × F 次点读依赖格）——
   量级预期与 D1 的窗口读同阶（+依赖点读 µs 级），但没有探针证明；如果总测试看到滚动的
   帧成本抬升，第一嫌疑是「每个公式格逐列点读」，修法是让 dep 列的读随窗口查询一起来
   （D4 的 `hidden_join` 已经有这个机制），不是把公式挪回 Rust 预计算。
7. **`date` 的字节比较**依赖 ADR-0062 的定宽（与 D2/D4 同一条依赖，已有既有测试钉住形状）。

## 10 · 测试计划（追加到「留给最终统一测试」清单，编号接 D5 的 38）

**B. 功能（headless 可证的）**

39. **增量重算的两个数**（ADR-0083 的契约，本刀最核心的一条）：10 000 行的库、1 个 number
    列 + 1 个公式列 `[Points] * 2`，并排一个 5 行的同构库作对照。改一个单元格后：
    (a) `db_formula_evals` 的增量在两个库上**相同**（= 窗口行数 × 可见公式列数，两库都不是
    `COUNT(*) × F`）；(b) 用两次投影的 `DbRow` 比较，绘制文本变化的格**只**出现在被改行上。
    建议测试名 `editing_one_cell_recomputes_the_window_and_only_its_row_changes`；
40. **公式求值的逐函数表**：`+ - * /`、一元负、每个比较符、`and/or/not`、`if` 的三臂、
    `length/round/abs/min/max/text`——每个一条断言（含 `round(2.5) == 3`、`round(-2.5) == -3`、
    `min` 的数值族与文本族各一）——建议测试名 `every_function_computes_the_value_it_names`；
41. **拒绝的清单**（与 40 对照）：`"Total: " + [Points]`、`length([Points])`、`min("a",1)`、
    `1/0`、`a < b < c`、未知函数、未知列名——各自是 `Err`（语法/求值两类分对），且**保存被
    拒**（`db_formula_accept` 返回 false 且 config 未变）——建议测试名
    `a_formula_that_cannot_compute_is_refused_at_save_time`；
42. **Empty 传染**：空 number 格上 `[Points] * 2` 是空、`[Points] + 1` 是空、空串拼接
    `"x" + [Empty]` 是空——例外两条：`if([Empty], "a", "b")` 是空（条件为空）、
    `text([Empty])` 是 `""`——建议测试名 `an_empty_operand_makes_an_empty_value`；
43. **环检测保存时**：`[Self]`、`A→B→A`（两个公式列）都是**保存被拒**且 notice 非空、
    config 未变；`A→B→C`（无环链）保存成功且 C 的值经 B 的表达式算出来（公式引用公式）；
    手写一个环进 `db_properties.config`（绕过保存门）后求值画 `Error` 且**不挂**（深度上限）
    ——建议测试名 `a_formula_cycle_is_refused_when_it_is_saved_and_never_hangs_the_frame`；
44. **`config` 的读改写**：带 `{"options":[…]}`（或任何外来键）的列写公式后外来键原样保留、
    顺序不变；空表达式**删掉** `"formula"` 键；`config_formula` 与 `config_set_formula`
    往返——建议测试名 `a_formula_edit_keeps_the_keys_this_build_does_not_own`；
45. **四个预算**：2 049 token 的表达式被拒（语法）、33 层嵌套被拒、宽表达式撞
    `FORMULA_MAX_STEPS`（用 `1+1+…` 造宽）、超过 `FORMULA_RESULT_MAX` 的拼接被拒为**求值**
    错误——建议测试名 `a_formula_too_large_to_evaluate_is_an_error_not_a_hang`；
46. **一条真实写入的往返**：`db_formula_accept` 一个表达式 → 重开库（`load`）后
    `config_formula` 读出同一条；undo → 旧表达式回来（`PropertyConfigSet` 的 revert）；
    redo → 新的回来——建议测试名 `a_formula_survives_a_reopen_and_an_undo`；
47. **公式列不可排序/过滤**：`sort_column(Formula) == None` 且 `ops_for(Formula)` 为空
    （D2/D4 已有，本刀把它变成「有理由的拒绝」——测试同时断言**没有** SQL 侧公式求值路径：
    `RowRequest` 的列集里公式列不产生 join）——建议测试名
    `a_formula_column_is_never_compiled_into_sql`；
48. **导出跟随**：带一个公式列的页导出 Markdown，文件里公式列的值 = 计算值（不是空白），
    行数 = 视图行数——建议测试名 `a_formula_exports_the_value_it_shows`；
49. **只读展示**：公式格的 `TableCellView::editable == false`，且 `checked` 不因公式画出
    `Yes` 而变 true（flag 只属于 checkbox 列）——建议测试名
    `a_formula_cell_is_read_only_and_never_a_checkbox`。

**C. 视觉（接着 §10 的编号）**

50. sweep 对照 D5 基线：既有的 67 + D3/D4/D5 的 database 场景像素**逐字节相同**（本刀对
    没有公式列的库零像素改动——`DatabaseCell` 的 `clickable` 只是让 kind 14 可点）；
    `database-formula` / `dark-database-formula` 为 new，人工核对：`Double` 列显示
    62/20/14/78/6、`Label` 列显示 done/open/done/open/done，列宽与表头正常；
51. 公式编辑器的人工截图：点一个公式格 → 弹窗居中、输入框预填表达式、preview 显示
    `= 62`；把 `*` 删掉一个 → 错误行出现一句话；按 Enter → 表里的值变成新表达式的值；
    Columns popup 底部一行「+ New formula column」→ 新列出现在表尾且编辑器打开。

**D. 性能（D6 欠的这一条，随 D8 收口）**

52. **改一个单元格的端到端 + 增量重算行数**（SPEC §三十九 要的「打开公式编辑器的耗时」与
    红线第三条一起）：release 探针（D1/D2 的 `#[ignore]` 打印型写法），10 000 行 ×
    (number + 公式) 的库上量：
    (a) 一次 `db_set_cell_text`（flush + change + 窗口重读 + 公式重算）的端到端；
    (b) `db_formula_evals` 的增量与 `realized_rows`/`total` 并列打印（证明前者 ≈ 窗口 ×
    公式列，后者 = 10 000）；
    (c) 打开公式编辑器到预览出现（`db_formula_current` + 一次 `db_formula_preview`）的耗时；
    (d) 滚动一屏的重算成本（一次 `db_refresh` 的 evals 与耗时）。
    原始行落 `benchmarks/results/2026-09-22-track3-d6-formula.jsonl`，进
    `docs/PERFORMANCE.md` 随 D8。

## 11 · 给整合者的注意事项

1. **共享文件的提交方式照 D0–D5**：`docs/DECISIONS.md`（只在末尾追加 ADR-0082…0084，排除
   Track 4 前插的 ADR-0080/0081）、`PLAN.md`（只追加 D6 节）、`docs/SPEC.md`（公式引擎句与
   性能红线第三条两处）、`src/core/mod.rs`（只加 `pub mod database_formula;`，排除 Track 2
   的 `date`/`reference`）、`src/core/persistence.rs`（枚举末尾 +1 变体）、`src/core/command.rs`
   （+1 变体 +1 plan 臂）、`src/storage/repository.rs`（+1 臂）、`src/app/state.rs` /
   `src/app/controller.rs`（D6 段落）、`ui/Types.slint` / `ui/AppWindow.slint`（D6 段落）。
   **整文件提交**（本 track 独占、工作树无别人的改动）：`src/core/database_formula.rs`（新）、
   `src/storage/database_store.rs`、`ui/components/DatabaseFormulaPopup.slint`（新）、
   `ui/components/DatabaseCell.slint`、`docs/REPORT_TRACK3.md`。脚本在
   `.scratch/track3-d6/stage.py`。提交后共享文件在工作树里仍是 modified（那是别人的改动）。
2. **`PropertyConfigSet` 是给后来者留的门**：D2 的欠账「选项列表没有 Change 臂」可以用它
   （整文档替换 + 调用方读改写），option editor / number format / rollup 目标都走同一条；
   `db_absorb` 的臂已经在了。
3. **`RowRequest` 未变**（本刀没有加字段），但**投影的 realize 入口**从 `table_rows` 换成了
   `state::db_table_rows`——若别的 track 也在 `db_refresh` 里加 realize 点，请用同一个入口
   （否则那条路径的公式列会是空白）。
4. **ADR 借号 0082–0084**（§8 明说）。
5. **迁移号不动**；`CURRENT_VERSION` 工作树 19 / 提交树 18。
6. **CHANGELOG**：本刀有**用户可见**的一半——公式列能算值、能打开编辑器编辑（通过 Columns
   popup 新建）。本刀没碰 CHANGELOG（铁律），整合者若要收口，条目草稿：
   > - A database column can be a **formula**: `[Column] * 2`, `if([Done], "done", "open")`, and
   >   seven functions over numbers, text, booleans and dates (no scripting runtime — a small
   >   hand-written interpreter). Its expression lives in the column, its values are computed on
   >   the way out of SQL, and only the rows on screen are ever computed.
   > - Rollup and relation columns are **not in this build**: they wait for the reference layer
   >   (ADR-0084), and their cells stay blank rather than pretending.
7. **`quire_shot` 不用改**：`needs_db` 的 `contains("database")` 已覆盖新场景。
8. **静态自查的方法学**（可能对别的 track 有用）：括号平衡检查器必须处理 Rust 的 lifetime
   （`Formatter<'_>`、`&'static str`）与含引号的字符字面量（`b'"'`），否则会把代码吞掉报
   假阳性；本刀的检查器在 `.scratch/track3-d6/balance.py`。
