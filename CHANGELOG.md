# Changelog

## 构建 (unreleased)

### 存储

- **桌面端其实一直在保存不了任何东西** (ADR-0142). 真实库缺 FTS 索引表
  (`search_pages`), 而 `apply` 把搜索镜像和被索引的行放在**同一个事务**里,
  所以每一批写入都以 `no such table: search_pages` 失败并整体回滚。笔记、
  任务、窗口尺寸、主题、设备 id —— 没有一样存得进去, 而库照常打开、
  `integrity_check` 照样通过、版本号照样是"最新"
- **`sync.device-id` 每次启动重新铸造只是症状.** 它是一个 settings 行; 写不进去
  就每次启动都是新身份, 手机端于是把同一台电脑记成 5 台新设备, 报
  "the address moved to another device" —— 唯一能看到的现象, 却指向了错的机器
- **写失败不再静默** (ADR-0142). `take_last_error` 此前在整个桌面端**没有任何一处
  调用**, `persistence_force_flush` 的注释说错误会浮现, 但那句话一直是假的。现在
  防抖 flush 每次尝试后都会读取它并写进通知栏; 同一个库的 5 份备份
  (13:23–22:44) 内容逐字节相同, 就是它静默了九个小时的样子
- 索引表由 `quire-core` ADR-0006 修复: 每次打开都校验并回放可重放的迁移步,
  修不好就**拒绝打开**, 而不是把一个会拒绝所有写入的文件交出去
- `state.rs` 新增 `persistence_pending()` / `persistence_last_error()`;
  `persistence_test.rs` 补上此前**完全没有覆盖**的
  `record_setting` → `record` → 防抖 → 落库 整条路径的测试

### 单实例

- **桌面端现在是单实例的** (ADR-0136). 重复启动同一版本会弹窗提示并退出本次启动,
  不会再出现两个窗口抢同一个数据库
- **新版本会替换旧版本.** 启动的版本比正在运行的版本新时, 旧版本通过它自己的退出
  通道 (ADR-0105) 优雅退出 — 落盘、写下 clean-exit 记录、释放 exe 文件 — 然后新版本
  继续启动。这正是 `just deploy-workshop` 一直在手动做的事, 现在程序自己会做
- **旧版本不会顶掉新版本.** 正在运行的版本更新时, 本次启动弹窗说明并退出
- 判定发生在日志、数据库和窗口**之前**: 会被劝退的启动不会先打开数据库

### 笔记

- **单击笔记卡片只选中, 不再弹出笔记页** (ADR-0137). 笔记页 (标题 / 正文 / 评论) 现在
  只能通过右键 → 打开 进入, 详细信息 通过右键 → 详细信息。之前每点一张卡片就把它
  盖在列表上, 列表没法浏览
- **详细信息不再藏在正文下面**, 而是右键菜单里独立的一项 — 它以前够不着, 因为它在
  浮层里要往下滚才看得到
- **标签筛选搬回右侧栏** (ADR-0138). 笔记页的标签列回来了: 全部笔记、标签树、每个
  标签的计数、反向筛选、清除筛选。顶栏那一行只留 搜索笔记 / 排序 / 回收站。标签是
  一棵树, 而 chip 条只能横向滚动 — 滚掉一半标签的筛选器不是筛选器
- 标签列的选中行改成中性抬起的底色 + 2px 强调色竖条, 不再是一整块蓝底
- **新建笔记输入框会在离开笔记页时销毁**. 之前切到 任务 / 文档 / 同步 再回来,
  撰写层还带着没写完的草稿原样弹出。输入框属于打开它的那一页: 现在切标签、去
  文档、去同步、前进后退都会丢弃它 — 与 ✕ / Esc / scrim 同一条规则 (ADR-0115:
  ➤ 之前什么都不写入, 所以丢弃是免费的)
- **自动弹出输入框只在启动时生效**. `auto_input_on_start` 名字里的 start 本来就
  是这个意思: 会话从笔记页启动时照旧弹出, 但侧栏 / 指令面板的到达不再自己弹 —
  要输入框的门是 ＋ 和 Alt+N。设置里的开关文案同步改为「启动时自动弹出新建笔记
  输入框」

### 窗口

- **标题栏上最大化/最小化旁边的三个点没有了**. 页面自己的动词住在拥有它的那一行上 ——
  侧栏的行有自己的 ⋯ 和自己的右键菜单 —— 窗口级再开一扇通往同一个菜单的门, 只是多一个
  地方让菜单的锚点和它说的那一行对不上
- **窗口记住了它自己的矩形, 而不只是大小** (ADR-0143). 位置 (`window.x` / `window.y`) 现在
  和大小一起存, 所以把窗口拖到副屏的��户, 下次启动还在那块屏上
- **关窗时如果窗口是最大化的, 存的是最大化之前那个矩形**. 最大化中的窗口报的是**显示器**
  的大小, 以前那一版把这个当成窗口大小存了下来 —— 于是在大屏副屏上最大化着关窗, 下次启动
  就得到一个跟屏幕一样大的窗口, 而且因为位置从来不存, 它还一定开在主屏上
- **启动时保存的矩形要先问过显示器才生效** (`platform::monitors::place`). 完整落在某块屏
  的工作区里就一个像素都不动; 挂边了就挪进来; 比落点那块屏还大就缩到工作区 —— 窗口底部
  (＋ 添加任务那一行) 掉到桌面外面, 而能把它拖回来的标题栏也跟着在外面了; 存它的那块屏
  已经拔掉了就居中放到主屏, 尺寸能放下就保留 —— 喜欢 4K 上 1600 宽的人, 在 1080p 主屏上
  该拿回的是那个 1600 宽的窗口, 不是一个 1080 的
- **最大化按钮会说它要去哪**: 最大化时画「还原」, 不再是同一枚「最大化」让人试
- **三处平台行为是查出来的, 不是猜的** (winit 0.30.13 / Slint 1.18 源码): 最大化是真正的
  `ShowWindow(SW_MAXIMIZE)`, 而 winit 给无边框窗口保留 `WS_CAPTION`、在 `WM_NCCALCSIZE`
  里伪造边框, 所以 Windows 仍然给任务栏留位 —— 最大化不会盖住任务栏, 自绘标题栏落在工作区
  里; 最小化后再还原**保持**最大化状态 (Slint 后端在最小化期间拒写最大化标志, 因为那时
  `winit::Window::is_maximized()` 报 `false`); 跨不同 DPI 的显示器拖动时, `WM_DPICHANGED`
  不缩放最大化的窗口 (`allow_resize = !MAXIMIZED`)
- **尺寸仍然存物理像素, 没有改成逻辑像素**. 改的话跨 DPI 恢复会更准, 但缩放本身就是窗口的
  scale factor, ADR-0095 的整条结论是"窗口的物理尺寸不随缩放变" —— 为解决一个它并没有的
  问题去推翻那条决定不值得。跨 DPI 那一格由夹紧兜住: 放不下就缩到落点那块屏, 是有界的损失,
  不是掉到桌面外
- **升级后第一次启动会忘记窗口位置**: 只有 `window.w` / `window.h`、没有位置行的旧记录读作
  "没存过矩形", 走平台默认位置开。拿一个编造出来的原点去配上存下来的大小, 正是这条记录要
  拦的那个状态
- **没在双屏机器上验证过**: 规则本身有单元测试 (`platform::monitors`), 平台行为是读源码
  读出来的, 但这里没有第二台物理显示器

### 任务

- **拖一张卡进另一列, 现在看得见它会落在哪**. 指针所在的那条缝张开一个落位框
  (150 ms 展开, 和参考版同速), 落下的那张卡随后带一圈强调色落位环, 620 ms 里淡到
  透明 —— 看板会在卡片换列的瞬间整体重排, 这圈环是唯一还指着"刚才动的是哪张"的东西
- **拖拽中原卡变暗到 32%**: 指针带走的是这张卡的意思, 留在原位的那个不该装作还是它
- **拖回原列被拒绝, 且立刻熄灭**: 一列的顺序就是排序的结果, 拖回本列没有可说的话;
  早先亮着的那一列不会留在原地声称一个不会发生的落位
- **两处渲染器的实话**: 落位框是实线而非参考版的虚线 `{4,3}` —— `Path` 在
  Slint 1.18 没有 dash 数组; 卡片也不会真的被撑开让位 —— `ListView` 委托的高度
  喂给列表布局, 布局又喂回每个委托的 `absolute-position`, 而"指针在卡的上半还是
  下半"只有 `absolute-position` 知道, 渲染器对这个环直接报 "Recursion detected"。
  于是卡片守住自己的座位, 落位框骑在参考版 `dropIndexAt` 会选中的那条缝上

### 设置

- **设置里的文字可以复制了** (ADR-0135). 版本、渲染器、数据文件夹、快捷键、开关
  说明 —— 全部是可选中的只读输入框, 选中后 Ctrl+C。数据文件夹改成换行显示, 以前是
  截断的且无法选中

### 同步

- **桌面端同步更激进** (ADR-0140). 默认间隔从 60 秒改为 10 秒, 预设加回 10 秒档 —
  Android 端一直有的那一档 (10 秒 / 1 分钟 / 5 分钟 / 30 分钟) — 下限从 15 秒降到
  10 秒。桌面一直开着、一直插着电, 一个回合每台设备只花几毫秒, 等才是用户能感到的
- **笔记页的刷新按钮不再隐身** (ADR-0140). 之前只在有已配对设备时才画, 一个
  没有设备可拨的回合是安全的空回合 — 在网络安静时隐藏的刷新按钮读起来就是没有
  刷新按钮
- **10 秒 那档现在是它字面上的意思** (ADR-0141). 同一条间隔被钳了两次, 两个数字:
  store 那层钳 10, 而点按钮走的回调钳 15 — 所以选了「10 秒」存下来的是 15。桌面端
  一直激进, 不需要像 Android 端那样加 Wi-Fi 判断: 没有可省流量的链路, 而「没在局域
  网上」这件事 peer 列表已经答了 (60 秒没广播的设备根本不会被拨)。所以在家庭局域网
  上, 两端跑的是同一个 10 秒, 只是走的路不同
- **废弃设备会自动淘汰**. 设备表里的行只进不出: 重装过、换过机器或早已不存在的设备
  每台永远占一行, 只能逐行点「忘记」。现在 30 天没有上线的设备在打开同步页或听到
  任何广播时自动退场, 日志里记一笔; 标题栏还有「淘汰离线设备」按钮, 一次清掉所有
  灰点的行 (按钮只在有灰点时出现, 计数就是它要扫的行数)
- **淘汰保留合并记忆**. 淘汰只删 peers 表的行, 保留 `sync.shadow.*`: 删除要靠 shadow
  才能传播 — 快照里缺的行只有对着 shadow 才读作"对方删了", 对着空 shadow 读作
  "新行", 把 shadow 一起删会让离开期间删掉的笔记随设备回来。被淘汰的设备若再次
  广播, 下一回合从上次的共识继续; 「忘记」仍是行和记忆一起删的那个动词
- **`last_seen` 现在按天落盘**. durable 形式此前把 last_seen 归零 (避免每四秒一次
  SQLite 事务), 代价是重启后所有行都读作"从未", 淘汰无从谈起。现在 durable 形式
  保留量化到天的 last_seen: 每台活跃设备每天最多多一次持久写, 换来「最后在线」
  跨重启仍然有意义、30 天淘汰有了可以 aging 的依据。旧版本留下的 last_seen=0 的行
  不会被自动淘汰 (无法 aging), 由「淘汰离线设备」清掉

### 部署

- **`just deploy-workshop` 只留版本归档** (ADR-0139). C:\workshop 现在每个版本一个
  `quire-desktop-<version>` 文件夹; 会被每个版本覆盖、因此需要先请运行中的实例退出的
  裸 `quire-desktop` 安装路径没有了。版本文件夹每次都是新的, 没有什么被覆盖, 也没有
  运行中的实例挡路

### 构建

- **`target` was 71 GB; it is 7.6 GB** (ADR-0134). Nothing was wrong with the code
  — cargo was keeping every build unit it had ever made for this crate, and with
  a ~1 GB library. One sweep reclaimed it, and the machine went from 104 GB free
  to 161 GB
- **`just sweep` — clean the caches, keep incrementality.** `cargo clean` takes
  the release tree with it and the next deploy pays a cold `codegen-units = 1`
  build. `sweep` deletes only `incremental/` and `build/`, the two directories
  keyed on unit identity that grow without bound; the next build links instead
  of recompiling
- **`just size` — where the disk went.** The tree largest-first, with each
  directory's file count and newest date, so a repeat visit is comparable
- **`just shot` builds into its own `target-shot`.** It needs `--features
  software` while the app needs the default `femtovg`, and two feature sets are
  two compilation units to cargo — so alternating between the two minted a new
  ~900 MB library every time and kept the old one (26 copies, 63 GB). Separate
  directories stop the churn as well as the growth
- **`[profile.dev]` sets `debug = 1`.** Line tables only: a backtrace's function
  names survive, which is how UI work here is diagnosed, and the artifacts are
  far smaller
- **`$CARGO_HOME`'s 2.8 GB registry cache is now auto-cleaned**
  (`cache.auto-clean-frequency`, stable since Rust 1.88)

## 0.1.0 — Windows RC (in progress)

First functional release: a local, single-file-database notes workspace.

