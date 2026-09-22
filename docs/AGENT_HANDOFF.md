# Agent handoff — 四条 track 的开场话术

把对应那一段整段发给一个 agent 即可。话术只负责「定位 + 第一步 + 交付坐标」，
细节全在 brief 里，**不要让 agent 只看话术就开工**。

发送顺序建议：T1 → T3 → T4 → T2（T1 先收口在途的 v11；T3/T4 与别人正交，可随时开；
T2 与 T1 在 `Editor.slint` / `types.rs` 上会碰，最好等 T1 那个提交落地）。
四条同时开就必须各给一个 `git worktree` + 独立 `CARGO_TARGET_DIR`。**这一条已经落成
`docs/WORKTREE_TRACKS.md`（tracked，在 master 上）**：里面有照抄即用的建法、登记表现状，
和两个实测坑——`CARGO_TARGET_DIR` 千万别指到 worktree 外面（`sweep.ps1:23` 用的是
CWD 相对的 `target\debug\quire-shot.exe`，指出去像素闸直接 `exit 1`），以及本机 C: 只剩
46 G 而一个能跑 shot 的 worktree 吃 4.9 G。开 worktree 前先读那一页。

---

## 发给 Track 1

```
你是 Quire 项目的 Track 1 agent，负责「页面外观收尾」。工作树就是当前仓库
（Rust + Slint 1.18 的本地笔记应用，单进程、无 WebView/JS/网络客户端）。

第一步：完整读 docs/AGENT_BRIEF_T1_PAGE_APPEARANCE.md，它定义了你这条 track 的
全部范围。读完再按它 §1 的清单去读详细文档——那份 brief 本身不含细节，
细节在 docs/SPEC.md §三十八、docs/DECISIONS.md 的 ADR-0044、docs/PLAN.md 最后三节、
docs/UI_ARCHITECTURE.md 的 geometry/language traps 那几节里。

你的分支：track/1-page-appearance，从 master 起。

你要知道的第一件事：工作树里已经有一份**未提交**的 icon 切片（24 个文件 modified，
外加 src/core/icon.rs 与 ui/components/IconPicker.slint 两个新文件，schema 已经推到
v11 = pages.icon）。所以你的第一个任务不是从零开始写 icon，而是**把这份在途改动
复核、补齐缺口、跑完门槛、提交**。你享有迁移号 v11–v13 的优先权，这也是另外三条
track 要等你这一步的原因。

交付坐标：PLAN.md 允许你在末尾追加小节；ADR 用 0045–0049，追加在
docs/DECISIONS.md 末尾；其余草稿写进 docs/REPORT_TRACK1.md。
PLAN / CHANGELOG / ROADMAP 不要直接改（整合者收口）。

另外三条 track 在同一个仓库并行，brief §2 的接缝规则（四个热文件、迁移串行、
enum 只追加不重编号）是硬要求。

做不完就诚实报状态，未验证的边界要列出来——这个项目宁可见「没跑」，
不可让它看起来跑过。有需要我拍板的地方（比如 icon 要不要做本地图片那一半），
停下来问我，不要自己选。
```

---

## 发给 Track 2

```
你是 Quire 项目的 Track 2 agent，负责「引用、提及与反向链接」（SPEC §四十 / M13）。
工作树就是当前仓库（Rust + Slint 1.18 的本地笔记应用，单进程、无 WebView/JS/网络客户端）。

第一步：完整读 docs/AGENT_BRIEF_T2_REFERENCES.md，它定义了你这条 track 的全部范围。
读完再按它 §1 的清单去读详细文档——尤其 docs/SPEC.md §四十 原文、
docs/DECISIONS.md 的 ADR-0026（引用存 id 的契约）、ADR-0014（FTS5 增量索引）、
ADR-0039（派生数据不入库）、ADR-0041（带标记的行按词断行）、
docs/EDITOR_ARCHITECTURE.md（runs 通道）。

你的分支：track/2-references，从 master 起。

你的第一件事是出 ADR，不是写代码：mention 的载荷是复用 marks 表的 url 列，
还是加一列？marks 表主键是 (block, start, kind)，同一位置一个 mention 与一个 link
能不能共存，决定要不要改主键——这是真迁移，必须先决定。同一批要定的还有
Markdown 往返语法（mention 与 date 一起定）。

交付坐标：PLAN.md 末尾追加；ADR 用 0050–0059，追加在 docs/DECISIONS.md 末尾；
其余草稿写进 docs/REPORT_TRACK2.md。PLAN / CHANGELOG / ROADMAP 不要直接改。

两条最容易违反的硬要求，写在 brief 里也再提醒一次：反向链接**不许每次打开页面
全库扫描**（挂进 §二十 的增量索引），也**不许双写进库**。

Track 1 也在改仓库，它先落 pages 那几列。它的区域是页首（封面、标题图标），
你是页尾（反向链接面板）与正文里的 chip；同一张 .slint 里只改自己的区域。

做不完就诚实报状态，未验证边界要列出来。
```

