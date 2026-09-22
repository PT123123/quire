# Agent brief · Track 3 — Database（SPEC §三十九 / M14）

你是 **Quire** 的 Track 3 agent。Quire 是一个本地优先、Rust + Slint 1.18 的
Notion 式笔记应用：单进程、无 WebView、无 JS/WASM 运行时、没有网络客户端。
本文件是你的全部范围定义。

**这条 track 是本计划里最大的一项** —— SPEC 自己写着「这是 Quire 与 Notion
差距最大的一层」。本文件把它切成 D0–D8 九个阶段，并明确标出**一条可以独立
交付的竖切**（D0–D5）与**第二批**（D6–D8）。做不完是正常的，**做到哪报哪**，
但要保证每次提交之后仓库是绿的、可用的。

---

## 1 · 开工前必读（按顺序，别跳）

| 文档 | 你要拿走的东西 |
|------|----------------|
| `docs/ARCHITECTURE.md` | 分层与六条硬规则（UI 不碰 SQL/IO、核心不知道颜色、状态全在 `UIState`、语义操作一律走 Command） |
| `docs/SPEC.md` **§三十九**（`三十九、第二十五阶段：Database`，行 ~2480 起） | 你的需求原文，六节：对象模型 / 属性类型 / 视图 / 操作 / 性能红线 / 排期前提 |
| `docs/SPEC.md` **§三十七 硬性约束**（行 ~2421） | 「凡『行是动态的』块要多改两处」+ 新块种类的六个接点。database 块如果走块路线，这六个接点一个不能少 |
| `docs/SPEC.md` **§十二**（虚拟化前提）、**§二十二 / §二十三**（性能预算与红线）、**§三十二**（Agent 的性能规则）、**§三十三**（特别禁止） | 你的性能红线原文 |
| `docs/DECISIONS.md` **ADR-0031** | table **是网格不是数据库** —— 这是你与它的边界，写得很清楚（「schema、过滤、排序属于 §三十九」）。你必须与它同构，不要顺手把简单表格改成数据库 |
| `docs/DECISIONS.md` **ADR-0028**（折叠子树零 realized row）、**ADR-0032**（分栏：Slint 没有递归组件，容器在自己那一条 delegate 里画）、**ADR-0026**（引用存 id 的契约）、**ADR-0039**（派生数据不入库）、**ADR-0042**（`visible: false` 拦不住 binding） | 你会反复撞到的先例 |
| `docs/EDITOR_ARCHITECTURE.md` | 命令系统、`project_blocks`、单个 `TextEdit` 的边界 |
| `docs/UI_ARCHITECTURE.md` §State flow / §Overlay layering / §Slint geometry traps / §Slint language traps / §Adding a component | 加组件与踩坑清单 |
| `docs/PERFORMANCE.md` §Method | 「量什么才算通过」 |
| `PLAN.md` 的 `## M10 批次 B · slice 4 — table 网格` 与 `slice 5 — columns 分栏` 两节 | **最重要的先例**：一个「行是动态的」块在这个项目里被拆成什么样、报告怎么写 |
| `docs/ROADMAP.md` §Definition of done | 五条，缺一条不算完 |

---

## 2 · 并行纪律（同一仓库同时有四条 track，必须照做）

### 2.1 分支与提交

- 一条 track 一个分支：`track/3-database`，从 `master` 起。
- **一个 phase（或一个可独立验收的子切片）一个提交**，只 `git add` 你这一刀明确改到的文件，**永不** `git add -A` / `git add .`。
- 永不做 `git checkout` / `switch` / `restore` / `reset` / `stash` / `rebase`。
- 只在 `cargo check --all-targets` 干净时才提交。不 push（除非用户明确说了 push）。

### 2.2 四条 track 的文件归属

| Track | 主题 | 主要 territory |
|-------|------|---------------|
| 1 | 页面外观 | `Page` 结构体、`pages` 列、封面/锁/模板/版本历史 |
| 2 | 引用与反向链接 | `MarkKind`、mention / date / 反向链接索引、页面底部面板 |
| **3（你）** | **Database** | **新模块为主**：`src/core/database*.rs`、`src/storage/database*.rs`、`src/services/database*.rs`（新）、`ui/components/Database*.slint`（新）、你自己的迁移步 |
| 4 | 欠账清零与 RC 收官 | `src/platform/**`、`install/**`、`benchmarks/**`、`Cargo.toml` 依赖、弹窗滚动 |

你是四条里与别人**重叠最少**的一条（你以新文件为主）。但仍然要守下面的接缝。

### 2.3 共享接缝（四个热文件）