### 同步 (a page of its own)
- **同步 is now a destination, not a settings section** (ADR-0132). It has its own
  row in the sidebar between 任务 and 搜索, and three boxes — 设备 / 统计 / 日志 —
  instead of two lines per device inside a 400 px popup that had to scroll. The
  reference app makes it a page too, and a device table, its statistics and its log
  are things you go to *look at*
- **The device table carries what the row used to hide.** 设备 / 类型 / IP:端口 /
  最后在线 / 最后同步, with an online dot and the two verbs (立即同步 / 忘记) per row.
  立即同步 is disabled for a device that is not on the network, because dialling
  one is the round that times out
- **统计 says what a round moves.** What this device carries (笔记 / 任务 / 清单 /
  页面 / 附件 / 总计) and, per device, 对端 … 本机 … 变化 … — the 变化 count is a
  delta against the stored shadow, keyed by the 唯一 ID rather than the row number,
  because a row number is per-device and would report every note as changed
- **⟳ on the 同步 page re-reads the device list**, and 立即同步全部 starts a round
  with every device on the network. They are separate buttons because they answer
  different questions: "is anything there?" and "go and get it"
- **The log is the whole record, not the last twelve lines**, and it has a
  清空日志. A popup had no room for more; a page scrolls
- **Three doors, one round.** The 笔记 list's 刷新, the page's 立即同步全部 and the
  同步 row's right-click all start the same round through one function, so they
  cannot become three different rounds. A device that is stored but not answering
  is still named in the status line rather than silently skipped
- 配对 stays gone (ADR-0128): a device is on this list because it announced
  itself, and the 按 IP 添加 door stays gone for the same reason. No 安全码 column
  either — the shadow is encrypted with the device key and this shell has nothing
  to show you that you could check against the other screen

### 笔记 / 任务 (organizer)
- **The 笔记 list is measured in ActivityWatch's pixels** (ADR-0133). The list
  insets are 20/20/16/16, a card sits in a 5-above/6-below gutter, and its padding
  is 10/10/14/10 — the reference's own numbers from `inboxpage.cpp`, which is what
  makes a card read as a card in a column rather than as a row with a border drawn
  round it. The search field is 240 px, and the age line and the body are 6 px
  apart
- **A sort box on 笔记** (最新创建 / 最新更新 / 按内容), the reference's three orders.
  It is separate from 任务's sort rather than a fourth choice in it, because the
  two halves' orders have nothing in common. 置顶 is not a sort mode — a pinned
  note stays at the top under all three
- **刷新 is now a ⟳ button** instead of a word, matching the reference's 30 px box
- **An empty 笔记 list explains itself** — 还没有笔记 / 点击右下角 ＋ 新建一条 — and
  says something different when a filter matched nothing, so a search that found
  no rows does not tell you that you have no notes
- **配对 is gone: a device on this LAN is a device we sync with** (ADR-0128).
  Open a second copy of Quire on the same network and the two find each other and
  start syncing — no 配对 button, no 发起配对, no 发起配对 row. The 同步 section is
  now one list of 本网络上的设备, and the row's status line says when the last
  round was rather than repeating 已配对 on every line. A push from an address this
  device has never heard is still refused
- **The 笔记 list is a set of cards, not a set of rows.** Each note is now a
  rounded plate with a border and its own surface, matching what the Android shell
  has always drawn. The 6 px gap is the list's own spacing, so a card's height
  still means "what it draws". Task rows are unchanged
- **「全部笔记」 no longer shouts.** The selected filter chip was a blue plate with
  a blue border and blue text on that plate, and it is lit by default; it is now a
  neutral raised surface with a plain border. The accent colour goes back to
  meaning "this needs attention now"
- **The refresh button follows the same rule**: shown only when there is a device
  actually on the network to sync with, and it names the ones it had to skip
- **刷新 in both tabs: one round with every device on the network, right now**
  (ADR-0127). It is a sync round, not a reload of the local list — a list that is
  correct but missing a note from another device looks identical to one that was
  never refreshed, and the round is the only thing that makes it complete
- **A round that lands now redraws 笔记 and 任务.** It previously redrew pages,
  blocks and databases but not the organizer, so merging three new notes left the
  tabs drawing the list as it was before the pull — which is why the 同步
  section's 立即同步 button worked and then showed you nothing

### 修复
- **The line reading 「Android 未能把写入的更改落盘」 was this machine, not the
  phone** (core ADR-0005). A sync round makes the receiving side flush its own
  write queue, and the flush could lose a race against the app's own background
  writer and fail — with no `busy_timeout` set, a collision failed *immediately*
  rather than waiting its turn. The core now waits (5 s, SQLite's own default for
  a busy handler), and the message says 本机 so the device named beside it is not
  blamed for it

### Editor
- Block editor: paragraphs, headings 1–3, bullet / numbered / to-do lists,
  quotes, code blocks, dividers
- One focused block owns the live text input; everything else renders
  lightweight (10 000-block pages stay flat in memory and rendering)
- Enter / Backspace merge & split semantics, empty list items leave the
  list on Enter or Backspace
- List nesting: Tab / Shift+Tab (depth 1), drag-free reordering via the
  block menu or Ctrl+Shift+↑/↓, Ctrl+D duplicates a block
- Drag the block handle (⋮⋮) to reorder: an accent line marks the landing
  spot and the drop commits one undo-able move; landings that would split a
  nested list away from its parent are rejected
- Markdown line-shortcuts: type "# ", "## ", "### ", "- ", "* ", "1. ",
  "[] ", "[x] ", "> ", "---", "```" or "$$ " at a block start to convert it as
  you type (one undo step)
- Callout block: a tinted rounded box with an emoji and text (slash menu
  "Callout"; turns into Text/Code/Divider like the other kinds)
- Block menu (⋮⋮) — Notion's set, minus the collab/AI items that stay out
  of v1: Turn into (Text / Callout / Code / Divider), Duplicate, Copy link
  to block (puts a quire://block anchor on the system clipboard), Move to
  (every other page; the whole subtree crosses in one undo step), Text
  color and Background color (live-picking palette with swatches), Move
  up/down, Copy block, Paste below, Delete. Clicking a quire://block or
  quire://page link jumps inside the app. The slash menu and Turn-into
  list only carry kinds without a symbol shortcut (ADR-0022)
- "+" handle opens Notion's insert menu: it creates the empty line below
  and shows the full block list (Text, Page, Link to page, To-do, Headings,
  Bulleted / Numbered, Quote, Divider, Callout, Code, Toggle list, Image, File,
  Table, Columns, Math, Table of contents, Embed, Table view, Synced block,
  Linked view) — picking a
  row converts the new line, clicking away or Escape keeps the empty line,
  typing filters the menu. The five remaining database rows (Board, Gallery,
  List, Calendar, Timeline) are still muted "later" placeholders: they name
  layouts, and a layout is a view of a database you already have — see
  Databases below
- Toggle list: a collapsible section. The chevron folds its whole subtree
  out of existence — the hidden blocks get no rows at all, so they cannot
  be tabbed into, dragged or renumbered — and the fold is a view setting,
  not content, so it survives restarts and never bumps the document.
  Turning a Toggle into another kind re-opens it rather than stranding its
  children; Markdown export degrades a toggle to a quote line
- Image block: pick a file from the insert menu, the slash menu or Turn
  into — or press Ctrl+V with a screenshot on the clipboard — and the
  picture lands in the page. The bytes go to an
  `attachments` folder beside the database (the app keeps only a
  reference), oversized pictures get a downscaled display copy while the
  original stays untouched, and the ⋮⋮ menu's "Image width" sets the row
  to 25 / 50 / 100 % of the column. Click a picture to view it full-width
  behind a scrim; clicking anywhere closes it. Undo removes the reference,
  never the file. The picker offers png, jpg, bmp and gif — the four
  formats the decoder is asked for; a gif becomes a still, its first frame
- File block: any file at all, from the same three doors as a picture. The
  row shows the name with its extension, the size, and two buttons — Open
  hands the bytes to whatever the system has registered for that type,
  Save-as copies them out under their original name. The file is streamed
  straight into the `attachments` folder and never read into the app, so a
  2 GB attachment costs the same as a 2 KB one until you press one of those
  buttons. Undo removes the reference, never the file
- Table block: a simple N×M grid, from the insert menu, the slash menu or
  Turn into. Tab moves through the cells and Shift+Tab back; tabbing past
  the last cell adds a row, so the table grows as far as the typing goes.
  Hovering the grid reveals its toolbar — Add row, Add column, Delete row,
  Delete column — where adding goes after the row or column the caret sits
  in and deleting takes it, and a table never shrinks below 1×1. Cells hold
  text with the same inline marks as anywhere else (Ctrl+B / Ctrl+I /
  Ctrl+E / Ctrl+Shift+X / Ctrl+M work inside a cell). Turning a line into a table
  keeps its words in the top-left cell, and turning a table back into text
  gives every cell back as its own paragraph; Markdown export writes a
  GitHub-flavoured table (import still reads those lines as text). This is
  a grid, not a database — schema, filters, sorts and views are a later
  milestone
- Columns block: a line becomes a side-by-side layout, from the insert menu, the
  slash menu or Turn into. Two boxes to start, three at most, and hovering the
  layout reveals its strip — Add column, Delete column — where a box is added at
  the right end with a line inside it and a deleted box's words move into the
  box before. The line's own words open the first box, so nothing is lost by
  converting a line; the layout itself holds no text of its own. Tab walks the
  lines inside the layout and stops at either end (you leave a box by clicking
  out), and an empty box says "Empty column" until you click it, which gives it
  its first line. Layouts are ordinary blocks all the way down — a box is a
  child block and so is every line in it — so undo covers every layout edit and
  no new storage is needed. Markdown export flattens a layout into page-level
  paragraphs, keeping the words and the reading order
- Math block: a formula on its own line, from the insert menu, the slash menu,
  Turn into, or by typing "$$ ". It stores what you type — a LaTeX subset as
  source — and paints the nearest Unicode reading of it: `\frac{a+b}{2}` becomes
  one-line `(a+b)/2`, `\sqrt{ab}` becomes `√(ab)`, `x^2_i` becomes `x²ᵢ` wherever
  the alphabet has the glyph. Nothing is thrown away: a command it does not know
  comes back as its own source, so the worst case reads "that did not render"
  rather than "that vanished". Click the block and the source is what you edit.
  Markdown writes and reads a `$$ … $$` fence, verbatim inside like a code fence
- Contents block: a page's own table of contents, from the insert menu, the slash
  menu or Turn into. The list is not stored — every line is read off the page each
  time the page is projected — so renaming a heading renames its line, deleting one
  removes it, and a heading a folded toggle hides leaves the list until the toggle
  opens again. Lines indent by heading level, an untitled heading reads "Untitled",
  and clicking a line puts the caret at the end of that heading. The block has no
  text of its own; converting a line into one keeps that line's words stored but
  unpainted, the way a divider does, so turning it back gives them back. Markdown
  writes and reads a single `<!-- quire:toc -->` marker line
- Embed block: a link shown as a card, from the insert menu, the slash menu or
  Turn into. The block stores the address and nothing else — the card's headline
  (YouTube, Figma, Google Maps, GitHub, or the host itself for a site the app has
  never heard of) and the address line under it are both read off that text as it
  paints, so there is no second copy of the link to go out of step. The arrow
  hands the address to your browser; nothing is fetched, no page is embedded and
  no favicon is downloaded, which is what keeps a card cheaper than the iframe it
  stands in for. Click the card and the address is what you edit — the headline
  changes as you type it, and an empty card says "Embed / No address yet" rather
  than showing a blank box. Markdown writes the bare address on its own line, so
  the file reads as a link in any other renderer; import turns a line back into a
  card only when it is one address and nothing else, so a sentence that happens to
  contain a url stays a sentence
- Fixed a marked line with room to spare painting its runs apart. A paragraph's
  inline marks render as side-by-side runs, and the row laid them out with
  Slint's default `alignment: stretch`, so any leftover width was divided among
  the runs as gaps — ~155 px between three of them on the first short formula
  line the sweep had ever shot. Every marked line in the then-44-scene baseline
  overflowed its frame instead, so nothing showed it before. The fix is on all
  three run rows — block, table cell, column line — and moved none of them
- A paragraph carrying inline marks now wraps like a plain one. This was the
  documented platform wall: Slint `Text` has no inline formatting, so marks paint
  as separate runs, a run was one whole stretch of text, and a layout cell cannot
  break — a marked paragraph ran off the edge and was clipped mid-word while the
  identical unmarked paragraph wrapped. `build_runs` now cuts every *unmarked*
  stretch to one word per run and the row lays the runs out with a wrapping
  `FlexboxLayout`, so the breaks land where the words do; marked stretches stay
  one cell, because an underline, a code box or a link's click target split at
  every space is worse than the bold phrase it still cannot break (ADR-0041).
  What it costs, measured rather than assumed: ≈2.8 ms per projection of a
  10 000-row page whose every tenth line is marked (and nothing on an unmarked
  page), 1.031× the control's memory on that same page, and ≈3 px of extra line
  spread on a marked line whose words were already fitting
- Fixed the page title sitting below where it is bound. The row's title band
  sets its height but used to leave `y` alone, and Slint centres such a child
  vertically in its parent — harmless while every first block was one line
  tall, visible as soon as the first block was a table, whose grid then
  painted over the title. The offset was ~34 px on a paragraph page and
  ~60 px on a table page; the band now states its `y`
- Page block: embeds a child page (insert menu "Page"). The row shows a
  page icon and the child's live title (renames propagate), clicking it
  opens the child, deleting the block deletes the child page, and
  duplicating copies the child so the two blocks never share a target.
  Exports as a `quire://page` link that re-imports clickable
- Link-to-page block (insert menu "Link to page"): points at any existing
  page through a filterable picker; the target is not owned — deleting
  the block, duplicating, or pasting it never touches the page
- Rich paste: pasting markdown with block structure (headings, lists,
  to-dos, quotes, code) splits it into real blocks with inline marks;
  plain text still pastes natively at the caret
- Ctrl+V pastes a screenshot: when the clipboard carries no text but holds a
  bitmap (`CF_DIBV5`/`CF_DIB` — what Snip-and-Sketch and PrintScreen write),
  it becomes an Image block stored like any other attachment. An empty block
  *becomes* the picture; a block with words in it gets the picture below, so a
  paste never leaves a stray empty line. One undo step. When a copy carries
  both text and a picture, the words win
- Every popup (page menus, ⋮⋮ menu, slash menu, command palette, search)
  dismisses on a click outside it and on Escape; UI state follows so
  nothing stays blocked behind an already-closed menu
- Modals — the delete confirm, Settings, the Add-link card — now dim the window
  behind them. Anchored popups deliberately still do not: a menu that asks you
  to pick a type for one line should not hide that line
- The "/" block menu now measures its own height when it picks where to sit, so
  a long list stays inside the window instead of running off the bottom edge as
  soon as another kind is added to it
- Fixed the handle (+/⋮⋮) being clickable while invisible on the block
  being edited: it now shows whenever the row or the buttons are hovered,
  editing or not
- Inline marks: bold (Ctrl+B), italic (Ctrl+I), inline code (Ctrl+E),
  strikethrough (Ctrl+Shift+X), links (Ctrl+L + dialog; click a link to
  open it — internal quire:// links navigate in-app), inline math (Ctrl+M over
  a selection: the same LaTeX subset as a math block, rendered inside the
  sentence; Markdown writes it as `$…$`, and a sentence that merely mentions
  prices — "costs $5 and $10" — stays prose)
- Toggle the sidebar with Ctrl+\ — it had been Ctrl+B, which is bold, so the
  same chord was labelled two different things in two different places
- A brand-new page can be written in: click the "This page is empty" panel, or
  press Enter in the title, and the page makes its own first paragraph, focused
  and one undo step away from gone (ADR-0033)
- Code blocks can be coloured: the block's own ⋮ → Language menu picks the
  language (Rust / Python / JavaScript / TypeScript / Markdown / JSON / Bash) and
  the block paints five token colours over its own text — keyword, comment,
  string, number, name. Only the language is stored; the colours are derived while
  drawing, so a keystroke never lexes and a block being edited shows plain text
  (ADR-0042). A Markdown fence's info string carries it both ways (`rs` in,
  `rust` out), a rich paste keeps it, and a language this build cannot lex is no
  colour rather than a broken block
