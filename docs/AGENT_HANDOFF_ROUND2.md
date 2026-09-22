# 第二轮派活 · T1 / T2 的空闲产能

快照时间：2026-09-22 01:42。**这份文件里的数字会过期**，动手前先自己重跑 §0 的三条命令。

工作区当前停在 `track/3-database`，HEAD = `0786336`（T3 刚提交的 D0）。

---

## 0 · 现场

```
git log --oneline -3
git status --short
cargo check --all-targets 2>&1 | tail -3
```

| Track | 你以为 | 01:42 实际 |
|-------|--------|-----------|
| **T1** 页面外观 | 完了 | `master` 上两个提交：`62a86c2`（页存 emoji）、`78ddf35`（ADR-0046）。SPEC §三十八 的五刀里**只交了 icon**：emoji 那一半 ✅ ADR-0045；本地图片那一半 ❌ 由 ADR-0046 判定不做在 icon 上（改归 cover）。**cover / lock / version history / 模板 四刀一刀未动**；`docs/REPORT_TRACK1.md` **不存在** |
| **T2** 引用与反向链接 | 完了 | ADR-0050/0051 写完，但**实现正停在半路**：`MarkKind::Mention \| Date` 与 `Mark.date` 已进 `src/core/types.rs`，调用点没跟上 —— 此刻 `cargo check --all-targets` 是**红的**（19 个 `E0063 missing field date` + 4 个 `E0004` 非穷尽 match），`cargo build --release` 同样红。报告里写的分支 `track/2-references` **不存在** |
| **T3** Database | 没完 | D0 已提交（`0786336`：探针 + ADR-0060…0065 + `src/core/database.rs`）。D1–D8 未开始。仓库里 `PLAN.md` / `docs/DECISIONS.md` / `docs/SPEC.md` 仍显示 modified，但那是 **T2 与 T4 未提交的内容**（见上两行），不是 T3 留下的脏数据 |
| **T4** 欠账 + RC | 没完 | Slice 0/1 已提交（`7825d66`：门槛脚本化 + 两个 verify 脚本的删除守卫修复）。**它已经把两份 ADR 草稿写出来了（未提交）**：ADR-0080 = PDF 首页缩略图走纯 Rust 光栅器（`hayro` 已落进 `Cargo.toml`），ADR-0081 = **bookmark 关掉而不是延后**（由 embed 卡片承担），并据此改了 `docs/SPEC.md`。T4.3 未开始 |

**一句话**：此刻**没有任何一条 track 能过门槛**，挡住的是 T2 那半刀。

---

## 1 · 给 T1 agent 的下一批活（四刀 + 两件小事）

就是它自己 brief 里没做完的部分，不需要新范围：

| 顺序 | 刀 | 约束 / 现成的坑 |
|------|----|----------------|
| 1 | **lock**（只读开关） | `pages.locked` 一列（SPEC §三十八 的数据前提）。四处入口全关（TextInput / slash 菜单 / 拖拽 / ⋮⋮ 编辑项）+ 可见锁定态，**不许静默吞输入** |
| 2 | **cover** | 它同时是 ADR-0046 的下游：那张图的家在这里。**必须先回答**：`reclaim_attachments`（`src/app/state.rs:2017`）的「活着」集合只来自**块的 `attachment` 字段 + undo 栈 + 剪贴板**，它不认识任何**页级**引用 → cover 一旦把附件 id 存进 `pages.cover`，下一次回收就会删掉用户的封面图。ADR-0046 已为 icon 记过这个陷阱，cover 必须真修掉它。另：封面之上的标题对比度要过 §二十一，不得用最弱配色 |
| 3 | **模板** | 页内按钮 + 新建时选模板 + workspace 模板库；**表示必须是「块序列的副本」**，不得引入第二套格式；导入导出走 §二十六。不需要新迁移 |
| 4 | **version history** | 复用 §二十五 的 snapshot，不另造存储；**还必须给保留策略的磁盘与 RAM 数字**（不接受无限增长）→ 要有对照构建，放最后。第二个坑：命名版本是**磁盘上的快照**，reclaim 同样看不见它们 → 「快照算不算引用者」必须在这一刀回答，否则恢复旧版本会恢复出一张已被回收的空图 |
| 5 | **侧边栏删页进 undo**（小） | `src/app/state.rs:2394` 的 `delete_page` 记完 `Change::PageDeleted` 就删，历史栈只有编辑器命令。T3 报告 §8.2 明说「真正的修法在 Track 1 的 territory」，ADR-0063 也把这条写成明面缺口。修掉它，那条缺口就能划掉 |
| 6 | **补 `docs/REPORT_TRACK1.md`** | brief §2.5 要求的四样（PLAN 段落草稿 / CHANGELOG 条目 / ADR 全文 / 未验证边界）现在一份都没有。上一刀的 PLAN 段虽然直接进了 `PLAN.md`，但 ADR 全文与未验证清单没有落处 |

