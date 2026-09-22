# Agent brief · Track 4 — 欠账清零与 Windows RC 收官

你是 **Quire** 的 Track 4 agent。Quire 是一个本地优先、Rust + Slint 1.18 的
Notion 式笔记应用：单进程、无 WebView、无 JS/WASM 运行时。
本文件是你的全部范围定义。

你这条 track 的性质和另外三条不同：**你不做任何新功能面**，你做的两件事是
「把已经写在账上的欠账做掉（PDF 缩略图 / bookmark 卡片）」和「把 M8 这个
release candidate 真正收口」。另外三条 track 在长新功能，你在**关账**。

---

## 1 · 开工前必读（按顺序，别跳）

| 文档 | 你要拿走的东西 |
|------|----------------|
| `docs/ARCHITECTURE.md` | 分层与六条硬规则；尤其 `platform/` 的定位（「只在 Slint 有真实缺口的地方做薄适配层」） |
| `docs/SPEC.md` **§三十七 批次 A**（image / file / PDF 三条，行 ~2278）与 **§三十七 批次 C**（bookmark 那一条，行 ~2358） | 你的两个欠账的需求原文 |
| `docs/SPEC.md` **§二十一**（可读性与视觉验收）、**§二十二**（性能预算）、**§三十三**（特别禁止）、**§二十四**（release profile） | 你的硬边界 |
| `docs/DECISIONS.md` **ADR-0029**（照片落盘与降采样缓存）、**ADR-0030**（file 附件不读取）、**ADR-0040**（embed 卡片只存地址、**刻意不抓取**）、**ADR-0035**（剪贴板位图：解码与 Win32 FFI 分离）、**ADR-0024**（release profile 已审计过）、**ADR-0015**（旋转快照） | 你的两刀都要照这些先例同构 |
| `CHANGELOG.md` 的 `### Known limitations`（文件末尾）与 `### Build & test` | **你的待办清单来源** |
| `docs/ROADMAP.md` 的 `## Definition of done` 与两张 `## M8 verification snapshot` 表 | T4.4 的模板：门槛是什么、表怎么填、什么叫「这一条没重跑」 |
| `docs/PERFORMANCE.md` §Method、§M8 · release profile audit | 你的两刀各自欠什么数字 |
| `PLAN.md` 最后三节 | 你的收尾报告要照这个文体与信息量写 |
| `docs/AGENT_BRIEF_M8_TAIL.md` | 上一轮「A1–A5 tail package」是怎么被切成任务的 —— 你的 T4.4 是它的续集 |

---

## 2 · 并行纪律（同一仓库同时有四条 track，必须照做）

### 2.1 分支与提交

- 一条 track 一个分支：`track/4-backlog-rc`，从 `master` 起。
- **一个 slice 一个提交**，只 `git add` 你这一刀明确改到的文件，**永不** `git add -A`。
- 永不做 `git checkout` / `switch` / `restore` / `reset` / `stash` / `rebase`。
- 只在 `cargo check --all-targets` 干净时才提交。不 push（除非用户明确说了 push）。

### 2.2 四条 track 的文件归属

| Track | 主题 | 主要 territory |
|-------|------|---------------|
| 1 | 页面外观 | `Page` 结构体、`pages` 列、封面/锁/模板/版本历史 |
| 2 | 引用与反向链接 | `MarkKind`、mention / date / 反向链接面板 |
| 3 | Database | 新模块为主（`core/database*`、`storage/database*`、`ui/components/Database*`）+ 新迁移 |
| **4（你）** | **欠账 + RC** | **`src/platform/**`、`src/services/**` 的新模块、`install/**`、`benchmarks/**`、`docs/PERFORMANCE.md` 你自己的小节、`Cargo.toml` 的 `[dependencies]` 追加** |

你是四条里与别人重叠最少的一条（你以 `platform/` / `install/` / `benchmarks/` 为主）。
**唯一的例外是 T4.3 的弹窗滚动**，那一刀会碰到 `ui/components/` 的菜单类组件，见下。

### 2.3 共享接缝（四个热文件）

1. `src/core/types.rs` / `src/storage/migrations.rs` —— **你原则上不该碰这两个**。你的两刀都不需要新的块字段或新迁移（PDF 缩略图是 file 块的绘制细节；bookmark 若复用 `embed` 的地址存储就不需要新列）。如果你发现非加不可，**停下来报告**，不要自己加。
2. `ui/Types.slint` / `src/app/controller.rs` —— 只加你自己的 property / callback / match 分支，不重排既有行；`apply_scene` 只追加你的场景。
3. `Cargo.toml` —— `[dependencies]` 只允许**追加**；`[features]` 与 `[profile.release]` **一个字都不许动**（后者是 ADR-0024 审计过的，改它必须重跑 A3 审计）。加任何新依赖之前先问：能不能用手头已有的东西做完？
4. `ui/components/*.slint`（T4.3）—— 只在你自己的区域改（菜单/弹窗的滚动容器），不要顺手整理别处，不要整段重排。Track 1 在页首、Track 2 在页尾，你们不要互踩。