- Per-page find bar (Ctrl+F) with hit counter and selection navigation. Every
  match is now painted, not just counted: a hit is one word-run cell with a pale
  fill and an amber border, so the box says where the text is without taking the
  block's own colour below what the palette already allows. A hit that lands in
  the middle of a word splits the word, one that lands inside bold or a formula
  tints the whole mark, and one inside a table cell or a column box rides on the
  row that draws it. Quote and callout hold their text in a `Text` of their own
  and now share that flexbox, so the counter and the page reconcile: on the swept
  page 16 hits are 14 boxes plus the 2 in the block the bar stepped into, which
  shows the editor's selection instead (ADR-0043)

- Undo/redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z) — per page, command-based

### References
- Typing `@` in text opens the picker the slash menu already uses: its first row
  is a date, the rest are the workspace's pages. Applying one replaces the `@`
  and the filter words typed after it with a chip in a single undo step
- A mention stores the page's id and never its title, so renaming a page stays
  one write and no reference has to be re-printed: the chip, the backlink
  panel's group headers and Markdown export all read whatever that page is
  called right now. A page that is gone reads `(deleted page)` and greys out;
  clicking a live chip jumps to the page
- `@date` is a chip of its own, with a clock instead of a page mark: it stores
  the ISO day it was set to rather than "today's number", so it does not drift
  and only one spelling is ever written
- Backlinks: the bottom of every page lists the blocks that point at it, grouped
  by source page — five rows collapsed, fifty expanded, and one line that states
  the total so a page cited 200 times never pushes the text off screen. It is
  derived, not stored (no table, no column), and two indexes keep one page open
  at 78 µs instead of a full-library scan. Clicking a row jumps to the source
  block, opening its page first when it lives elsewhere
- Synced block: a second view of another block, from the insert menu or the
  slash menu. The copy holds no text of its own — it draws its source's line, and
  typing into either one writes that single content, undone by one Ctrl+Z.
  Deleting a mirror takes only its row; deleting the source leaves the mirrors
  visible, read-only and saying `(deleted source)`. A pair that would point at
  each other is refused at the moment you write it, not discovered while
  drawing. Markdown writes a mirror as its source's line, and import
  deliberately does not learn the syntax — a block id from another file would be
  a reference born broken

### Databases
- A database is one block with a tab strip: eight layouts over the same records
  — table, board, list, calendar, gallery, timeline, form, chart — where "+"
  adds a second view and each view keeps its own filters, sorts, grouping and
  layout. The insert menu's other five layout names stay muted on purpose,
  because a layout is a property of a view rather than a second kind of block:
  the way to a board is a tab, not a new block
- Seventeen property kinds: title, text, number, select, multi-select, status,
  date, checkbox, url, email, phone, files, created time, last edited time,
  formula, rollup, relation. The Columns popup shows and hides columns and, on
  its second panel, changes what a column is; created and last-edited stamps
  come from the record itself and are never typed. A cell with no value stores
  nothing at all, so "empty" means one thing in every screen
- Filter, sort and group are questions asked of the database, not a pass over
  rows already in memory: the operators offered depend on the column's type, a
  rule whose value you have not filled in constrains nothing, and a date sorts
  as text because its format is fixed width
- The search box is one needle over a record's title and text, and the header's
  "N rows" answers to it — the count comes from SQL before the window opens
- Formula: an expression sheet with a real lexer and interpreter (no script
  engine, no new dependency). `if / length / round / abs / min / max / text`,
  four budgets that cap tokens, depth, steps and result size so nothing can run
  away, and a cycle is refused when you save rather than hanging the draw. A
  formula's value is computed as the window is projected and never stored —
  which is also why sorting or filtering a formula column says no: that would
  mean evaluating the whole column first
- Rollup: three questions in order — through which relation, which column, what
  fold (count / sum / min / max / average, or none). Its chooser lists only what
  the save path would accept, so a column that cannot be folded reads greyed
  there rather than refusing afterwards
- Relation: a column points at another database, optionally declaring the
  back-pointer on the far side, and its cell is picked from a list with its own
  search box and a tick on what the cell already holds. One pick writes the cell
  and every mirror it implies as a single undo step, and the pairing rule is what
  makes a relation cycle unrepresentable instead of detected
- Linked view: a second block that draws a database which already exists, so one
  table can sit on two pages without the records being copied
- Each row's hover slot carries two actions: delete (record, its values and its
  page go together in one Ctrl+Z) and "T", which makes that row the database's
  template — every later "New row" arrives prefilled from it in one undo step,
  and a column the schema has since lost simply does not prefill
- Chart is the eighth layout and it plots aggregates rather than rows: one
  grouping query feeds bar / line / pie drawn with the primitives already in the
  app (no chart library), realizing zero rows however large the database is
- Ten thousand rows are never all present: the count is computed first, then a
  window, so only the rows on screen exist as objects (kilobytes rather than
  megabytes) and scrolling inside a window that has not changed recomputes
  nothing

### Notes & tasks (SPEC §四十一)
- A **second top-level area**, beside the document editor rather than inside it:
  「笔记」and「任务」rows in the sidebar, two palette commands, and
  `active-area` decides which of the two `AppShell` mounts (so a keystroke can
  only reach what is on screen). Opening a page — sidebar, palette, backlink,
  `Alt+←` — is also *leaving* the area, in one line
- **Notes**: title, body, pin, tags, and a list that reads by first line and
  relative age, newest edit first with the pinned ones above.
- **Tasks**: done, due date, priority, list, tags, notes, a checklist and a
  repeat rule. Checkbox / priority / list / repeat / due commit immediately;
  title, body, notes, tags and a checklist line commit on the same 300 ms
  debounce the prose rows and database cells use, so one burst of typing is one
  Ctrl+Z rather than one per character. A row that did not change is not a step
  at all — and does not move its `edited` stamp
- **One selector, two halves**: 收集箱 / 今天 / 近七天 / 全部 / 已完成 are
  predicates over the catalog, and the list chips are its other half. Three sort
  orders (added / priority / due), all derived — nothing about which one is
  showing reaches the file (ADR-0073), so a restart opens on 笔记 · 收集箱
- **The area has its own undo stack** (ADR-0099): Ctrl+Z inside it walks the
  organizer's entries, Ctrl+Z in the editor walks the open page's, and neither
  can reach the other's. Deleting a list files its tasks in the inbox in the same
  step, so one Ctrl+Z brings back the list *and* its tasks (ADR-0101). A row
  delete no longer lands there at all — see the undo bar below (ADR-0108)
- Three tables in quire-core (schema v24–v26: `notes`, `task_lists`, `tasks`),
  nine row-level changes, nine commands, and the same LAN sync as everything
  else — **the snapshot version moves to 2** for it, so both shells must be
  updated together (ADR-0102)
- Seven scenes plus their dark arms (`notes`, `notes-detail`, `notes-search`,
  `tasks`, `tasks-detail`, `tasks-list`, `tasks-overdue`), all in the sweep list
  and all planted through the real write path
- **The area is one card with three columns** (ADR-0104), the way the reference
  this page was modelled on draws it: a **nav column** (208 px) of view rows and
  lists with their counts and 已完成 pinned at the bottom, the **rows** (52 px,
  round checkbox, tag and list pills, the priority glyph, a due badge, the
  list's dot, and a ⋯ that opens the shared context menu), and the **detail
  panel** (340 px, always there) as a form of boxed choice rows. A footer carries
  the completed switch and 已完成 X / Y. The detail panel no longer slides in from
  zero width, so selecting a row no longer reflows the list
- **平铺**: a kanban board, one column per list, with the inbox as a column like
  any other. Cards drag between columns (the same `DragArea`/`DropArea` pair the
  page tree uses), a column's ＋ line adds straight into it, and the board's
  header counts lists rather than rows
- The 笔记 tab gained the tag column (most-used first) and a header that counts
  notes; a list's header counts its own 项待办, a board's counts 清单
- **A tag is a path** (ADR-0107). `项目/工作` is one tag, the filter keeps the whole
  **subtree** on segment boundaries — `项目` keeps `项目` and `项目/工作` and drops
  `项目2`, which a plain prefix test would keep — and the 笔记 tab's tag column shows
  **one level**: the children of the filter, each carrying the number of notes at or
  under it, counted once per note however many of its tags pass through it. An
  **↑ 上级** row and the breadcrumb above the column are the way back. The Compose
  shell keeps the same three rules in `OrgModel`, so both platforms segment a tag
  identically
- **转为待办**: the note detail's ✓ button turns the note into a 收集箱 task — titled
  by the note's own title or its first line with the opening markdown stripped, its
  body as 备注 and its tags carried — and removes the note, with the notice band as
  the way back. The Compose shell applies the same rule, so a note converted on
  either device makes the same task
- The tag pills on task rows and board cards draw a path as `#项目 / 工作`. The tags
  *input* still edits the flat comma-separated form: `NoteRow.tags` is the display
  line now, so the draft asks the catalog for the raw tags instead
- Two scenes join the sweep: `tasks-board` and `dark-tasks-board`
- **A delete waits behind an undo bar** (ADR-0108). 🗑, the ⋯ menu's 删除 and the note
  转为待办 all *hide* their rows for three seconds instead of writing:
  `org_defer_delete` / `org_undo_pending` / `org_commit_pending(token)` are the whole
  API, and only the third one reaches a command. So 撤销 writes nothing and puts
  nothing on the stack — the Ctrl+Z the notice band used to lean on is not needed,
  and after the bar expires the delete is a step: **one per bar**, since ADR-0111
  routed the commit through `exec_all`, so a batch that came from one gesture goes
  back with one Ctrl+Z. One slot and one timer, so
  a second delete **commits** the first rather than queueing behind it, and the token
  is what makes the superseded timer find nothing to do
- The hiding is in **every projection** — note rows, the tag column, task rows, the
  smart-view counts, the view header, the footer progress, the board, and the list
  chips — so no number on screen ever counts a row the pixels dropped
- The bar itself is `AppShell`'s, not the area's, and it floats bottom-right (380×46,
  24 px in) rather than sitting in the layout: a pending delete outlives the area it
  was asked from, and a band that pushed the three columns around would move the
  whole page for a 380 px pill. A long note title elides; 撤销 keeps its own width
- **转为待办 now makes the task and hands the note to that bar**, so it is four steps
  on the organizer's stack instead of five, and 撤销 on its bar brings the note back
  while the task stays. Closing the app inside the three seconds drops the delete:
  the row simply comes back
- `undo-bar` and `dark-undo-bar` join the sweep, deferred but not timed — a capture
  that waited past the expiry would photograph "deleted", not "pending"
- **The note page gained 详细信息** (ADR-0109): a `▸` disclosure *below* the body with
  six read-only rows — 编号 / 创建 / 修改 / 标签 / 长度 / 置顶 — and the reference's own
  honest line that there is no version history to restore from. Below rather than in a
  sheet because Slint has no sheet and a popup for six rows is a window inside a window,
  and above the body it would shove the field being typed into every time it opened
- Its dates are `2026-09-26 · 刚刚`, not the reference's `yyyy-MM-dd HH:mm`: this shell
  keeps no timezone, so a minute would be either UTC (wrong to the reader) or a
  conversion nobody owns — the same call `core::date::today_iso` makes. Past thirty days
  the age *is* the date, so it prints once
- 长度 counts **characters**, not bytes, and measures the body or the title when there is
  no body. All six values are Rust's text: `org_note_details` is a projection like every
  other number on the screen
- The detail pane's own `修改 刚刚` line is **gone** — 详细信息 owns that fact now, the
  list row already carries the age, and the reference's note page has no such line
- `notes-info` and `dark-notes-info` join the sweep. The first version of the scene arm
  forgot `org_load_drafts` and photographed a page with an empty title, empty tags and an
  empty body beside a correct 详细信息 — the block was right and the scene was lying
- **收件箱 is home** (ADR-0110). The back stack holds `NavStop`s — a page or the
  organizer on a tab — instead of page ids, which is what let `Alt+←` from 笔记 *jump
  over* the area and repaint a document under it. A back step with nothing behind it
  lands on 笔记 and parks the stop being left on the forward stack, so the walk down
  into 收件箱 is reversible and only home itself ends it
- What reaches the file is one meta row: `current-area` beside the `current-page` it
  sits next to, so a library last read in the area opens there. Which tab and which
  filter were open still do not (ADR-0073), and the write rides the persistence funnel
  rather than the undo history — walking into an area is not a step `Ctrl+Z` can take
  back. A meta row is not a schema change: no `user_version` move, no other shell locked out
- `org_open` split into the move and the paint (`org_show`), and `nav_here` is derived
  from the two facts the window paints from, so no path can change the area and leave
  the history pointing somewhere else. A benchmark fixture is not a session: with
  `--blocks` the landing stays on its document, which is where every sweep arm starts
- **多选** (ADR-0111): a picking mode over both halves of the area. The header's 多选
  button, or a task row's ⋯ → 多选 (which enters with that row already picked — what a
  long press means on the reference shells), and once inside a row tap picks instead of
  opening, through the same callbacks, so a pick cannot move the detail pane. The box in
  front of a task row becomes the pick and the note row grew one — an overlay at the row's
  top-right, so the title's line gains 22 px of right padding while picking and its
  ellipsis stops 30 px short of the box instead of running under it. The header is replaced
  by `已选 N 项 · 全选/取消全选 · 复制|完成 · 删除 · ✕`