**号段**：cover / lock / version history 取 ADR-0047/0048/0049；**模板借 0052**（`0050–0059` 名义上归 T2，但 T2 只占了 0050/0051）—— 这一条要你点头。

**迁移体一律走 `add_page_columns()`**（缺哪列补哪列），配「v11 库升上来读回原值」的断言。

---

## 2 · 给 T2 agent 的活（先把红收掉，再谈继续）

1. **最高优先级：让 `cargo check --all-targets` 回绿。** 19 个 `E0063 missing field date` + 4 个 `E0004 MarkKind::Mention|Date not covered`。在回绿之前 T1/T3/T4 全都跑不了门槛，这是它对另外三条 track 的实时阻塞。

2. **ADR-0050 有两处必须先解决，不要继续往下写：**
   - **自相矛盾**：它说「不新增列」，又说「date 写入 `url = ""`，日期内容存入 `Mark.date: Option<String>`」。但 `marks` 表的列只有 `block/start/end/kind/url`（`src/storage/migrations.rs:99-106`；`kind` 是 `TEXT NOT NULL`，没有 CHECK），**`Mark.date` 没有落地的列** → 存盘再读回来日期就没了。二选一：
     - **（推荐）date 也存 `url`**（`url = "2026-09-22"`；`Mark.date` 删掉，或读时从 `url` 派生）→ **零迁移**；
     - 或者真加一列 → 那么「无列变更」那句要改，迁移号要跟 T1/T3 排。
   - 既然新 kind 纯是 Rust 侧字符串，ADR-0050 里「DB migration v12 仅注册两个 MarkKind 字符串」这一步应该是**零迁移**。v12/v13 正被 T1 的 cover/lock 与 T3 的 D1 抢，别白占。

3. **提交纪律**：`docs/DECISIONS.md` 里现在**同时**躺着你的 ADR-0050/0051 与 T4 的 ADR-0080/0081（T3 的 0060…0065 已经在 `0786336` 里提交了），`PLAN.md` 只有你的 M13 小节；代码侧 T4 的 `Cargo.toml`+`Cargo.lock`（hayro）/ `src/services/pdf_thumb.rs` / `install/verify-*.ps1` / `ui/components/EditorBlock.slint` 与你的 `types.rs`/`command.rs`/`state.rs`/`controller.rs` 都在同一棵树上。`git add` 按文件走，`DECISIONS.md` 这种共享文件要么用 T3 那套 `git hash-object -w` + `update-index --cacheinfo` 只写自己那段（可以抄），要么跟 T4 约一个先后。

4. **分支**：报告里写的 `track/2-references` 不存在 —— 补建，还是把报告那行改掉，你定（见 §3.4）。

