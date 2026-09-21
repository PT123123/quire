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