- The picked ids live in Rust as one set **tagged with the half of the area they belong
  to**, because note 3 and task 3 are both `3`: the reference's single shared set lights
  the *other* kind's rows after a tab switch, and 删除 then acts on rows nobody picked.
  Moving between tabs or into the area ends the mode, so no path leaves a set behind
- **全选 and the count read the projected models**, not a second copy of the filters:
  "what is showing" *is* those two `VecModel`s, which is also why a row a filter hid — or
  a row sitting behind the undo bar — is not lit, not counted, and not in the batch
- **The bar's ✕ was invisible, and is now drawn** (`OrgButton.icon-name` → the `x` icon).
  The button held its slot at the header's right edge with every pixel of it `FFFFFF`:
  `✕` is U+2715, Dingbats, and no face in this app's font chain carries it — the exact
  failure `DatabaseFilterPopup` already documents for its own delete button. It is worse
  here than there because the mode binds no Escape, so an invisible ✕ is an invisible
  *only* door. The sweep's `notes` arm moved by 0 px, which is why the render caught it and
  the code did not. `☑` (U+2611) was checked the same way and **does** draw (ink at
  x 563–577 in `tasks`) — a gap this slice had to close first, since no arm had ever drawn
  a row with subtasks
- **A batch costs one undo step.** `exec_all` plans every command against the same
  pre-state and pushes one history entry, so batch 完成 and the bar's batch delete each
  take one Ctrl+Z to reverse; the reference sends N single-row writes there and needs N.
  `exec_org_all` is the same call on `ORGANIZER_STACK`, so a batch lands on the area's
  stack and not the page's
- **复制 writes nothing at all**: the picked notes as their bodies (a note with no body
  says its title), joined by a blank line, in the order the page drew them
- No core change and no rev bump — `exec_all` and `ORGANIZER_STACK` both exist at the
  pinned rev, which is why this slice ships ahead of the 引用/评论 bump the other two
  shells have to make together. Nothing about the mode reaches the file, the schema or
  the LAN snapshot: a selection is a session fact
- `notes-select`, `tasks-select` and their dark arms join the sweep. Fifteen of the
  seventeen organizer arms moved by exactly the ink of the new header button's two
  characters (188 px light / 185 px dark, in a 22×12 box); the two board arms and the
  other 133 scenes are byte-identical

### Notes & tasks — 标签在右、反向筛选、指令 (ADR-0112)

- **The tag column moved to the right** of the card: the order is now
  `[list] │ [detail] │ [nav]`, with the card's rounded corner following the nav. The
  reference puts the filter on the right, and that is the side a pointer reaches without
  crossing the rows it filters
- **反向筛选**: a tag path can be *hidden* as well as *shown*. Each tag row carries a
  drawn ⊖ control; hidden paths render muted with the control lit, the column prints one
  `排除 #项目 #工作` line, and a `清除筛选` row clears both halves. `AppState.org_excluded`
  is a `BTreeSet<String>` in Rust — the rows *are* the projection — and the two halves
  are kept from contradicting each other: hiding the path 仅显示 names drops the include,
  and picking a hidden path un-hides it
- A hidden path takes its **subtree** and leaves its lookalike: hiding `项目` hides
  `项目/工作` and leaves `项目2` alone — `tag_matches` with the answer turned around
- **The tasks' half filters by tag too**, which it never did: one `OrgTagColumn`
  component under the 清单 section, one include path and one excluded set over whichever
  half is on screen. Switching tabs clears both (a note's path is not a task's)
- **指令**: the door an AI's answer comes back through. `复制` now carries each row's
  **唯一 ID** (`ID: <uuid>`, or `local:<id>` for a row an older peer sent), which is what
  lets a batch name the rows it means — an integer id would name a different row after a
  sync had renumbered it. The dialog takes the same `{"operations":[…]}` payload the
  reference server does, with a **复制示例** button that hands over a worked template
  covering every accepted action
- Actions: `create / update / delete / add_tags / remove_tags / set_tags / comment` for
  notes; those plus `set_completed / move / set_priority / set_due / add_subtask /
  set_subtask / remove_subtask` for tasks. `add_tags` and `remove_tags` are **increments**
  — the AI never has to know what a row already carries. An action the half cannot take
  is refused **by name** (`笔记不支持动作 set_completed`) and counted, never skipped.
  `restore` is refused too: this build's delete is final, and there is no 回收站 to
  restore from
- One batch is **one undo step**, and its operations **see each other**: the parser
  simulates the batch on a copy of the catalog, so `add_tags` followed by `update` keeps
  the tag and `create` followed by an operation naming the new uuid works
- **Replies are real**: `comment` writes a note whose `ref_note` names its target (core
  ADR-0001, which the bumped `rev` brings), the note list and the tag column skip replies,
  and the detail pane shows them under the body as `评论 N`
- 详细信息 gains the **唯一 ID** row, and the pin now bumps `quire-core` to
  `4899857` — the rev that carries both the 唯一 ID (core ADR-0002) and the 引用
- `notes-filter`, `notes-commands` and their dark arms join the sweep

### Notes & tasks — 清单管理、回复框 (ADR-0113)

- **A stored 清单 is managed from its own ⋯** on the 任务 tab's nav row (drawn on hover
  or when the row is the selected one): 重命名 / 颜色 / 删除清单, through the same shared
  context menu the page tree and a task row already open. The 颜色 row carries the
  list's current swatch, and opens the **closed** palette as swatch rows with the
  list's own colour checked
- **重命名 is inline**: the row's name becomes an input in its own place — the
  sidebar's page rename, moved into the 32 px nav row. Enter commits, Escape puts the
  old name back, and a blank name is refused. It reads a new `org-list-renaming`
  property and *not* the sidebar's `renaming-id`: those id spaces overlap, and a page
  numbered like the list would wear the rename box
- **删除清单** keeps its wording — the notice band says how many tasks moved to 收集箱 —
  and the view falls back to the inbox only when the list being *looked at* is the one
  that went (the deleted list may be a neighbour of the selected one)
- **The note page writes its own replies.** The `引用` list has been read-only since
  ADR-0112 (only 指令's `comment` could add one); the page now carries a field and a
  评论 button, committed on Enter or the button and *not* on a 300 ms settle — a reply
  is a **create**, and half a row is not a row. A reply to a note that is gone is
  refused rather than written: a dangling ref is a thing a merge leaves behind, not a
  thing a write should make
- `ContextMenu` now renders `MenuRow.check` and `MenuRow.swatch` — both fields have been
  on the struct since the block menu needed them, and this popup ignored them. The list
  colour picker needed the swatch, and the page menu's Style rows show their checks now,
  which the swept `page-style` scene says out loud
- The never-wired `org-list-color-set` / `org-list-deleted` callbacks are gone; the ⋯ is
  the door, and the delete logic is one shared call
- The reply card takes an explicit `height` off its wrapped text's preferred-height (the
  board card's own idiom): a bare `Rectangle` in the detail column's layout absorbs the
  pane's leftover height, which stretched one line of reply down half the panel — a
  shape no scene had photographed until the reply scene existed
- `notes-reply`, `tasks-list-menu`, `tasks-list-color`, `tasks-list-rename` and three
  dark arms join the sweep
- **No core change and no rev bump**: a list is existing `UpdateTaskList` /
  `DeleteTaskList` commands, and a reply is one `CreateNote`

### Notes & tasks — 回收站 (ADR-0114, core ADR-0003)

- **A delete is a stamp.** The 🗑, the row menu's 删除 and the selection bar's 删除
  now write `deleted_at` on the row (`UpdateNote` / `UpdateTask`) instead of removing
  it, so a delete is reversible twice over: the 撤销 bar for three seconds, and the
  bin for as long as the user wants
- **回收站 is a nav row on both tabs** — the bin is the *list* turned over, so what it
  shows is the half the user is standing on, and its count is that half's
- A binned row carries its own two verbs, drawn in the row: **恢复** (back where it
  was) and **彻底删除** (the only write that really removes it, and the only one the
  undo stack has to reach for). 清空回收站 in the header purges the whole half as
  **one** Ctrl+Z
- The **search box applies to the bin** and the tag filter does not: the 标签 column's
  counts are the live rows', so a filter over them would narrow by a number drawn
  from somewhere else. Picking a tag, a view or a list closes the bin
- **`edited` does not move** when a row is binned — the content did not change, so
  详细信息's 修改 still says when the note was last written
- 指令 gains **`restore`**, and its `delete` now means *bin* — the same verb the 🗑 is
  — so an AI batch can take a row out of 回收站 as easily as it put one in
- **Core rev `4899857 → fbfdaca`**, which brings the tombstone, the `live_*` /
  `trashed_*` accessors and **snapshot version 3**. The version is the loud half: an
  older peer that could not see the tombstone would read a binned row as an ordinary
  remote edit and resurrect it, so a v2 build and a v3 build refuse each other
  instead. This shell and quire-compose ship together
- `bin`, `tasks-bin` and their dark arms join the sweep
- One new test (`the_bin_holds_stamped_rows_and_only_a_purge_removes_them`) and two
  updated ones: the deferred delete's expiry now asserts a *tombstone* in the file
  rather than a missing row, and the batched delete asserts all three rows are still
  there, binned

### Notes & tasks — 卡片版面、卡片菜单、悬浮 ＋ 与撰写层 (ADR-0115)

- **A note card is read age-first**, the way the reference app draws one: the age
  leads at the top-left, the ⚑ pin sits beside it, the row's ⋯ is at the right edge,
  then the note's own words, then its tags. The row is as tall as what it draws, so
  a captured note (no title) is one line shorter instead of carrying a gap
- **A note card answers a right-click** — and a drawn ⋯ on hover or on the selected
  row — opening the shared context menu: 置顶 / 取消置顶, 打开, 详细信息, 复制内容,
  复制唯一 ID, 转为待办, 删除. Both doors are off in 回收站 (the row draws 恢复 /
  彻底删除 itself, as a binned task row already does) and while 多选 is on
- **新建笔记 is a floating ＋** at the list's bottom-right (56 px, round, accent), and
  the nav column's ＋ is now drawn on 任务 only — the notes half's ＋ is that button
- It opens a **capture layer**: a card over the rows with a multi-line field, a
  placeholder, a hint line, and a floating **➤**. Nothing is written until ➤ — ✕,
  Escape and a click on the scrim simply drop the draft, so dismissing the layer
  costs no undo step and leaves no empty row behind. Ctrl+Enter sends
- **`#标签` in a draft become the note's tags**, byte for byte by the Android shell's
  own rule (`note_tag_tokens` is `MarkdownText.tagTokens` written out again): the same
  sentence typed into either shell files under the same tags. A bare `#` and an
  all-digit name are not tags — "issue #3" does not make a label called `3`
- `org-note-create` is gone with its caller, the nav column's ＋; 任务 keeps
  `org-task-create`
- **No core change and no rev bump**: a pin, a copy, a conversion and a delete are
  commands that already existed, and a capture is one `CreateNote`
- `notes-capture`, `notes-capture-empty`, `notes-menu` and their dark arms join the
  sweep; the empty capture is a scene of its own because it is the one that draws the
  field's placeholder and the dimmed ➤
- Four new tests: the card menu's rows (and 置顶 read as the way *back*), the capture
  writing body + tags, the token rule's three refusals, and an empty draft writing
  nothing

### Notes & tasks — 筛选预填、打开笔记页自动弹出输入框 (ADR-0117)
- **A new row keeps the tag filter it was made under.** 笔记's 悬浮 ＋ opens the
  capture layer with `#筛选标签 ` already in the field, and 任务's ＋ plus both
  quick-add lines (the list column's and a board column's) attach the path to the
  row they create — on the create itself, so keeping it costs no second Ctrl+Z. The
  pre-filled `#token` is read back by ➤ through the same `note_tag_tokens` rule the
  user's own typing goes through, and the caret is placed *after* it
  (`org-capture-caret`): a caret at 0 would put the next keystroke in front of the
  tag and file the note under a token that never started with `#`
- **打开笔记页时自动弹出输入框**, a new Settings row under 笔记 and on by default:
  *arriving* at 笔记 — the sidebar row, a palette command, a cold start that lands
  there — opens the capture layer with the caret in it, the way the reference app's
  inbox does. A back/forward step does not: `org_show` is the paint history also
  lands on, `org_land` is the arrival, and only the second pops the layer
- The flag is the shared `notes.auto_input` row, **absent meaning on** — the first
  settings row in this shell whose default is not "off", which is what
  `setting_flag_or` exists for. The Android shell reads the same row, so the choice
  travels with the library rather than with the machine
- Not in this slice: 反向筛选 paths are *hidden*, not inherited, so neither the
  pre-fill nor the task merge ever carries one; and the layer still opens empty when
  nothing is filtered, because a draft left behind by a dismissal is a note nobody
  asked to finish (ADR-0115)
- Two new tests: a task created under a filter carries it, and an absent flag whose
  default is on does not read as off

### Notes & tasks — 笔记页单栏、笔记浮层、标签建议 (ADR-0118)
- **The 笔记 half is one column.** Its 208 px nav column and its 340 px detail
  column are gone; the list owns the card. What the nav column carried became a
  **filter bar** in the list's own header: the 搜索笔记 needle, the tag chips (the
  level under the current path, each with its subtree count and the drawn ⊖ for
  反向筛选, plus 全部笔记 / the excluded set / 清除筛选) and a 回收站 chip. It is the
  Android shell's own shape, so the two shells filter the same way as well as by the
  same rules