5. 然后按 T2.1 → T2.4 继续（清单在 `PLAN.md` 的 M13 小节）。其中 **ADR-0051 自己写下的 Known Gap**（`delete_block` 路径要把 `__backlink:<id>` token 从旧 content 里摘掉，否则残留）必须一起做；**不许全库扫描、不许双写**这两条是硬要求。

6. 做完 M13 就**解锁 T3 的 D6 relation** —— T3 的 brief 里明写 relation 依赖 T2 的引用基础设施，没落地它就先做 formula/rollup。

---

## 3 · 要你拍板的四件事

1. **T4.2 bookmark —— 要你追认或撤。** T4 没停在那儿等：它写了 ADR-0081「**bookmark 关掉而不是延后**，由 embed 卡片承担」，并据此改了 SPEC。它还纠正了我 brief 里的一个前提 —— Quire **有** HTTP 客户端（`src/services/lan_client.rs`，无 TLS，只在 `--pull` 时用），所以真正的问题不是「有没有网络」，而是「要不要为 favicon 引入 TLS」。三条路的代价在 `docs/REPORT_TRACK4.md` §T4.2。**你若同意"关掉"，这一刀就地结束；若不同意，现在撤还来得及**（它还没提交）。
2. **T4.1 PDF 缩略图 —— 追认或撤。** ADR-0080 已按 `hayro`（纯 Rust，+4.55 MiB / +21.2% exe）写成草稿，`Cargo.toml` 也改了；`default-features = false` 把 jpeg2000 关掉了。撤的话要回收那一版依赖与 `src/services/pdf_thumb.rs`。
3. **迁移号排表。** `CURRENT_VERSION = 11`。建议现在就定死：**v12 = cover（T1）→ v13 = lock（T1）→ v14 起归 T3 的 D1**，**T2 零迁移**。不排的话三条 track 各自声明的「提交那刻 +1」必然撞（它们已经各自声明过一次 12）。
4. **T2 补不补分支。**

---

## 4 · 话术（整段粘贴）

### 话术 A —— 给 Track 1 agent（第二轮）

```
你是 Quire 的 Track 1 agent（页面外观）。第一件事已经完成并由你提交在 master 上：
62a86c2（页存 emoji）、78ddf35（ADR-0046），在途的迁移 v11 也归你。

现在继续你自己 brief 里剩下的四刀，范围不变，不要越界到别的 track ——
docs/SPEC.md §三十八 的 cover / lock / version history / 模板 四节原文就是范围。

顺序：lock → cover → 模板 → version history。理由：
- lock 最便宜，且能立刻关掉「静默吞输入」这条风险；
- cover 是 ADR-0046 的下游（那张图的家在这里），做完它 icon 那两刀才算真收口；
- 模板不需要新迁移（「块序列的副本」），主要成本在入口与 §二十六 Markdown 通道；
- version history 最重，要复用 §二十五 的 snapshot 并给保留策略的磁盘/RAM 数字，
  必须做对照构建，放最后。

两个坑，写进你的 ADR，不要留给下一刀：
1. src/app/state.rs:2017 的 reclaim_attachments，它的「活着」集合只来自块的 attachment
   字段 + undo 栈 + 剪贴板，不认识任何**页级**引用。cover 一旦把附件 id 存进
   pages.cover，下一次回收就会删掉用户的封面图。ADR-0046 已经为 icon 记过这个陷阱。
2. version history 的快照是磁盘上的，reclaim 同样看不见它们 —— 「快照算不算引用者」
   必须在这一刀回答，否则恢复旧版本会恢复出一张已被回收的空图。

另外两件小事：
- 补 docs/REPORT_TRACK1.md：brief §2.5 的四样（PLAN 段落草稿 / CHANGELOG 条目 /
  ADR 全文 / 未验证边界），现在一份都没有。
- 侧边栏删页不进 undo（src/app/state.rs:2394 delete_page）。T3 的报告 §8.2 明说
  「真正的修法在 Track 1 的 territory」，ADR-0063 也把这条写成明面缺口。它是你的一刀。

号段：cover / lock / version history 取 ADR-0047 / 0048 / 0049；模板借 0052
（0050–0059 名义上归 Track 2，但它只占了 0050/0051）。一律追加在
docs/DECISIONS.md 末尾，不要插中间。

迁移号用**提交那一刻**的 CURRENT_VERSION + 1（今天 = 12），提交前重读一次
src/storage/migrations.rs；迁移体走 add_page_columns()，配「v11 库升上来读回原值」的断言。

注意：现在的树是红的（Track 2 半刀，19 个 E0063 + 4 个 E0004），那不是你的 territory，
不要替它修；但你的门槛要等它回绿才能跑，可以先把代码写完。

做不完诚实报状态，未验证边界要列出来。
```