### 2.4 ADR 编号配额

ADR-0001…0044 已占用。**你的号段是 ADR-0080…0089**，按顺序取。四条 track 同时追加，**ADR 直接追加在文件末尾**。

### 2.5 只有整合者能改的文件

`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` 由整合者统一收口，**你不直接改** —— 包括 T4.4 要填的那两张 `M8 verification snapshot` 表：**你把表写好交给整合者**（写进你的报告文件），不要自己去改 ROADMAP。
把 PLAN 段落草稿、CHANGELOG 条目草稿、ADR 全文、ROADMAP 表格草稿、未验证边界写进 `docs/REPORT_TRACK4.md`（新文件，只有你写）。
> 例外：`PLAN.md` 允许你**在末尾追加**小节；`docs/PERFORMANCE.md` 允许你追加自己的小节（那一节按项目惯例都带日期与 ADR 号）。

### 2.6 环境坑（本机实测，别踩第二遍）

- 本机把 **bash 的 `rm -rf`** 重定向到回收站：批量删除（阈值 50）返回非 0，stderr 只有一行 `SAFE_DELETE_BULK_CONFIRM_REQUIRED`，文件原样保留；**返回非 0 会让 `rm -rf x && next` 整条短路**，`next` 静默不执行。你会在 installer / portable 验证里反复清安装目录，**清理与后续步骤绝不要用 `&&` 串起来**。
- 重定向到文件的日志，命令没执行时**既不创建也不截断**，`tail` 读到的是上一次的陈旧内容。安装/卸载这类流程尤其要用带时间戳的独立文件名，或先看退出码再决定读不读。
- 同一文件**不要在同一条消息里发多个 `Edit`**（互相覆盖）。
- 改完必须**真跑一遍**，不要只信编辑返回的 success。
- 测试建目录一律用 `quire::testing::ScratchDir`；跑数据只写 `.scratch/`，**绝不碰 `%APPDATA%\Quire` 或仓库旁的 `appdata/`**。
- 交原生程序（`python.exe` / `adb.exe` / `curl.exe` / `git` / `gh`）的路径必须是原生形式（`C:/...` 或 `C:\...`），POSIX 形式 `/c/Users/...` 会被当成 `C:\c\Users\...`。**脚本里统一包一层转换**。`git commit -F` 与 `gh release create <file>` 的参数尤其吃这条坑。

---

## 3 · 任务清单

### T4.1 · PDF 首页缩略图（SPEC §三十七 批次 A 的欠账）
**现状**：`.pdf` 与 `.zip` 除文件名外长得一样。SPEC 原文「先按 file 处理 + 首页缩略图；内嵌翻页阅读器不在本阶段」；2026-09-20 用户指示「首页缩略图这一半先跳过」——**现在把它做掉**。

- **第一步是 ADR，不是代码**：渲染路线待定是你的核心问题。至少比较两条路并给出数字：
  a) 引一个 native PDF 库（体积、打包、许可证、以及它是否把字节读进进程）；
  b) 纯 Rust 解析 + 现有 raster/绘制 primitive（能力边界、能画出多少）。
  判据按 SPEC §二十二 优先级排在功能性之前，**并且**：SPEC §三十七 批次 A 对附件有一条硬要求 —— 「附件字节不得进入进程：流式复制落盘、只记长度，**不设体积上限也不解码**」。所以你**不能**把「打开页面时先把所有 PDF 解码一遍」当实现。缩略图必须是**按需、有上限、有缓存、可回收**的：照 ADR-0029 的照片缓存那一套（落盘 cache 文件 + LRU 上限 + UI 只拿降采样副本），并说清它和 `MAX_EDGE = 1280`、32 MiB LRU 的关系（共用还是独立，为什么）。
- 交付：file 块里对 `.pdf` 显示首页缩略图（其它文件种类不受影响）；缩略图生成失败要**可见退化**成现在的样子（文件名 + 体积），不得让行渲染失败。
- 数字：一张缩略图的生成耗时、缓存上限、一个 200 页 PDF 的进程开销（这些都要有对照，别只说「很快」）。

### T4.2 · `bookmark` 卡片（SPEC §三十七 批次 C 的欠账）
**现状**：未做。SPEC 原文「链接卡片，抓标题与 favicon；离线或抓取失败退化为纯链接，且不得阻塞输入」。`embed` 已经交付（ADR-0040），并且**刻意不抓取**——SPEC 里写得明白：「没有内嵌浏览器，所以『卡片 + 交给系统打开』就是功能的全部」「不抓标题、不抓 favicon、不预览（那是 bookmark 的活）」。