1. `src/core/types.rs` — **只追加**。新枚举变体加在末尾，**永不重编号、永不重排**；不改既有变体的 `as_str`。给 `Block` / `Page` 加字段放在结构体末尾，并写清空值语义。
2. `src/storage/migrations.rs` — **串行接缝，先到先得**。runner 按数组顺序应用且不允许跳号：你的新步 `version` = **提交那一刻**的 `CURRENT_VERSION + 1`；提交前重新读一次这个文件，号被占了就改号并同步改测试名。迁移体走 `add_page_columns()` 那种「缺哪列补哪列」的范式（v10/v11 已立好），并配「vN-1 库升上来读回原值」的断言。
   - **数据库这一层注定要多步迁移**（`databases` / 属性 / 记录 / 值 / 视图定义），所以：**把「一次迁移 = 一个可独立回滚的语义单位」当纪律**，不要把一个 phase 的全部表塞进一个巨型 `sql` 块里 —— 迁移是这个项目唯一不可逆的东西。
3. `ui/Types.slint`（`UIState`）与 `src/app/controller.rs` — 只加你自己的 property / callback / match 分支，不重排既有行；`apply_scene` 只追加你的场景。
4. `ui/AppWindow.slint` — 新组件要在这里注册、新弹窗要在这里挂根。只加你的部分。

### 2.4 ADR 编号配额

ADR-0001…0044 已占用。**你的号段是 ADR-0060…0079**（你需要的 ADR 最多），按顺序取。四条 track 同时追加，所以 **ADR 直接追加在文件末尾**，不要插到中间。

### 2.5 只有整合者能改的文件

`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` 由整合者统一收口，**你不直接改**。把 PLAN 段落草稿、CHANGELOG 条目草稿、ADR 全文、未验证边界写进 `docs/REPORT_TRACK3.md`（新文件，只有你写）。
> 例外：`PLAN.md` 允许你**在末尾追加** `## Track 3 · D<n> <phase 名>` 小节。

### 2.6 环境坑（本机实测，别踩第二遍）

- 本机把 **bash 的 `rm -rf`** 重定向到回收站：批量删除（阈值 50）返回非 0，stderr 只有一行 `SAFE_DELETE_BULK_CONFIRM_REQUIRED`，文件原样保留；**返回非 0 会让 `rm -rf x && next` 整条短路**，`next` 静默不执行。你的 bench 会反复重建数据库 —— 清目录走脚本自带路径，不要把清理与后续步骤用 `&&` 串起来。
- 重定向到文件的日志，命令没执行时**既不创建也不截断**。用带时间戳的独立文件名，或先看退出码。
- 同一文件**不要在同一条消息里发多个 `Edit`**（互相覆盖）。
- 改完必须**真跑一遍**，不要只信编辑返回的 success。
- 测试建目录一律用 `quire::testing::ScratchDir`；跑数据只写 `.scratch/`，**绝不碰 `%APPDATA%\Quire` 或仓库旁的 `appdata/`**；GUI 验证用 `--db .scratch/<dir>/quire.db --auto-exit <secs>`。

---

## 3 · 任务清单（D0–D8）

### 「先证明通道存在」——这是本 track 的第一条纪律

在写任何 UI 之前，先证明**「10 000 行的库不全量 realize」**这条通道在你选的形状下真的成立（照 §十二 的虚拟化前提与 ADR-0028/0031 的做法：投影先算可见窗口，再取行）。
**这一步做完要有一个数字**（10 000 行的库打开/滚动时进程增长多少），做不到就说明形状选错了，早返工比晚返工便宜十倍。这一条是 SPEC §三十九「性能红线」的第一句。

### D0 · 决策与探针（出 ADR，不写功能）
至少要回答下面这些，每个一个 ADR，**先写下来再动手**：

1. **database 是什么实体**：一个页面？一个块（inline database）？两者都要？Notion 两者都有，本项目最省的形状自己定 —— 注意「+」插入菜单里已经有 `Table view / Board / Gallery / List / Calendar / Timeline` 这些 **muted 的 "later" 占位行，目前不可选**（见 `PLAN.md` 的 `## Track A round 6` 与 CHANGELOG），你的形状决定它们将来怎么被点亮。
2. **schema 存哪**：列定义走 JSON 一列，还是行表（`db_properties`）？判据是「filter / sort 必须在 SQL 侧完成」（见下），JSON 会让它很别扭 —— 但 JSON 省迁移。写清代价。
3. **值怎么存**：一张 EAV 表（record, property, value）还是按类型分列？同样是「SQL 侧过滤排序」在施压。
4. **record 与 page 的可逆关系**（SPEC 原文「record 可以同时是一个 page，这是 Notion 的核心而不是装饰」；「删 record 与删页面的行为都要有明确定义，且都进 undo」）—— 这是全 track 最难的一条，必须有测试。
5. **视图定义怎么持久化**（过滤 / 排序 / 分组 / 可见列 / 布局 一起存，SPEC 原文）。
6. **Markdown 通道怎么办**（§二十六 是内容通道）：一个 database 导出成什么？表格？一行标记？不导出？**必须有一个明确答案**，不能沉默 —— 上一个 slice 已经把「内容通道不含属性」这件事挂在了明面上（ADR-0044 的未验证边界第三条）。