### 话术 B —— 给 Track 2 agent（先收尾，再继续）

```
你是 Quire 的 Track 2 agent（引用与反向链接）。ADR 阶段完了，但实现停在半路，
现在整棵树是红的：cargo check --all-targets 报 19 个 E0063
missing field date in initializer of Mark / Command，加 4 个 E0004
MarkKind::Mention | Date not covered。这是你对另外三条 track 的实时阻塞
（T1/T3/T4 都要跑同一个门槛），最高优先级是把它收掉、让门槛回绿。

收的时候，ADR-0050 有两处必须先解决，不要继续往下写：

1. 它自相矛盾：说「不新增列」，又说 date 存进 Mark.date: Option<String>。但 marks
   表的列只有 block / start / end / kind / url（src/storage/migrations.rs:99-106，
   kind 是 TEXT NOT NULL，没有 CHECK），Mark.date 没有落地的列 —— 存盘再读回来日期
   就没了。二选一：(推荐) date 也存 url（url = "2026-09-22"，Mark.date 删掉，或读时
   从 url 派生）→ 零迁移；或者真加一列 → 「无列变更」那句要改，迁移号要跟 T1/T3 排。
2. 既然新 kind 纯是 Rust 侧字符串，ADR-0050 里「DB migration v12 仅注册两个 MarkKind
   字符串」这一步应该是零迁移。v12/v13 正被 T1 的 cover/lock 与 T3 的 D1 抢，别白占。

提交纪律：树上同时有 T3 的未提交内容（src/core/database.rs / docs/SPEC.md /
benchmarks/results/2026-09-22-track3-probe.jsonl，已 staged）与 T4 的
（Cargo.toml + Cargo.lock 的 hayro / src/services/pdf_thumb.rs / install/verify-*.ps1 /
ui/components/EditorBlock.slint）。只 git add 你自己那几个文件，或者等 T3 提交完再提交。
T3 用 git hash-object -w + update-index --cacheinfo 避开了夹带，可以抄。
另外你报告里写的 track/2-references 分支不存在，要么补建要么改报告。

回绿之后按 T2.1 → T2.4 继续（清单在 PLAN.md 的 M13 小节）。其中 ADR-0051 自己写下的
Known Gap —— delete_block 路径要把 __backlink:<id> token 从旧 content 里摘掉 ——
必须一起做。T2.3 / T2.4 的反向链接不许全库扫描、不许双写，这两条是硬要求。

做完 M13 就解锁 T3 的 D6 relation（T3 的 brief 里明写 relation 依赖你的引用基础设施）。

做不完诚实报状态，未验证边界要列出来。
```

---

## 5 · 顺序建议

1. **T2 先回绿**（它挡着所有人）。
2. **T1 同时开工 lock**（不与任何人共享文件：`pages` 列 + 页首 UI）。
3. **T3 提交完 D0，直接进 D1**（D1 要建表，迁移号按 §3.3 排定后取）。
4. **T4 把 ADR-0080 / 0081 交给你拍板，同时并行推 T4.3**（已知限制逐条给结论不依赖你的决定）；追认之后再跑 T4.4 的 RC 门槛与出包。