**⚠️ 这一刀要先把产品方向问清楚，再动代码。**
原因是它意味着一次方向转变：Quire 到目前为止**没有任何网络客户端**，这是 §一（本地优先）与 §三十三（特别禁止）立住的边界。

**你的第一步：写一份 ADR 草案，把三条路各自说完，然后停下来问用户，不要在没答复前实现抓取。**
a) **做**：引入一个 HTTPS 客户端（不许 WebView、不许 JS/WASM）。那么必须同时回答：只做用户显式粘贴/点击后的抓取，还是自动？超时多久？失败怎么退化（SPEC 已给答案：退化为纯链接）？**抓取绝不能在 UI 线程上（SPEC 原文「不得阻塞输入」）**；缓存放哪、占多少磁盘、要不要上限；以及隐私边界（一个本地笔记应用在用户没点的时候发不发请求）。
b) **不做**：出一份 ADR 把「bookmark 由 embed 卡片承担」写清楚，把 SPEC §三十七 批次 C 的这一条改成已决，并把「抓标题与 favicon」这条需求正式划到不做（要写理由：无 WebView/无网络客户端是设计而不是缺失）。
c) **中间路**：只解析地址本身能表达的信息（域名、路径、识别出的产品），也就是 embed 已经做的 —— 那么这一刀的实际交付是**把 embed 的边界写清楚，并把 bookmark 条目关掉**。

**无论选哪条，先给用户看 ADR 草案。** 用户点头之后再动代码；得到 a) 就按 a) 实现并补齐数字，得到 b)/c) 就把这一条在 SPEC/ROADMAP 里关掉（改写由整合者做，你给文案）。

### T4.3 · 已知限制收口（`CHANGELOG.md` 的 `### Known limitations` 就是你的清单）
逐个判断「收口」还是「写清即可」，**每一个都要有一个明确结论写进报告**，不允许留在那里不表态。其中至少这两条应当真的修掉：

- **「菜单高于窗口会溢出底部」**（原文：Move-to 在大工作区里溢出底部 —— 锚点会 clamp 但列表不滚动）。这是 SPEC §二十一 的 popup 正确性条目。给锚定弹窗做滚动（或分页/自适应高度）。注意：这个项目在「菜单高度写死」上栽过两次（`+` 菜单、`/` 菜单都是靠自己量高度修好的），**照那两次的做法做**。Settings 对话框已经没有这个问题，别把它改回去。
- **同一条的上游原因**：锚定弹窗的高度/位置计算散在多个组件里 —— 如果你发现可以收成一个共用的量高+clamp 路径，值得做（但要有 ADR 或至少写清代价）。

其余各条按这个口径处理（建议，不强制）：
- 「从一个已打开的菜单直接切到另一个要点两次」—— Slint popup 的标准语义。**大概率写清即可**，但要把「为什么不能修」写成一句可验证的话（不要只写「标准行为」）。
- 「图片替换磁盘文件需重启才显示」—— 要不要加文件 watcher？**判据是 §二十二**：一个 watcher 会带来轮询/线程与内存，收益只是「不用重启」。若判不做，必须写清是**判断**而不是遗漏。
- 「附件回收是手动的」—— 已经有 ADR-0037 讲清为什么（撤销契约保护 100 步、绝不列目录）。这一条**写清即可**，别去改。
- 「表格单元格里 Enter 不拆行 / 退格不合并 / 上下键不跨行 / Ctrl+L 没接」—— ADR-0031 已记录为边界。其中 **Ctrl+L 未接**是个小口子；要修的话它在 `ui/components/Editor.slint`（热文件），照 §2.3 的规则只改自己的区域，并报出来。
- 「带标记的段落仍有两种形状会裁切」—— Slint 的墙（ADR-0041 写在 `docs/EDITOR_ARCHITECTURE.md` §Platform wall）。**写清即可**，不要去啃。
- 「图片 bench 的 fixtures 是生成的渐变图，真实照片熵未测」—— 补一次高熵图的读数（不用真照片，用一张高熵噪声图即可），把「乐观」这个词从账上换成一个数字或一句明确的「仍是估计」。

### T4.4 · Windows RC 收官
**目标**：把 M8 从「🔄 in progress」变成「✅」，并且这一步的证据可以被人重跑。