- **A note opens as a 浮层 over the rows** — 420 px, inset from the card's top and
  right, holding the fields the docked column held, scrollable, with the ✕ every
  other popup in this window closes from; Escape does the same. Picking another row
  moves it, and closing it puts the selection down without writing anything
- **任务 keeps both columns**, deliberately: 收集箱 / 今天 / 最近 7 天 / the lists are
  that half's navigation and have nowhere else to be
- **The capture field suggests tags.** Typing a `#token` raises a tray of known tags
  over the field; tapping one rewrites the token into the whole tag and puts the
  caret after it. The pool is the live catalog of both halves, the prefix matches
  the whole tag or its last segment (so `#工` finds `项目/工作`), and the tray holds
  at most five
- **The sidebar can no longer light two rows at once.** The page tree's highlight is
  Rust's projection of `open_page` and the organizer opens no page — so 笔记 showed
  *its* pinned row lit beside the previously-open page's. The row now paints
  `node.selected && active-area != "organizer"`
- **设置 closes on a click outside it.** It was the one close policy that cannot be
  dismissed from outside — the engine eats that click and the dialog then ignored it
  — so only 完成 and Escape got rid of the card
- Also fixed: the 回收站 row's 恢复 / 彻底删除 pair was pinned with a layout's
  `self.width` (which is the whole row, because a layout fills its parent) and so
  rendered over the note's own text
- New sweep scenes `notes-capture-suggest` / `dark-notes-capture-suggest`; `notes`,
  `notes-filter`, `notes-detail` and `notes-info` now photograph the new geometry
- One new test: the suggestion rule — only the last `#` counts, a space or a
  newline ends the token, `#工` finds both `工作` and `项目/工作`, and an empty
  prefix returns the pool capped at five
- Not in this slice: the tray reads the draft's *tail* token rather than the one the
  caret sits in (Slint exposes no caret position), so moving the caret back into the
  middle of a line costs a suggestion rather than applying the wrong tag

### Sidebar — 左栏每一行的右键菜单 (ADR-0120)
- **Every row in the left rail answers a right-click now**, not just the page tree's
  own rows. The 收藏 / 最近 rows open the same page menu their tree row does — they
  were dead (the guard was `kind == "page"`), so the same page had a menu in one
  listing and nothing in the other
- The four fixed entries are callable too: 笔记 opens the notes half and adds
  **新建笔记** (the floating ＋'s door), 任务 / 搜索 / 设置 open what they name
- The header at the top of the rail — with the chevron that promised a menu and
  opened nothing — is a **button**: it opens the workspace's menu, 新建页面 /
  **打开数据文件夹** / 设置. The folder row is absent in a memory-only session, which is
  the settings dialog's own rule
- A page's menu is anchored from **the row that was clicked** rather than looked up
  by id, which fixes right-clicking the tree row of a *favorited* page — the popup
  used to open on the 收藏 row instead (ADR-0120)
- 收藏 / 最近 section headers keep no menu: they are labels, and their verbs would be
  invented to fill the silence
- **Fixed on the way past**: `TREE_TOP_PX` was still the 140 of a rail with two pinned
  rows, so every page-menu screenshot anchored 56 px above its row (and the emoji grid
  with it). It is 196 now — 40 title bar + 36 header + the four pinned rows + 8 spacer
- New test `the_rails_own_rows_offer_the_doors_they_have`; new shot scene
  `sidebar-menu`

### HTML — 在浏览器中打开 (ADR-0119)
- **HTML is not a content format; the system browser is its only door.** A new
  palette command, **在浏览器中打开 HTML…** (页面), picks a local `.html` / `.htm`
  file and hands its path to `ShellExecuteW` — the same call the attachment blocks
  use — so the registered browser draws it. Nothing is fetched, parsed or drawn
  in-app, and the file does not become a page
- There is **no HTML block, importer or exporter**, and that is the decision, not
  a gap: a content channel is ≈ 3–4k LOC and lossy both ways, and in-app rendering
  needs the WebView §三十三 forbids. The cost analysis is ADR-0119
- The Android shell gets the matching OS-open path (its ADR-0023): the embed card
  opens its address through `Intent.ACTION_VIEW`, behind the same http/https/mailto
  allow-list the desktop already checks
- `open_with_default` is used rather than the link path's inline `cmd /C start`, so
  a picked path never reaches a command line
- New test: the palette row is present in 页面 and resolves to its own action

### Theme (ADR-0116)
- The palette is now **ActivityWatch's twelve themes**, ported field for field from
  `aw-qtui/src/theme.h`'s `kThemes[]` and chosen by name in Settings:
  暗夜蓝 midnight (default) / 石墨灰 graphite / 紫罗兰 violet / 森林绿 emerald /
  琥珀暖 amber / 海洋青 ocean / 珊瑚红 rose / 明亮 light, plus the four
  gradient themes 翡翠绿 jade / 深空蓝 deepblue / 暮光紫 twilight / 荣艳红 crimson
- **跟随系统** is the one choice that is not a palette: it resolves to `midnight` or
  `light` from the platform's colour scheme, and everything downstream reads a real id
- Settings' 外观 section is a preview grid — each card paints its own ramp in its own
  ink, so the picker needs no legend (it replaced the two 浅色/深色 cards)
- The four gradient themes paint a vertical ramp (`Colors.page`) on a full-window
  rectangle behind the shell, with the editor and organizer transparent above it so
  the ramp is one surface; the nine flat themes set `grad2 == bg`, so one rule draws
  both kinds — and they come out byte-identical to the pre-catalog build. The ramp
  deliberately does *not* live on `Window.background`, which is the renderer's clear
  colour and silently flattens a brush to its first stop
- The selection is an id, not a mode: `UIState.dark` became `UIState.theme`, and
  `Theme.dark` is derived from it. A library written by the previous build still
  opens in the theme it was left in — the old `dark` spelling resolves to `midnight`
- Notion's ten block swatches, the find marker and the cover veil are unchanged and
  shared by all twelve: they are content colours and arithmetic, not chrome
- `Ctrl+Shift+L` still means light↔midnight rather than walking the list, and the
  command palette's 切换主题 now persists — it used to assign the UIState global
  directly and never reach `AppState`, so the choice was lost on quit

### Workspace
- Page tree: create / rename in place / duplicate (nested lists survive) /
  delete with confirmation; favorites; recent pages; last page restored
  on startup
- Page title edits in place; word/char count in the editor footer
- Page look (top bar ⋯ → Style): a page picks its own typeface — Default, Serif
  or Monospace — and switches Full width and Small text. All three are stored on
  the page (schema v10: `pages.font`, `pages.layout`), never on a block: one
  derived token layer applies them to the document tier, so the sidebar, menus,
  palette and settings keep their own type. Small text shrinks body and headings
  by the same factor; full width drops the centred column for a left gutter.
  Not undoable, like Favorite — a look is a property, not an edit — and a
  duplicated page starts with its source's (ADR-0044)
- Page icon (top bar ⋯ → Set icon): a 96-emoji grid, twelve rows of eight, plus a
  None row to clear it. The page stores the emoji itself (schema v11:
  `pages.icon`), not the grid's index, so the catalogue can grow without
  rewriting anybody's page. An unset page shows its title's first character in the
  sidebar tree — the shortcuts keep their star and clock until a page really has
  an icon, and the page title shows nothing above itself rather than its own
  first letter at 46px. Like Style, setting one is not an undo step, and a
  duplicated page starts with its source's (ADR-0045). An icon is an emoji only —
  a picture belongs on the cover, not in a 16 px slot (ADR-0046)
- Page cover (top bar ⋯ → Set cover): a local picture behind the page title, with
  Change cover and Remove cover beside it. The page stores an attachment reference
  (schema v12: `pages.cover`, nullable), never a path, so the STORAGE reclaim can
  tell that the page still draws those bytes. A fixed dark veil sits between the
  picture and the title, which is what makes the title's contrast a bound rather
  than an opinion: white ink measures **6.19:1** over the worst picture a user can
  pick — a pure-white one — with room to spare over the 4.5:1 floor (which needs
  only α ≥ 0.535, and this veil is α 0.6196).
  `benchmarks/scripts/contrast_probe.ps1` reads that number off the
  rendered screenshot, self-checks its arithmetic, and ships with a scene built to
  fail so the gate is shown able to say no. The page's own emoji takes the same
  rule (it measured 1.04:1 before this was caught on pixels). Like Style and the
  icon, setting a cover is not an undo step, and a duplicated page starts with its
  source's (ADR-0047)
- Page lock (top bar ⋯ → Lock page): a read-only switch on the page itself
  (schema v13: `pages.locked`, `NOT NULL DEFAULT 0`, so "open" is a value and an
  old library upgrades with nothing locked). It closes the document — every
  block's content and the page's own title — while leaving the page's *look*
  (icon, cover, Style, favourite), the tree (move, delete, duplicate) and every
  read (navigation, search, folding) alone. Two gates, because one is not enough:
  the command funnel refuses the write, and the row's `editing` binding refuses
  the caret, so a stale edit cannot survive a lock/unlock/relock cycle. Nothing
  is swallowed silently — the notice bar says which switch to flip, the page says
  it in a pill above its title, the ⋯ row reads "Unlock page", and the ⋮⋮ menu
  keeps only its two read-only Copy rows. Undo is refused while a page is locked
  and its stack is kept, not dropped; a duplicated page starts **un**locked,
  which is the one place a lock departs from a look (ADR-0048)
- Template library (top bar ⋯ → Templates): a template is a page you cannot open —
  its rows are a body to copy from, and that is all it ever is (schema v14:
  `pages.template`; no second content format, so marks, colours, code languages,
  column layouts and page references ride along in a copy untouched). Six rows
  manage the whole feature: insert one into the page you are typing in (the slash
  menu and the "+" handle offer the library too, filtered by what you type), start
  a new page from one, save the current page as one, export and import through the
  same Markdown channel pages use, and delete one. An insert is one Ctrl+Z step, not
  eleven, and it fills the empty line it lands on rather than sitting below it. Five
  presets — Meeting notes, Weekly review, Project brief, Bug report, Long-form draft
  — land on a library's first start, and deleting one is permanent: they are imported
  once, not re-added at every launch. A template stays out of the sidebar, the page
  tree, the palette, search and the shared LAN workspace, which is a consequence of
  it never being attached to the tree rather than a list of places that remember to
  hide it (ADR-0049)
- Fixed: an **empty** list item used to vanish from a Markdown export, so a
  template's blank row — the line left open for somebody to fill in — did not
  survive its own round trip. An empty block now writes its marker bare (`-`,
  `1.`, `>`, `#`) and comes back as the same empty block, which also fixes
  ordinary pages whose drafts include an unfilled bullet (ADR-0049)
- Page version history (top bar ⋯ → Version history): one popup, two views — the
  versions this page has had, and what changed between one of them and what is on
  screen now. The three actions live on three surfaces: naming is the field at the
  bottom (always there, because "now" is worth a version), comparing is a row click,
  and restoring is a button that only exists *after* a comparison, because a restore
  replaces the page. A version is §二十五's snapshot mechanism pointed at one page:
  a `VACUUM INTO` copy in a `versions/` folder beside the database, narrowed to that
  page — so reading one back goes through the same loader the app opens the library
  with, and there is no second content format for marks, colours, code languages,
  tables or page references to fall out of. Which versions exist is two metadata
  rows each: the name the user typed, and the attachment files its rows point at,
  which is what stops the reclaim from freeing a picture a version still needs.
  Twenty per page, oldest going, the panel says the number out loud, and deleting a
  page forgets the versions it had. A restore is **one** Ctrl+Z step and rolls back a
  page's *content* only — the title, icon, cover and style the user chose since stay.
  The comparison is line-level and identifies a line by its block, so a moved or
  edited line reads as the pair it is; a stretch too large to align is reported as a
  rewrite rather than computed (ADR-0091)
- Slash menu ("/") for block types; command palette (Ctrl+K) with page
  jumping, plus Go Back / Go Forward (Alt+← / Alt+→) along the pages
  visited this session — a page deleted since drops out of the history
  instead of being opened
- Full-text search (Ctrl+P) — titles and content, Chinese included
  (FTS5 + segmentation)
- Context menus on pages and blocks; pages move: Move up / Move down
  reorders siblings, Move to reparents anywhere outside the moved
  subtree (the whole hierarchy travels, recorded and restored across
  restarts)

### Persistence & reliability
- SQLite (bundled, no server): pages, blocks, marks, colors, attachments,
  settings, metadata (schema v1–v14)
- Debounced batched writes; Ctrl+S forces a save; close saves too
- Rotating snapshots on every open (5 generations), restore-at-open when
  the main file is damaged, damaged file quarantined (`.corrupt`)
- Named page versions (ADR-0091) share that mechanism and not its lifecycle:
  one self-contained SQLite file per version in a `versions/` folder beside the
  database, twenty newest per page, pruned by hand or by the cap and never by age.
  A file the index no longer names is swept on the next save, so a save that died
  halfway leaves nothing invisible behind
- Startup integrity checks; schema migrations (v1–v14). The steps that only add a
  column share one helper and each guard on its own column's absence, so a
  half-migrated file — one somebody edited by hand, or restored to the middle of a
  sequence — converges instead of erroring on a duplicate column name
- Picture and file bytes live in an `attachments` folder beside the database
  and the row is only a reference — a reference whose file row is gone still
  loads the library and renders as a missing picture. A file is copied in
  without being read, so nothing about attaching one scales with its size
- Settings → STORAGE can **reclaim** the attachments nothing points at any
  more: the rows go first (`Change::AttachmentDeleted`, which no command plan
  emits), then the files beside them, and the notice bar reports how many and
  how many bytes. "Nothing points at" counts every page's blocks, every step
  still on any page's undo *or* redo stack, the copied block, and the pictures
  any stored version points at, so the
  100-step undo cap is how long a picture is protected, and the sweep reads
  only the rows this session loaded, never the folder. It runs on the UI
  thread and costs about half a second per thousand attachments
- An unclean end (panic, kill, native crash, power loss) is recognized on
  the next start: the notice bar says so and the fact is queryable in the
  metadata table (a panic additionally keeps its report)

### Desktop integration
- Frameless window with custom title bar, light + dark themes (persisted)
- System tray (ADR-0096): closing the window now hides it instead of ending the
  session, and the only exit is the tray icon's right-click menu — 「显示主界面」
  brings the window back (as does a left-click on the icon, on platforms that
  report one), 「退出」 quits. Nothing intercepts the close: Slint keeps the event
  loop alive while anything visible is left and counts a visible tray icon the
  same as a visible window, so the tray existing *is* the behaviour. The icon is
  the one artwork `install/make_icon.ps1` rasterises, the same picture the exe,
  the installer and the shortcuts carry, transparency included
- That exit is reachable from outside the app now (ADR-0105): `quire.exe --quit`
  asks the *running* instance to end its own session — the same
  `slint::quit_event_loop()` the tray menu calls, so the final flush and the
  clean-exit record still happen — and exits 0 only if an instance accepted,
  non-zero if nothing was listening. It answers before any session of its own
  starts, so asking is never a second window or a second log line. It exists so
  a deploy can replace a live exe without killing it: a killed session is
  missing its end record, which is precisely what the next start reports as a
  crash that never happened
- The light theme's quietest text is measured rather than eyeballed: the third
  text tier (block handles, footer, sidebar, every hint row) went from 2.5–2.7:1
  to 4.1–4.4:1, and the two weakest block colours from 2.81 and 2.48 on their own
  tint to 3.61 and 3.56 — the band the dark theme has always sat in (ADR-0023)
- Window size remembered; last-opened page restored
- Whole-window zoom: Ctrl+= (or Ctrl+Shift+=) and Ctrl+- step the shell and the
  page together, Ctrl+0 resets, and the factor is remembered across restarts
  (also listed in Settings). Slint's one lever for this is the window's **scale
  factor** — the number that turns logical into physical pixels — so the zoom
  is "pretend the display is Z times denser": that single number covers
  `Theme`'s ladder, every literal px in `ui/`, glyph rasterisation and
  hit-testing, and the window's physical size never moves (the layout is given
  a smaller logical viewport, which is what zooming in means for a window that
  is not a scrollable document view). A monitor or display-scaling change
  re-announces the platform's own factor through the same door, so what the
  window reports is compared against `base × zoom` on every resize and the zoom
  is put back instead of being swallowed