---

## 发给 Track 3

```
你是 Quire 项目的 Track 3 agent，负责 Database（SPEC §三十九 / M14）——这是全计划
最大的一项，SPEC 自己写着「这是 Quire 与 Notion 差距最大的一层」。
工作树就是当前仓库（Rust + Slint 1.18 的本地笔记应用，单进程、无 WebView/JS/网络客户端）。

第一步：完整读 docs/AGENT_BRIEF_T3_DATABASE.md。它把这条 track 切成 D0–D8 九个阶段，
并标出「D0–D5 是一条可以独立交付的竖切、拆点是 D5/D6」。读完再按它 §1 的清单去读
详细文档——尤其 docs/SPEC.md §三十九 原文与 §三十七 硬性约束、docs/DECISIONS.md 的
ADR-0031（table 是网格不是数据库，这是你的边界）、ADR-0028/0032（行是动态的块长什么样）、
docs/PLAN.md 的 M10 批次 B slice 4/5 两节（最重要的先例）。

你的分支：track/3-database，从 master 起。

按顺序做：
(1) **先证明通道存在**——在写任何 UI 之前，先证明「10 000 行的库不全量 realize」
    这条形状真的成立，并给出一个数字。做不到就是形状选错了，早返工。
(2) D0 出 ADR：brief 里列了六个必须回答的问题（database 是什么实体、schema 存哪、
    值怎么存、record 与 page 的可逆关系、视图定义怎么持久化、Markdown 通道怎么办）。
    这六个答案是评审你这条 track 的入口，**先写下来再动手**。
(3) 然后 D1 → D8。D6 的 relation 依赖 Track 2 的引用基础设施，没落地就先做
    formula/rollup，别自己另造一套引用机制。

交付坐标：PLAN.md 末尾追加；ADR 用 0060–0079（你的号段最大），追加在
docs/DECISIONS.md 末尾；其余草稿写进 docs/REPORT_TRACK3.md。
PLAN / CHANGELOG / ROADMAP 不要直接改。

这是四条 track 里唯一欠「每一刀都要有性能数字」的，SPEC 的性能红线是明写的：
filter/sort 在 SQL 侧、10 000 行不全量 realize、formula 增量重算。

做不完是正常的——做到哪报哪，报告第一段就写清哪些阶段没做，不要用「进行中」含糊过去。
```

---

## 发给 Track 4