### D1 · 数据层：`databases` / 属性和值的表 / 记录的存储
- 表、迁移、repository 读写、`core` 侧的对象模型（`Database` / `Property` / `Record` / `View`）。
- 重建：一个库从磁盘读回来必须与关掉前逐字段一致（照既有 `persistence_test.rs` 的做法）。
- 记录与页面的所有权/生命周期契约（D0 第 4 条）在这里落地。

### D2 · property 系统（14 种类型）
必做：`title / text / number / select / multi-select / status / date / checkbox / url / email / phone / files / created time / last edited time`。
降级：`person` —— 没有账号体系，退化为工作区内本地成员名单，**纯字符串**。
- `created time` / `last edited time` 是**派生值**，不许双写（照 ADR-0039 的纪律）。
- `files` 复用既有附件通道（ADR-0029/0030），不要另造一套落盘。
- 每种类型要有一个「存进去读回来还是它」的往返测试；`date` 与 `number` 的排序必须是**数值序/时间序**而不是字符串序（这是最容易悄悄错的一条，要有测试）。

### D3 · 第一个视图：table view
- 走**虚拟化**：SPEC 原文「10 000 行不得全量 realize；视图先算可见窗口再取行」。
- 交付：列宽与隐藏列、行内编辑（单元格编辑复用既有的单个 `TextEdit` 纪律，照 ADR-0031 的 table 怎么做 —— 但**不要改** ADR-0031 的简单表格，那是另一个 kind）、视图切换器（此刻只有一个视图，但切换器的形状现在就要立对，否则后面每加一个视图都要改一次）。
- 单元格里的行内编辑要能承载属性类型（select 是下拉、checkbox 是勾、date 是日期选择）—— 这是 UI 量最大的一刀，**先做 title/text/number/checkbox，其余类型逐个补**，不要一次全铺。

### D4 · filter / sort / group
- **必须在 SQL 侧完成，不在 UI 侧过滤**（SPEC 原文，也是最容易违反的一条）。这一条要有证据：量「10 000 行的库加一个过滤条件」的耗时，与「取回 10 000 行再在内存里过滤」对比。
- group by 的渲染同样走虚拟化（分组头不能被实现化成 10 000 行）。

### D5 · 视图族（除最后一种）
顺序即实现顺序：**board → list → calendar → gallery → timeline → form**。
- 每个视图都要：虚拟化、视图定义持久化、切换耗时进 `docs/PERFORMANCE.md`。
- 不要为某个视图引入渲染库；用现有 Slint primitive。

> **D5 结束就是一条完整可交付的竖切**（能建库、能填、能过滤排序、表格视图能用）。如果要把这条 track 再拆给两个 agent，**切点在这里**：D0–D5 是「基座 + 表格视图」，D6–D8 是「计算属性 + 视图族 + 高级特性」。

### D6 · 计算属性：formula / rollup / relation
- **公式引擎的限制（SPEC 原文）**：纯词法 + 自写解释器，**不引入 JS / WASM 运行时**；表达式必须**有限求值**；**relation 环检测在保存时做，不在渲染时做**。
- **必须可增量重算，禁止每次输入全库重算**（SPEC 性能红线）。这一条要有一个数字：改一个单元格之后重算了多少行。
- **relation 依赖 §四十 的引用基础设施 —— 也就是 Track 2 的 `@page mention` 那一层。** 所以这个 phase **排在 Track 2 落地之后**；Track 2 没落地就先做 formula / rollup，relation 单独留一刀，不要自己另造一套引用机制（SPEC 排期前提的原话就是「否则简单表格和 relation 会各造一遍轮子」）。
- 双向关系、rollup 目标属性、`person` 的退化形态都要在 ADR 里写清。