- Settings: appearance, LAN sharing, the database folder (open it in
  Explorer, take a backup on demand, reclaim unused attachments)
- Markdown export/import (page level, inline marks round-trip)
- "Copy Page as Markdown" (command palette): the page through the
  exporter onto the clipboard, CJK-safe (Win32 FFI write path)
- An attached file opens in whatever the system has registered for its type
  (one `ShellExecuteW` call — no `windows` crate, no subprocess), and saves
  back out to any path picked in a Save-as dialog
- A link — inline, in a link block or on an embed card — leaves the app only
  when it is an address a browser understands: http, https or mailto. A local
  path, a network share, `file://` or a protocol the shell happens to have
  registered now does nothing, because a link's target is data that arrives
  from a file and the shell will *run* a path as readily as it opens a url
- Installer (Inno Setup): per-user, Start menu + desktop shortcuts,
  optional `.md` "Open with" association; `--open <path>` dispatch
- GPU rendering (FemtoVG default; Skia / wgpu builds selectable); idle
  CPU ≈ 0, a 10 000-block page costs ≈10 MB over the empty shell

### LAN sync (quire-core's `services::sync`)
- Two Quire installs on one network keep each other's workspace by exchanging
  whole snapshots and merging them three ways (aw-server-plus's model, adapted
  to Quire's integer ids and its change lists). The protocol lives in
  `quire-core`, so both shells depend on one implementation instead of
  carrying a copy each: UDP discovery on 46000, a dependency-free HTTP server
  on 5878 (`/sync/info`, `/sync/snapshot` GET and POST,
  `/sync/attachment/<id>`, `/sync/pair`), a worker thread that runs one
  pull-merge-push cycle at a time, and a Slint `Timer` on the UI thread that
  answers every job that has to touch the session — the workspace is `Rc`-bound
  to that thread, so nothing else may. What stays in each shell is the session
  glue (building a snapshot out of the live workspace and walking a merged one
  back in) and that pump. The crate also grew
  `SqliteRepository::records_of` for it — every record row of one database in
  one query, where the snapshot used to search by an empty needle and then
  point-read each row — pinned by an assertion in the storage suite
- The merge is against a **shadow** (what the two devices last agreed on, one
  settings row per peer): a row only the other side moved is taken, a row only
  this side moved is kept, a row both sides edited keeps this device's copy and
  says so in the log, and a row *neither* side had before — where both minted
  the same integer id, because ids are per-device `max + 1` — is renumbered
  against this session's own watermarks and kept alongside, cascading to the
  pages, blocks, attachments, databases, columns, views, records and cells it
  carries. First contact between two populated devices runs without a shadow
  and treats every differing row as a conflict: two workspaces meeting for the
  first time is one user decision, not one algorithm
- Pairing is trust-on-first-use: a device found on the wire (or typed in by
  hand as `ip[:port]`, for networks where the announcement cannot get through)
  is asked to pair with one POST, and both sides end up in each other's table.
  Sync then runs on demand (**Sync now**) or on a minute timer while the
  automatic switch is on; attachment bytes travel over the same protocol
  (`/sync/attachment/<id>`), pictures re-entering through the ordinary
  `import_bytes` path so the receiver builds its own preview. The settings
  dialog's SYNC section lists the devices, their last sync and the two ways in
- What the snapshot deliberately does not carry: settings rows (device-local by
  policy — theme, window size and the peers table stay where they were set),
  named versions, and the `created`/`edited` stamps of database records, which
  the receiving store stamps for itself
- Dark is the default theme now (`dark_setting`): a library with no stored
  `theme` row opens dark, and a stored "light" still wins — the two platforms
  share that rule

### LAN sync — 对端的沉默、跨页移动与同步页 (ADR-0121…0124)
- **A peer's silence about a collection it cannot carry is no longer a deletion.**
  The Android shell exports no `databases` and no `attachments` — it has neither
  store — and the three-way merge cannot tell that silence from "the user deleted
  every row of them", so one agreed round and then one empty answer used to take a
  desktop's tables and pictures out. The answer is a gate on **who may speak for
  which collection** (`peer_carries_the_whole_library`): a Windows / macOS / Linux
  peer's empty list is a deletion and lands; an Android peer's, or an unknown kind's,
  is not, and this library's own rows are answered back — into the diff, into the
  snapshot pushed back, and into the shadow, so the next round reads the same
  nothing and keeps reading it as nothing (ADR-0122)
- The same rule from the other end: the phone **trims an inbound snapshot at its own
  door** instead of refusing the round, which is what used to make every
  desktop-initiated sync fail with 409. Trimmed rows stay out of the shadow it
  agrees to, for the reason above in reverse (its ADR-0024)
- **A database *block* is not a database row.** The phone's veto read the block and
  switched this device's sync off for good after a single round with a desktop that
  had a table anywhere in its library — no server, no announcements, no pairing, and
  a line telling the user to sync with the desktop they had just synced with. The
  veto now reads rows only, which is the case it was ever about (its ADR-0025)
- **An export that cannot read its own records refuses to answer** rather than
  describing a database it only read half of: the write queue is force-flushed first
  (the last few seconds of typing were otherwise a peer's "you changed nothing"),
  `records_of` / `values_of` failures propagate, a schema with no store behind it is
  refused, and the pump **drops the reply channel** so the peer's round fails instead
  of merging a shorter snapshot
- **A block that moved pages is a move** (`BlockMovedToPage`), not a delete on the
  page it left plus an insert on the page it joined: the id is UNIQUE, so the insert
  failed and took that page's whole batch down with it while the in-memory document,
  whose insert only checks the page it was handed, kept a copy on both. The deletes
  and the field diffs now read one library-wide snapshot (ADR-0123)
- **A merged page list is tested for a tree before the tree is touched.** The merge
  decides row by row and the tree is one object, so two shapes reached
  `Workspace::attach` from ordinary use and each answered with an unwrap on a lookup
  it expected to be there — on the UI thread, because a sync apply is a `Job` the pump
  drains. A parent the peer deleted beside a child this side renamed: the delete
  cascades, the kept child is re-inserted under a page that is gone, and the app
  stops; with the guard short-circuited the new test dies at `workspace.rs:156`
  (`parent exists`), so that is a proven crash and not a story. And two devices that
  each nested the other's page: `move_page` refuses the second half — correctly — while
  the apply recorded the move anyway, so `pages.parent` held a cycle the memory tree
  did not, and a restart finds **no root for either page**: an empty sidebar with every
  row still on disk. A page whose parent is not here now lands at the top level, a
  refused move leaves this device's own place standing, and what was actually built is
  written back into the merged row, because that row is the pushed answer and the
  shadow — an assertion this device cannot keep would be argued again every round
  (ADR-0124)
- The schema deletes the merge decided finally land (`PropertyDeleted`,
  `ViewDeleted`, `DatabaseDeleted`), so a table deleted on one desktop stays deleted;
  `AttachmentDeleted` is deliberately **not** among them — a false deletion there is
  a lost file rather than a stale row
- **An inbound push is refused unless the device is in the peer table as paired**:
  the core's server marks any sender `paired: true` on the strength of the snapshot
  naming itself, so this is the only gate there is on either end
- The round says what it is doing: the port is tried before the engine starts (a
  listener that cannot bind used to die on a thread nothing reads), a round that runs
  past five minutes is reported rather than leaving the busy dot lit forever, the
  automatic cycle only dials peers that announced in the last minute (a silent device
  is a device whose server stopped too), and a merge that dropped rows the peer
  cannot carry logs how many it kept
- The 同步 section of the settings dialog grew the rest of what a user needs to
  finish a pairing by hand: **本机名称** is editable, the interval is four chips
  (30 / 60 / 300 / 1800 s) instead of a hidden setting, **本机地址** is shown with the
  port and says 未联网 when there is no route, a `●` marks a round in flight, and the
  last twelve log lines — including the merge's conflicts, which were previously
  invisible — are listed under 最近记录 (ADR-0121). Each paired row now also prints
  the address it was last reached at, because 「the address moved to another device」
  is a sentence the user can only act on with the number on screen; and a rejected
  push or an unanswered round logs the peer's **name** rather than its
  device id, since the log's peer column is 96 px and elides a hash into nonsense
- A round that keeps an attachment row back says how many it kept: an inbound row
  with neither bytes nor a local copy lands as nothing (a row is the promise a peer
  uses to decide a file already crossed), which left the cycle reporting 已同步 over
  a picture that stays missing every round — the apply now counts those rows and
  logs 对端有 N 个附件没有把文件带过来
- Three more from the same read-through: a peer row **minted** by `sync_note_device`
  kept whatever port the caller handed it, and `0` is the shell's own word for
  「nothing was said」 — so the existing-row branch fell back while a new row stored
  an address nobody can dial (a new row now takes `SYNC_PORT`, and a caller that *did*
  name a port is still not second-guessed); a merge that took the library's **last**
  page redrew the editor but not the tree, because `open_page` is what normally
  rebuilds it and there was no page left to open, so the rail kept listing pages that
  exist nowhere but there; and the five-minute sweep said 本轮同步已放弃, which is a
  verdict on the peer's round rather than on this dialog's patience — every step of a
  round carries its own HTTP budget (60 s for the snapshot, 120 s per attachment,
  120 s for the push), so a dozen files to carry can still be working long after the
  dot went out. It now says the machine stopped waiting, and a late `SyncDone` still
  logs the real outcome under it
- Sync no longer keeps an idle machine's databases paying for somebody else's
  broadcast. `record` spent a database-cache stamp on **every** change batch, and the sync
  pump records a `sync.peers` settings row each time a peer announces itself (every four
  seconds), so the stamp was always newer than any open window and the next read of every
  table and gallery on screen missed the cache while the editor had recorded nothing but an
  address. A batch that is *only* settings now leaves the cache alone (ADR-0086: the record
  template lives on the database entity, so no setting can move a cell); batches that carry
  rows still spend it exactly as before, and zoom is unaffected either way because a new
  viewport changes the cached window itself. See `docs/PERFORMANCE.md`, §M2.5p
- A peer's announcement no longer reaches the file at all. `sync_note_device` stored the
  whole peers table on every beat, and the only field a beat moves is `last_seen` — which
  both of its readers want *now*, from memory (the auto-cycle's sixty-second gate, the
  dialog's fifteen-second online dot). The cost was a SQLite transaction every four
  seconds per listening device, and because a durable write is what arms the periodic
  snapshot, a **whole-file copy every ten minutes** of a laptop nobody is touching. The
  table is now written into memory always and recorded only when something a restart would
  still want moved: a peer added or dropped, an address, name, kind or port corrected, a
  pairing decided, a round that succeeded
- The same read caught a stranger one: the load path read `settings` only when the file
  had **pages**, so a library whose last page was taken — which a merge from a desktop
  peer can legitimately do (ADR-0122) — came back from the next start with no theme, no
  pairings, no merge shadows and **no device id**; the id it minted then is a *new device*
  to every peer that knew this one, so 配对 breaks from both sides at once, and the
  built-in library the last session deleted came back (its 「already seeded」 flag is a
  settings row too). Settings are read from the load now, whatever the page count is
- New tests: `an_export_that_cannot_read_its_records_refuses_to_answer`,
  `a_block_that_moved_pages_survives_the_round_either_way` (both directions),
  `a_phone_that_carries_no_databases_does_not_delete_this_ones` (+ its log line),
  `a_desktop_that_dropped_a_database_takes_it_out_of_this_one_too` (the control),
  `only_a_desktop_peer_speaks_for_the_whole_library`,
  `a_stored_attachment_never_writes_where_its_name_points`,
  `a_parent_the_peer_deleted_takes_the_child_it_kept_to_the_top`,
  `two_devices_that_each_nested_the_others_page_keep_their_own_trees` (+ its store
  round-trip and the control that says that round-trip had rows to read),
  `a_self_address_carries_the_port_it_was_found_on`,
  `a_row_that_arrives_without_its_file_is_said_out_loud` (+ the control that the row
  really is in the merged answer),
  `a_peer_row_minted_without_a_port_gets_the_sync_port` (+ its named-port control),
  `a_settings_row_alone_does_not_spend_the_database_cache` (+ its added-row control),
  `a_peer_beat_that_changes_nothing_durable_writes_nothing` (+ six durable-fact controls,
  each read back out of the file rather than off the queue),
  `another_session_does_not_replay_the_beat_it_just_loaded` (+ its moved-address control on
  the cold start), `an_empty_library_with_settings_behind_it_keeps_its_identity` (+ the
  control that the file it opens really has settings and no pages).
  **Still open, and written up in `docs/ROADMAP.md` §"Sync backlog owned
  by `quire-core`"**: everything that needs a `quire-core` change — the push carries no
  attachment bytes, one unreadable file fails a whole round, an inbound push cannot name
  its sender's kind and records an ephemeral port, `GET /sync/snapshot` answers anyone
  who asks, and the fetch decision is keyed by integer id so a colliding attachment is
  never fetched for. Four of the nine were proven against the pinned rev by throwaway
  probes or by reading the fetch loop, not by guessing; **three of them are closed by
  the section below.**
- Correction to the section above while passing through: discovery is **5879**, not
  46000, and the automatic cycle runs on the interval the user chose, not a fixed
  minute

### LAN sync — 笔记与任务按 唯一 ID 合并，revision 说了算 (core ADR-0004, ADR-0125)
- **`notes` and `tasks` are merged by their 唯一 ID, not by their integer row id.** Two
  devices that each filed a note under row 1 hold *two notes* — the integer is a
  per-device watermark and the uuid is the name — so the merge no longer renumbers the
  organizer at all, and the state a renumber created (two distinct rows left sharing one
  唯一 ID, ROADMAP item 7) is now unreachable rather than papered over. A row that
  arrived is inserted under a fresh local id, and the merge reports the `remote id →
  local id` map, so a reply's `ref_note` follows its parent to wherever it landed
- **A conflict is settled once, by the row's revision** — `"{millis:013}-{device}"`
  compared as a string, the newer copy standing. Both ends compute that answer from the
  same two strings, so the round that *finds* a conflict is the round that settles it.
  The old rule kept this device's copy **and wrote that copy into its own shadow**, so
  the next round read the winner's row as a one-sided edit and converged on an argument
  nobody won (ROADMAP item 8). This is `aw-server-plus`'s rev rule, which the merge's
  own header had claimed to be "adapted" and never was
- **The bin needs no special case**: a trash and a restore are writes of the row like
  any other — which is exactly why the revision is a field of its own, since `edited`
  deliberately does not move when a row is binned, and a bin and the restore that undoes
  it would otherwise carry the same stamp with nothing to order them
- **A purge stays the shadow's job.** It is the one removal with no row to carry it, so
  the shadow remains what tells "a row this side never had" from "a row this side
  purged"; a stateless last-writer-wins cannot tell them apart and would resurrect what
  the user emptied out of 回收站
- **The `rev` column is new (core migration 30)**, backfilled in place from
  `MAX(edited, deleted_at) * 1000` so no legacy row reaches a merge unstamped, and
  **every organizer write restamps it** — from the one funnel each shell already had
  (`exec_org` / `exec_org_all` here, `apply` / `apply_all` on the phone), never from the
  twenty call sites that build the rows, and never over the `before` half an undo writes
  back (ADR-0125)
- **Both devices update together**: `SNAPSHOT_VERSION` is **4**. It is not a wire-shape
  change — `rev` is a defaulted field — but a build that still keys the organizer by
  `id` would answer this one's rows under ids this one never wrote, which is silent
  divergence rather than a visible failure, so the gate refuses instead
- New tests: `an_organizer_row_syncs_by_its_uuid_and_the_newer_revision_wins` here;
  `the_uuid_is_the_key_not_the_integer_id`,
  `an_organizer_edit_lands_and_the_newer_revision_wins_a_double_edit`,
  `a_bin_travels_as_a_write_of_the_row_it_belongs_to`,
  `a_comment_follows_its_parent_to_the_integer_it_landed_under`,
  `the_v30_step_adds_and_backfills_the_organizer_revisions` and
  `stamp_rev_covers_the_written_rows_and_never_the_before_half` in the core; and
  `the_funnel_stamps_the_written_row_and_never_the_before_half` on the phone

### LAN sync — 真机联调：一个真 bug、和去掉「按 IP 添加」(ADR-0126)
- **This is the first time two real devices paired, pulled, pushed and conflicted on
  the same Wi-Fi** (a Lenovo tablet on `dev.quire.compose` against this shell), and
  it paid for itself immediately. Discovery over UDP works both ways; 发起配对 from
  the tablet's row is accepted by this shell's engine with no dialog; the tablet's
  real 7 notes / 2 tasks / 15 pages landed here, and a note edited *here* with a
  newer revision replaced the tablet's copy in one round — the whole point of core
  ADR-0004, seen on screen rather than in a unit test
- **A round with the tablet reported `push answered 409` on every cycle**, and the
  refusal was this shell's own and correct: the body said it came from
  `TEST-DESKTOP`, i.e. from *this* device, and a push from a device that is not in
  the peer book is refused. The bug was the phone's — it answered a pull with the
  identity of the snapshot it had been sent. Fixed there (compose ADR-0027); the
  data had converged anyway, because the pull half of every round lands and this
  shell's own rounds pull, so only the *report* was wrong. Exactly the kind of
  defect that a test cannot see: `sync_apply_remote` is driven by the suite, and
  nobody ever looked at the identity it returns
- **The 「按 IP 添加」 row is gone** (ADR-0126): a device is found by its broadcast
  and paired from its row, so this shell no longer asks for an address the network
  should have told it. `Cmd::ProbeAdd` and the `awaited` entry keyed by address go
  with it. On a network that filters broadcasts the 已发现的设备 list is now simply
  empty — the honest answer — and the alternative weighed (the row kept under
  Settings as an advanced door) was not taken
- Not here yet, and the reason it matters: the library this ran against was a
  **scratch** one (`--portable`), because the user's real library is on schema 29
  and opening it with this build migrates it to 30, which the installed 0.1.10
  could then no longer open. Their real library is untouched and has no sync rows;
  `just deploy-workshop` is what puts this build there

### Build & test
- `just check`: `cargo check --all-targets`, the whole test suite, a release
  build. Visual regression and the RAM/CPU scenes run from
  `benchmarks/scripts/` (`sweep.ps1` compares against a manifest of hashes)
- `just deploy-workshop` writes two places (ADR-0105).
  `C:\workshop\quire-desktop-<version>\quire.exe` is that release's archive and
  never moves; `C:\workshop\quire-desktop\quire.exe` is the install — the one
  path the app is run from, overwritten in place on every deploy, so "the
  newest" stops being a folder name to remember. The instance living on the
  install path is asked to quit through the channel above and *waited for*
  (30 s) before the copy, and the step sits after the build and the push
  because closing the user's window is the one part of a deploy they can feel;
  if it accepts and then stays up, the deploy refuses rather than overwriting a
  live exe. An instance running out of an older version folder holds nothing the
  deploy writes, so it is named and left alone instead of being closed for
   nothing
- `just release-publish` is the other door a release leaves by (ADR-0106): it
  bumps the patch, builds the exe, wraps it twice —
  `build-installer.ps1 -SkipBuild` for `Quire-<version>-windows-x64-setup.exe` and
  `dist.ps1` for the portable `Quire-<version>-windows-x64.zip` — commits and
  pushes the bump, and attaches both to `v<version>` on `master`. The version is
  read out of `Cargo.toml` and into the exe's version resource, which `quire.iss`
  reads `AppVersion` from, so nothing is typed twice; the assets are built before
  the push, so an Inno Setup that is not installed fails before anything leaves
  the machine. `just verify-install` stays the user's own gate and is not run by
  the publish
- The benchmark row writers no longer commit a machine path. ADR-0094 rewrote
  the 22 `benchmarks/results/*.jsonl` files that had one, and the next run would
  have written it straight back: `exe` and `db` are absolute paths at run time.
  `benchmarks/scripts/redact.ps1` now substitutes `<repo>`, `<temp>`,
  `<localappdata>` and `<user>` before those fields are JSON-escaped, in all four
  writers, and warns instead of staying quiet when an account name still reads as
  a whole path segment (ADR-0094)
- A test that needs a folder — a database, a log family, an attachments
  directory — gets one from `quire::testing::ScratchDir`, which deletes it when
  the test ends. Each helper used to create a uniquely named `%TEMP%` directory
  and walk away from it: 3 639 of them had accumulated, and the naming that
  existed to stop a test reading a stale database was only load-bearing
  because nothing ever cleaned up
- The bench scripts delete the scratch database, its `.bak<N>` snapshots and
  their own report file when a run ends; the purge used to happen before the
  run and covered only the `.db`, so every label left its snapshot behind
- `quire-shot --hover x,y` parks the pointer without pressing a button, which
  is the only headless way to light UI that exists on hover alone (a table's
  edge toolbar, a row's ⋮ handle)
- `benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` prints, per scene, the
  bounding box of the pixels that moved between two sweeps. A re-sweep is
  judged from that table rather than from 42 pictures: when every box lands
  inside the band the change explains, one verdict covers the set
- The command palette's ids now resolve through `palette_action()` in Rust, and
  its dispatch is a `match` with no wildcard arm, so a new command that skips a
  variant is a compile error. A test walks the real command registry and asserts
  every row resolves to its own distinct action — the class of bug that once
  shipped with every palette row from id 9 up running one action, green tests
  and all (ADR-0034)
- `--pictures N` makes a bench scene of media: every `rows/N`-th row of the
  bench page becomes an image block, backed by a generated 1280×720 PNG from a
  pool of 200 files written when the library is first built. The fixtures are
  not re-encoded during the measured passes — a run that timed PNG encoding
  would not be measuring pictures — and the app prints its own decode cache
  figures (`--dump-state`, one JSON line on stderr) because a process memory
  counter cannot see a cache whose unit is one raster
- `--marks N` does the same for the other fixture scene D never had: it bolds the
  second word of every `rows/N`-th row, so a 10 000-block page can carry 1 000
  marked paragraphs and the RAM gate can see the runs channel at all. Without it
  the gate was blind — scene D has no marks, so a change to marked rendering
  measured 1.00× of nothing. `--dump-state` now also reports what it built
  (`gs+atlas-blocks=10024 marked=1000`), which lets each arm of an A/B prove its
  own fixture instead of taking the bench script's label on faith
- Scene F — continuous scroll — had never scrolled. Slint measures a list's
  content offset *negative* going down, and the harness timer was adding, so
  every frame wrote a value the clamp rounded straight back to zero: the scroll
  numbers recorded since M2 describe a repaint loop at the top of the page, not
  a scroll. `quire-shot --scroll-y` is now the control that catches this class
  of thing — two offsets that are real must produce two different PNGs — and
  `--scroll-step` sets how far a frame moves, so a flick and a wheel tick are
  separate measurements (ADR-0036)
- `benchmarks/scripts/scroll_ab.ps1` runs the scroll arms against two binaries in
  one sitting — the default build and a `--no-default-features --features
  skia-opengl` one in `target-skia/` — and fails the batch unless each binary's
  own first-paint line reports the renderer it is supposed to be. That
  self-identification is the whole point: the skia arm of the renderer verdict
  had been standing on the same broken scroll as the femtovg one, and a skia
  build that silently keeps the femtovg default would otherwise produce an A/B
  of a binary against itself (ADR-0036)
- The matrix's `-Only` filter now splits a comma list and throws when no scene
  matched. `powershell -File` passes `-Only A,B` to a `[string[]]` parameter as
  one string, so a multi-token filter selected nothing, printed nothing and exited
  0 — a run that measured nothing and said it finished. Two more shapes joined the
  matrix so every row the media batch published is reproducible from the script:
  the flick-sized scroll on a text page, and a short page of pictures
- A RAM gate now comes with its control. The previous four batches each reported
  a same-direction rise on both arms and recorded it as "session drift" without
  ever measuring what drift is, so the math slice built the commit before it
  (`2e9de99`) in a temporary worktree against the same target dir and ran the two
  exes alternately in one sitting: 1.016× on private bytes, with each binary
  first proving which build it is (md5, and the control tree has no math source
  file). The worktree goes once the number is in; the next kind that claims a
  per-row cost repeats the run against its own predecessor commit
- The contents block repeated that run against the commit before it (`cc7ccf0`) and
  came in at 1.007× on private bytes. It also bought the first whole-page projection
  number: the gate's scene has no contents block in it, so a green gate alone says
  nothing about a row that walks the page every time it is projected. Measured
  separately — 10 000 rows project in ≈38 ms with or without a 1 000-line list —
  and the delta is inside the noise of its own control
- The embed card is the third slice to run that comparison, and it lands in the
  same place: **1.005×** on private bytes, a 0.55 MB gap between the arms against
  the control arm's own 1.2 MB spread (rows
  `benchmarks/results/2026-09-21-m11-embed-ram.jsonl`). Three kinds in a row
  inside one megabyte says the gate has a *floor*, not that all three slices are
  free: a change whose whole per-row cost is a string function is below what this
  instrument resolves, and `docs/PERFORMANCE.md` now says that instead of
  publishing a ratio that only looks like a measurement
- The workspace is two crates now (ADR-0093): `crates/data` holds the model, the store
  and the windowless services — 31 455 lines that compile against nothing but `std`,
  `rusqlite` and `image` — and the root package keeps the Slint shell. That is the
  precondition for the Android port and for a Rust sync module, and it moved the gate
  command: **`cargo test` at a workspace root tests only the root package**, so the bare
  form here reported 134 passed and exit 0 while 431 data-layer tests sat unrun. `just
  check` says `--workspace` on all three lines now. Nothing else moved: 565 passed / 23
  ignored with the 589 test names proved identical to the run before, and 131 of 131
  sweep PNGs byte-identical between two shot binaries that differ by md5
- The same day that crate left this repository (ADR-0094): `crates/data/` is deleted,
  and `github.com/PT123123/quire-core` at a pinned rev is where the model and the store
  live now — 71 commits carried out with `git filter-repo`, of which 46 had a personal
  QQ address as author and committer and were rewritten to the GitHub noreply identity
  before the first push. The same pass then went over **this** repository, where 137 of
  174 commits and the tagger of `v0.1.0-rc1` carried the same address on a public remote
  — Track 3's handoff had asserted the metadata was clean because it had scanned the
  diff and never `%ae`, and that sentence has been corrected in place rather than
  quietly fixed. Every SHA quoted in these documents predates it. Landed as
  `origin/master` = `f475026` and `v0.1.0-rc1` = `1dbbde4`; the check that says the
  source did not move is that `cargo check` on the adopted head finished in **11 s**
  having recompiled nothing, and the shell's suite still reads 134 / 0 / 10 The rename is the only content change: 38 of 45 source files
  still hash byte-identical to the blobs this repository holds at `09ef5aa:crates/data/`,
  and the other 7 hash identical once the name is reversed. Two things follow that are
  worth knowing before quoting a green run. **The suite is no longer reachable from
  here** — a git dependency is not a workspace member, so `cargo test --workspace` runs
  the shell's 134 / 10 ignored and only *builds* the crate whose 431 tests gate it. And
  **cargo's own git cannot fetch a public repository**: it offers credentials the
  anonymous URL never asked for and takes a 401, so `.cargo/config.toml` now sets
  `net.git-fetch-with-cli = true` and a fresh clone builds without an SSH key

### Known limitations
- Switching directly from one open menu to another (e.g. ⋮⋮ on a different
  block while a menu is open) takes two clicks — the first click only
  dismisses the open popup (standard Slint popup semantics)
- A menu taller than the window (e.g. Move-to in a large workspace) is
  clamped to the space below its anchor and scrolls (the page menu) or is
  re-anchored when a submenu changes its row count (the ⋮⋮ block menu) — so
  nothing runs off the bottom any more. What is left is the difference
  between the two: a menu taller than the whole window scrolls only the
  page menu, because the ⋮⋮ menu is still a plain repetition, not a
  `ListView`. The Settings dialog no longer has that shape: it fits 1280x800
  with its shortcut list, ABOUT and Done on screen, and the sidebar chord is
  now one of the rows it lists
- Block colors are cosmetic: they do not survive a Markdown export/import
  round trip, and Callout blocks export as quotes
- Inline-mark paragraphs wrap between words now, and still clip in two shapes:
  a marked phrase longer than the line (a mark is one unbreakable cell), and a
  marked line that needs more lines than the same words unmarked — bold and mono
  are wider, and the row's height is measured from the plain text
  (Slint `Text` has no inline formatting yet)
- Pictures: replacing a stored file on disk in place needs a
  restart to show up. A page of pictures is now measured while it scrolls — the
  decode cache holds nine 1280×720 rasters and stops there by weight, and an
  on-screen picture costs the process roughly three times the raster — but the
  fixtures are generated gradients, so the disk footprint and any decode-time
  arm are optimistic against a real camera original. A clipboard picture other
  than a bitmap (a `file://` HTML image, an SVG) is not read — only
  `CF_DIB`/`CF_DIBV5`
- Attachments are reclaimed by hand, never on their own: Settings → STORAGE
  → Reclaim deletes the stored files no block, undo step or the copied block
  points at, and nothing runs it for you — a picture is protected for the
  100 undo steps of its page, and after that it waits for the click. And
  because the sweep reads only the rows this session loaded from the
  database, a file in the `attachments` folder whose row is already gone
  stays on disk: it is invisible to the count, and deleting it would mean
  listing the folder, which a session that failed to load its attachments
  would get terribly wrong
- A PDF attaches and opens, but shows no first-page thumbnail: it looks
  like any other file apart from its name. Deferred by explicit decision
  2026-09-20, and a version that was built but never committed was
  dropped on 2026-09-23. How it would be drawn is no longer an open
  question (four routes measured, the pure-Rust one chosen — docs/
  REPORT_TRACK4.md §T4.1); that it ships at all still is
- Tables are a grid, not a database: no per-column widths, no header
  formatting, no sorting. Inside a cell only Tab / Shift+Tab cross between
  cells — Enter does not split one, Backspace does not merge it with a
  neighbour, and the arrow keys will not step up or down a row. A cell takes
  bold / italic / code / strikethrough / formula and — since this slice, on a
  selection like the other four marks — a link (Ctrl+L). Converting a marked
  line into a table keeps its words
  and drops its marks. Hovering the grid adds its toolbar as a row, so
  content below a
  table shifts down by 22 px while the pointer is on it
- Math is a Unicode reading, not typesetting: `\frac{a+b}{2}` paints on one
  line as `(a+b)/2`, so there are no stacked fractions, no alignment, no
  equation numbers, and a superscript falls back to its plain characters
  wherever the alphabet has no glyph for it. Unknown commands come back as
  their own source. A Math mark and the other marks do not stack — a formula
  inside bold text keeps the formula and drops the bold on export
- A contents block lists the headings of the page it sits on, and nothing more: no
  collection across pages, no "show levels 1–2" setting, no compact or inline
  option, and a long heading is elided to one line rather than wrapped. Clicking a
  line moves the caret, not the scrollbar — a heading below the fold still needs a
  wheel turn first, which is the same limit a `quire://block/` anchor has
- An embed card says who a link belongs to, not what it is: no title, no preview,
  no favicon and nothing fetched, because the app has no WebView and makes no
  request it was not told to make (it does have an HTTP/1.0 client —
  `--pull <url>`, no TLS). This is **closed**, not pending: `bookmark` was
  withdrawn 2026-09-22 (ADR-0081), so no fetched title is coming. A site
  outside the 21 names it knows is labelled with its own
  host, and a Google url is read as its product from the subdomain or the first
  path segment — anything else behind google.com says "Google". And since this
  slice, a link whose target is not http, https or mailto opens nothing at all:
  `file://`, a local path and a network share are the shapes a document can carry
  that a shell would run rather than open
- Markdown reads tables as plain text: export writes GitHub-flavoured
  tables, and importing one back gives a paragraph per row. Deliberate and
  pinned by a test — the importer is line-at-a-time and a table needs
  lookahead
- A built-in template is only as rich as Markdown: the five presets carry
  headings, bullets, numbered items, a todo, a quote, a link, a contents block
  (the one `<!-- quire:toc -->` marker the channel knows) and one code block,
  because that is what the importer knows. A preset that asked for a callout, a
  table, a colour or a column layout would arrive as a paragraph, so the built-in
  library cannot show those shapes — a template you save from your own page can,
  since that one is a copy of real rows. And saving never overwrites: start from a
  template, edit it, save it back, and the library gains a second entry with the
  same name rather than updating the first, so pruning is a Delete you do by hand
- Chinese IME: the manual acceptance pass (docs/IME_CHECKLIST.md) is
  signed off — 2026-09-20
- Databases say what they will not do rather than guessing: a formula or rollup
  column cannot be sorted or filtered (that would mean evaluating every row of a
  10 000-row column, which is the red line the windowed read exists to keep), and
  a rollup cannot fold through another computed column, so its nesting depth is
  zero by construction
- A relation's picker lists a bounded number of the target's rows and its search
  box is the way past that bound — but nothing caps how many records one cell may
  *point at*. It is measured, not assumed: folding a window whose relation fans
  out to 10 000 rows costs 28.5 ms against 9.1 ms for reading those 10 000 values
  out and folding them in the app, so the upper bound is the fan-out rather than
  the table size. Whether to cap it is an open product decision
- Deleting a page from the sidebar is still not an undoable action (it predates
  the command registry), and a record owns its page in storage — so a record
  reached only through that page goes with it. Deleting the *record* is undoable
  and takes its page in the same step; that is the path this feature controls