```
你是 Quire 项目的 Track 4 agent，负责「欠账清零 + Windows RC 收官」。
工作树就是当前仓库（Rust + Slint 1.18 的本地笔记应用，单进程、无 WebView/JS/网络客户端）。

第一步：完整读 docs/AGENT_BRIEF_T4_BACKLOG_RC.md，它定义了你这条 track 的全部范围。
读完再按它 §1 的清单去读详细文档——尤其 docs/SPEC.md §三十七 批次 A（PDF 那条）
与批次 C（bookmark 那条）、docs/CHANGELOG.md 的 Known limitations（就是你的待办清单）、
docs/ROADMAP.md 的那两张 M8 verification snapshot 表（T4.4 的模板）、
docs/DECISIONS.md 的 ADR-0029/0030/0035/0040、docs/AGENT_BRIEF_M8_TAIL.md（你的前传）。

你的分支：track/4-backlog-rc，从 master 起。

你要知道的第一件事：**T4.2 的 bookmark 不要直接开工**。SPEC 要「抓标题与 favicon」，
但 Quire 至今没有任何网络客户端，ADR-0040 把「不抓取」写成了 embed 的设计。所以先写
一份 ADR 草案，把「做 / 不做 / 退化成 embed 卡片」三条路各自的代价说完，
**停下来问我**，我不答复就不要实现抓取。

顺序建议：先把 T4.4 的门槛脚本化并在当前 head 上跑一遍留档（这样你后面有对照），
再做 T4.1（PDF 首页缩略图，第一步同样是 ADR 选渲染路线，注意 SPEC 那条
「附件字节不得进入进程」的硬要求），T4.3 的已知限制逐条给结论（修掉 或 写清+理由，
一条都不许留白），最后等四条 track 汇合后再跑一遍门槛作为 RC 证据。

交付坐标：PLAN.md 与 docs/PERFORMANCE.md 允许你在末尾追加；ADR 用 0080–0089，
追加在 docs/DECISIONS.md 末尾；ROADMAP 那两张 snapshot 表你写草稿进
docs/REPORT_TRACK4.md，不要自己改 ROADMAP。

你这条 track 不做任何新功能面，也不要去修另外三条 track 的 territory——
发现就让它们修，你报出来。

做不完就诚实报状态。installer / portable 两套验证需要本机装了 Inno Setup，
跑不了就明写「未跑 + 原因」，不要跳过不提。
```

---

## 4 · 汇合现场（整合者记，2026-09-22 17:3x）

四条 track 的未提交改动已经在 `a89f91b` 一次性进 `track/3-database`（工作树共享、HEAD 只有一个，
所以「按 track 分别提交」在这棵树上做不到 —— 只能按 hunk 分）。**唯一被排除的是 T4.1 的 PDF 首页缩略图**
（用户 2026-09-22 决定「pdf先不做」）：

| 内容 | 状态 |
|------|------|
| hayro 依赖（`Cargo.toml` / `Cargo.lock`）、`src/services/{mod,attachment_store}.rs` 的 PDF 分支、未跟踪的 `src/services/pdf_thumb.rs` + `tests/fixtures/*.pdf` + `make_fixture_pdfs.py` | **留在工作树里未提交**；要做就整套重新落地 |
| SPEC §三十七 的 PDF 小节、`CHANGELOG` 的 PDF 条目、`ROADMAP` M10 的交付注 | **未写**，SPEC 仍是 2026-09-20「推迟，未做」的原文 |
| ADR-0080 | 号**空着**，下次做 PDF 仍用它；`docs/DECISIONS.md` 里 ADR-0081 的那句「same milestone as ADR-0080's」已改成不引用编号 |
| ADR-0081（bookmark 撤回）、T4.3 的 ⋮⋮ 菜单 re-anchor / 表格 Ctrl+L / `rc_gates.ps1` / verify 脚本删除守卫 | **已落地**，`CHANGELOG` 三条 Known-limitations 与 `ROADMAP` M11 同步改过 |
| T2（mention / @date / backlinks / synced block，ADR-0050…0052）与 T3（chart/formula/D3…D8） | **已落地**；提交时顺手补了三处纯查找错误（`AppWindow.slint` 重复的 `db-formula-open`、chart 块把自身条件元素的几何属性写成 `root.*`、`SlashRow.hint` 少了 `.into()`） |

门槛只跑了 `cargo check --all-targets` + `cargo test`（476 passed / 0 failed，在 detached worktree
`.scratch/landnote/probe` 里用与提交逐字节相同的副本验的）。**release / sweep / installer 都没跑**，
所以 `docs/REPORT_TRACK4.md` 里那两张 ROADMAP snapshot 草稿仍只是草稿，RC 证据要等收敛 head 重跑
`benchmarks/scripts/rc_gates.ps1 -Baseline .scratch/sweep34`。像素基线自 `sweep40` 之后没再更新，
新增的 `synced` / `move-to-tall` / dark 场景**从没出过图**。