### D7 · 高级特性
- `chart` 视图：**放最后，且不得为此引入图表库**；先用现有绘制 primitive 做 bar / line / pie 三种（SPEC 原文）。
- `linked database`：引用另一个库的某个视图，**不复制数据**。
- 数据库模板：新建 record 时的预填。注意 Track 1 在做「页面模板」，两边都要求「模板是内容的副本、不引入第二套内容格式」—— **照同一条纪律做，但不要改 Track 1 的文件**；如果两边需要共享一个形状，报给整合者，不要自己跨过去改。
- 视图内搜索（复用 §二十 的索引还是数据库自己的 SQL？出 ADR）。

### D8 · 性能收口
SPEC 原文要求**三个数字进 `docs/PERFORMANCE.md`**：
1. 10 000 行的库的内存；
2. 切换视图的耗时；
3. 打开公式编辑器的耗时。
另外你自己在 D2–D4 里欠下的（过滤耗时、增量重算行数、只编辑一个单元格的成本）也一起收。

---

## 4 · 每个 phase 都要过的门槛

1. `cargo check --all-targets` 干净、`cargo test --all-targets` 全绿（报数：passed / failed / ignored，按 target 分开报）、`cargo build --release` 零警告。
2. **视觉**：新场景加进 `apply_scene`（含 `dark-*` 臂；表格视图、每个视图族、filter 面板、公式编辑器都要有臂），然后
   `benchmarks/scripts/sweep.ps1 -OutDir .scratch/sweep<N> -Baseline .scratch/sweep<N-1>`，
   再用 `benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` 出 bbox 表，**逐场景说明谁动了、动在哪、为什么是它的改动解释的**。当前基线是 `.scratch/sweep32`（64 场景）。
   > 如果点亮了「+」菜单里那几行 muted 占位，`menu` / `plus` / `slash` 这几个场景必然动 —— 那正是要看到的数字。
3. **性能**：这条 track 的**每一刀**都欠数字（SPEC 性能红线是明写的），走 `benchmarks/scripts/bench.ps1` 的两臂对照；**必须有对照构建**（前一刀的 commit 在干净 worktree 里编出来，两臂交替跑，各自先自我标识 md5/size）。这个项目把四次「同向上涨」记成过「会话漂移」，后来靠同会话对照才排除掉。
4. `docs/SPEC.md` §三十九 对应小节标注已交付 + ADR 号（照 §三十七 各条的写法）。
5. `docs/REPORT_TRACK3.md` 更新（PLAN 段落 / CHANGELOG 条目 / ADR 全文 / 未验证边界）。
6. 未验证项**诚实列出**。D0–D5 若只做到 D3，就把「D4–D8 未做」写在报告第一段，不要用「进行中」含糊过去。

---

## 5 · 明确不做 / 不要碰

- **不要改 ADR-0031 的简单表格**（`table` kind 是网格，不是数据库）。你要做的是**新的** kind / 实体；把两者混起来是本 track 最大的风险。
- **不要另造引用机制**：relation 用 §四十 的基础设施（Track 2），不自己写一套 id 表。
- 不引入 WebView、JS/WASM 运行时、网络客户端、云同步、协作、评论、发布站点（SPEC §一/§三十三）。
- 不引入图表库、公式解析库、ORM（SPEC 明写「纯词法 + 自写解释器」；引库要先出 ADR 并附内存数字）。
- 不要做「依赖账号/成员的属性语义（Person 的协作含义、权限）」（SPEC §三十八 `## 本阶段不做` 与 §三十九 的降级条款）。
- **M9 Android 仍然停着**（用户 2026-09-20 明确决定）。
- 不要动 `[profile.release]`（ADR-0024 钉住的）。
- 不要改 Track 1/2/4 的功能代码；发现那边的 bug → 报出来。

**加依赖要谨慎**：`Cargo.toml` 被四条 track 共享，`[dependencies]` 只允许**追加**，同时 `[profile.release]` 一个字都不许动。加任何新 crate 之前先问：能不能用已有依赖 + 现有 primitive 做完？（ADR-0001 立的规矩是「单进程、Rust core、Slint UI、无 web runtime」，它比省事重要。）

---

## 6 · 报告格式

每个 phase 完成后（以及最后）报告一次：

- **改了哪些文件**：路径 + 为什么，关键行号；
- **跑了哪些命令**，输出是什么（粘关键行）；
- **门槛结果**：测试数、sweep 对比表、性能数字；
- **D0 那六个问题的答案落在哪个 ADR**（这是评审这条 track 的入口）；
- **偏离本 brief 的地方和为什么**（偏离本身不是错，隐瞒才是）；
- **给整合者的注意事项**：要折进 PLAN / CHANGELOG / DECISIONS / PERFORMANCE 的内容；
- **你没碰但注意到的别人 territory 的问题**。

做不完就诚实报状态。**一个未完成但诚实上报的 phase，胜过一个悄悄改坏了范围的 phase。**