1. **把门槛脚本化**：下面每条门槛写成一条命令 + 一句期望输出，先在**你开工时的 head** 上跑一遍留档，等四条 track 汇合之后再跑一遍作为 RC 证据（两轮都写进报告）：
   - `cargo check --all-targets` / `cargo test --all-targets`（报 passed / failed / ignored，按 target 分开） / `cargo build --release`（零警告）
   - `benchmarks/scripts/sweep.ps1`（最新基线；报「N 场景，M 动，动在哪」）
   - `benchmarks/scripts/audit_results.ps1`（`audit ok`：每张表都能从原始行重算）
   - `install/verify-installer.ps1`（4 项检查；**需要 Inno Setup**）
   - `install/verify-portable.ps1`（24 项检查；**绝不能碰真实的 %APPDATA%\\Quire**）
   - `just dist`（出包）
2. **填 ROADMAP 的 snapshot 表**（草稿交整合者）：表格格式照 `## M8 verification snapshot (2026-09-21, at 24a3432)`；**没重跑的门槛要明写「not re-run at this head」并给出理由**（照上一张表里那条「deliberately: this batch adds no block kind…」的写法）—— 这个项目的规矩是：宁可写「没跑」，不可让它看起来跑过。
3. **版本与文案收敛**：`Cargo.toml` 的 `version = "0.1.0"` 与 `CHANGELOG.md` 的 `## 0.1.0 — Windows RC (in progress)` 对齐；把 `(in progress)` 去掉的条件是**门槛全绿**。README 的安装/运行段落与实际分发物（installer + portable + zip）一致。
4. **把 `docs/` 的索引过一遍**：新增的文档（四条 track 的 brief 与 report）要不要进 README / ROADMAP 的引用列表。
5. 若发现**任何**门槛是红的，**不要修别人的 territory**，把失败原文与最小复现写进报告，交给对应 track 或整合者。

---

## 4 · 每个 slice 都要过的门槛

1. `cargo check --all-targets` 干净、`cargo test --all-targets` 全绿（报数）、`cargo build --release` 零警告。
2. **视觉**：新场景加进 `apply_scene`（含 `dark-*` 臂；PDF 缩略图、菜单滚动各要有臂），然后 `benchmarks/scripts/sweep.ps1 -OutDir .scratch/sweep<N> -Baseline .scratch/sweep<N-1>`，用 `benchmarks/scripts/diffbbox.ps1` 出 bbox 表，**逐场景说明谁动了、动在哪、为什么是它的改动解释的**。菜单滚动那一刀会让多个菜单场景动 —— 那正是要看到的读数。
3. **性能**：T4.1（缩略图生成与缓存）与 T4.2（若做网络抓取）都欠数字，写进 `docs/PERFORMANCE.md`；**必须有对照构建**（前一刀的 commit 在干净 worktree 里编出来，两臂交替跑并各自自我标识 md5/size）。T4.3 若只是给菜单加滚动，量「打开一个超高菜单的耗时与内存」即可。
4. `docs/SPEC.md` 对应条目标注已交付 + ADR 号（PDF 那条现在是「用户指示先跳过」，你要把它改写成最终状态 —— 文案交整合者，但变化要在报告里点明）。
5. `docs/REPORT_TRACK4.md` 更新。
6. 未验证项诚实列出。**T4.4 尤其**：installer 与 portable 两套验证依赖本机装了 Inno Setup，跑不了就明写「未跑 + 原因」，不要跳过不提。

---

## 5 · 明确不做 / 不要碰

- **不做任何 M12 / M13 / M14 的功能**（页面外观、引用、Database 是另外三条 track）。发现它们的问题 → 报出来。
- **M9 Android 仍然停着**（用户 2026-09-20 明确决定）。不要为它改 cfg、加依赖、动 `platform::data_dir`。
- 不引入 WebView、JS/WASM 运行时（SPEC §一/§三十三）。T4.2 即使在用户点头之后也只允许「一个 HTTPS 客户端」，不许 WebView。
- 云同步、协作、评论、发布站点：不做。
- 不要动 `[profile.release]`（ADR-0024 钉住的）与 `[features]` 的渲染器矩阵（ADR-0004）。
- 不要为了让门槛变绿而改测试、放宽断言或删场景。**红就是红。**

---

## 6 · 报告格式

每个 slice 完成后（以及最后）报告一次：

- **改了哪些文件**：路径 + 为什么，关键行号；
- **跑了哪些命令**，输出是什么（粘关键行）；
- **门槛结果**：测试数、sweep 对比表、性能数字；
- **T4.3 的逐条结论**：每一条已知限制 → 修掉 / 写清（附理由），一条都不许留白；
- **偏离本 brief 的地方和为什么**；
- **给整合者的注意事项**：PLAN 段落 / CHANGELOG 条目 / ADR 全文 / ROADMAP snapshot 表草稿；
- **你没碰但注意到的别人 territory 的问题**。

做不完就诚实报状态。**一个未完成但诚实上报的 slice，胜过一个悄悄改坏了范围的 slice。**
