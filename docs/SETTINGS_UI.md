# 设置弹窗显示逻辑规格

本文整理设置弹窗（`ui/components/SettingsDialog.slint`）的显示规则：什么在什么时候可见、
每一行控件归谁所有、数值从哪里来又写到哪里去。改设置相关的 UI 之前先读这篇——
这里的大多数规则都对应一次真实回归（出处写在括号里）。

## 一、弹窗本体

- `PopupWindow`，宽 **560px**（同步页搬回弹窗后定的，ADR-0132），窗口内居中。
- 高度 = `min(内容高 + 2×spacing-lg, max(窗口高 − 80, 320))`：卡片跟着内容长，
  但永远装得进窗口——底部行被截掉的对话框没有"滚下去"之外的路，所以内容区是
  一个 `Flickable`。
- `close-policy: close-on-click-outside`（ADR-0118 后半）。引擎吃掉外部点击并关闭
  弹窗，`AppWindow` 的 `settings-shown` 镜像把这次关闭写回 `UIState.settings-open`，
  Rust 层由此得知。不要改回 `no-auto-close`——那是唯一一种从外面关不掉的策略。
- 打开时**搜索框**聚焦（`changed opened`），它是这个弹窗现在最主要的输入。
- Escape 有两个受理者：0×0 的 focus-proxy 与搜索框自己的 `key-pressed`，都走
  `settings-close-requested`。
- `settings-tab` 在关闭后**不清零**：tab 是位置，不是草稿。

## 二、Tab 与搜索的显示规则

- `settings-tab` 单一 index：0 外观 / 1 通用 / 2 同步 / 3 快捷键 / 4 关于。
  一个状态，不是五个 bool。
- 搜索框为空：显示分段 tab 控件 + 当前 tab 的 section。
- 搜索框非空：**tab 控件隐藏**（"tab 是地点，搜索结果列表不是"），每个 section
  的显示条件换成 `settings-match(节关键词)`，节内每行再按**自己的** keywords 过滤。
- `settings-match(keywords)` 是 Rust 侧的大小写不敏感子串测试（`.slint` 表达式没有
  `contains`）。keywords 是一句空格分词的话，查询串整体子串匹配。
- **维护规则（最容易踩）**：一个 section 的关键词必须是**节内所有行关键词的并集**。
  漏一个词，那行单独能匹配、整节却被藏起——搜索"命中了却看不见"。
- 一切被过滤的元素都是条件（`if`)子元素而不是 `visible: false`：Slint 布局里
  `visible: false` 的元素**仍占着自己的槽位**（本文件的头部注释记着这笔学费）。

## 三、可复制文字规则（ADR-0135）

- **可以复制的行**用 `CopyableText`（`read-only` 的 `TextInput`，可聚焦、可选区、
  Ctrl+A/Ctrl+C，拒绝其余一切）：版本、渲染器、数据文件夹路径、快捷键表的两列、
  每个开关行的标签、清理说明。
- **保持纯 `Text` 的**是 chrome：节标题、字段名（「版本」「渲染器」）、说明行、
  按钮标签。复制一个标题不是任何人需要做的事。
- 长路径（数据文件夹）必须 `wrap: true`——elide 截掉的恰恰是辨认文件夹的尾巴。
- `CopyableText` **不声明自己的 TouchArea**（一层盖在输入框上的 TouchArea 会把
  点击和拖选一起吃掉）。

## 四、五个 tab 的内容清单

| tab | 内容 | 归属 |
| --- | --- | --- |
| 外观 | 12 张主题卡（跟随系统 + `Colors.themes`，卡即预览）+ 界面缩放行（ZoomRow） | 本文件 |
| 通用 | 三个开关（启动输入框 / Alt+N / Alt+M）+ 一条说明 + 局域网只读共享开关 + 存储块 | 本文件；存储块三按钮走 Rust |
| 同步 | 整节委托给 `SyncPage`（设备 / 统计 / 日志三个分段） | `SyncPage.slint`；节只负责关键词 |
| 快捷键 | `ShortcutRow` 单列表（keys 列 mono、108px 固定宽） | 本文件，纯展示 |
| 关于 | 版本、渲染器 | 本文件；渲染器是有意不进侧栏页脚的 |

存储块的规则：数据文件夹在 `storage-available == false`（内存会话）时显示
「内存中运行 — 未挂载数据库」并把三个按钮**整个藏起**（条件子元素，不是 visible——
占位的 hidden 行会把上方标签的宽度偷走）。清理按钮的唯一说明是按钮**上方**那行
可复制文字（SPEC §三十七：删除类操作的规则必须先于点击可读）。

## 五、界面缩放行（ZoomRow）

- **显示**：`Math.round(zoom-level × 100) + "%"`。`zoom-level` 是 Rust 推送的
  `in` 属性——wire 时推一次，之后每次缩放（键盘或按钮）都重推。行只**读**状态，
  不自己记账，键盘和按钮永远不会不一致。
- **步进**：− / + 走 `ZOOM_STEPS` 梯级（state.rs：0.5 → 3.0 共 13 档，
  `zoom_step` 找相邻档，不在梯上的值挪到邻档）。两端**禁用**：
  − 在 ≤0.5、+ 在 ≥3.0；**复位**在 |level − 1| ≤ 0.001 时禁用（一个把当前值
  原样写回去的按钮是噪音）。
- **与键盘同一条路**：按钮只调 `UIState.zoom-in/out/reset`，和 Ctrl+= / Ctrl+- /
  Ctrl+0 汇合到同一个 controller 回调（`zoom_step` → `set_zoom` 持久化 →
  `apply_zoom`）。
- **持久化**：`ui.zoom`，写前 clamp 到 [0.5, 3.0]（防手改）。1.0 = 跟随系统 100%。
- 下方说明行（muted、可复制）写明快捷键，让键盘路径在这里可被发现。

## 六、缩放与窗口的边缘规则（`controller::apply_zoom`）

这些不是设置弹窗自己的逻辑，但「外观 → 界面缩放」的每一下都过这里：

- `zoom` 是相对 **base_scale** 的倍数；base_scale 是平台自己的 DPI 因子，两者分开存，
  zoom 1 才能永远等于平台因子，resize 才能分清"哪个数是谁的"。
- **browser-style**：保持的是逻辑视口（≥ min 窗口 940×600），物理窗口跟着因子走。
  视口低于 min 会让 winit 的约束执行在每次布局变化时来"修"窗口——0.1.19 之前
  侧栏一开关就打架，就是这个原因。
- **上限只从上面 cap**：把 min 窗口装进所在显示器工作区的最大因子；zoom-out 只会
  让视口变大，永远不会违 min，所以不被 cap。显示器答不上来（窗口未落位、ghost
  坐标）按"某个真实显示器"算保守上限，绝不按"无上限"。
- **最大化**：以当前物理尺寸为界重新推导视口，不动 set_size（OS 拥有它的大小）。
- **DPI 变更**：`on_window_resized` 发现窗口实际因子 ≠ base×zoom → 采纳新 base，
  下一轮事件循环 `apply_zoom` 把 zoom 套回去（不在 resize 中途派发自己的事件）。
- **位置**：因子变化可能把窗口推出桌面（右/下边先出去），按工作区拉回；
  `monitors::place` 在启动时对保存的矩形做同一件事。

## 七、窗口与侧栏的边缘数字

- 窗口 min **940×600**：`AppWindow.slint` 的 min-width/min-height 与
  `controller.rs` 的 `MIN_WINDOW_LOGICAL_W/H` 是**同一对数字的两份声明**，改一处
  必须改另一处。resize-border 6px。
- 侧栏拖拽 clamp **200–480**（默认 260）：`Sidebar.slint` 的 clamp 与
  `SIDEBAR_WIDTH_MIN/MAX` 同源同规矩；拖拽中实时写 `sidebar-width`（关闭动画，
  `sidebar-resizing` 置 true 时动画为 0ms），松手才持久化（`sidebar.width`），
  并把钳后的值写回 shell——拖过界的超出量不允许在 rail 和设置之间留下分歧。
  开合持久化在 `sidebar.closed`。
- **树/工作区右键菜单锚点 = 侧栏实时宽度**（2026-10-01 修：曾硬编码 240，侧栏拖宽
  时菜单压在行上、拖窄时孤在编辑器里）：`x = sidebar-open ? sidebar-width : 0`，
  再钳到 `窗口宽 − 8`。
- 弹窗钳制惯例（全 app 一致）：`y ∈ [48, 窗口高 − 菜单高 − 8]`（取不到就 48）、
  `x ∈ [8, 窗口宽 − 宽 − 8]`；子菜单换行后必须按**新的**行数重锚
  （`reanchor_menu` / block 菜单的 `reanchor`），行高按菜单自己画的算
  （ContextMenu 30px/行 + 8 padding——曾经按 28+16 估，被 A4 sweep 修掉）。
- 命令面板/搜索面板高度 = 内容行数封顶（8×36 / 7×52）+ 输入行 + 页脚，
  从 y=96 起最坏 ≈ 500px，min 窗口 600px 装得下——给它们加行数时先验这道账。

## 八、持久化键清单（`settings` 表）

| 键 | 写入者 | 缺省语义 |
| --- | --- | --- |
| `theme` | 外观主题卡 / Ctrl+Shift+L | 缺失 = 跟随系统；非法值写为 `midnight`；light/dark 旧拼写读时映射 |
| `ui.zoom` | Ctrl+=/−/0、外观缩放行 | 缺失 = 1.0；写前 clamp [0.5, 3.0] |
| `sidebar.width` | 侧栏拖拽松手 | 缺失 = 260；读时同样钳 200–480 |
| `sidebar.closed` | Ctrl+\、侧栏开合 | "0"/缺失 = 开 |
| `notes.auto_input` | 通用开关 | **缺失 = 开**（`setting_flag_or(_, true)`，参考实现的默认反向读） |
| `hotkeys.alt-n` / `hotkeys.alt-m` | 通用开关 | 缺失 = 关；main.rs 注册热键读同一行 |
| `lan.share` | 通用开关 | 缺失 = 关（端口 5877，重启生效） |
| `sync.*` | 同步页 | auto / interval(≥10s) / peers / log / device-id / device-name / shadow.* |

写设置走 `record_setting`（一次批量写，和主题一样），读侧的缺省值规则写在
`wire`（controller.rs）——**改默认值要同时改读取处和这张表**。

## 九、快捷键表（「快捷键」tab 的行）

命令面板 Ctrl+K / 搜索页面 Ctrl+P / 页内查找 Ctrl+F / 新建页面 Ctrl+N /
切换侧边栏 Ctrl+\ / 切换深浅色主题 Ctrl+Shift+L / 撤销 Ctrl+Z / 重做
Ctrl+Shift+Z / Y / 粗体斜体代码公式 Ctrl+B I E M / 缩进 Tab Shift+Tab /
复制块 Ctrl+D / 保存 Ctrl+S / 放大缩小复位 Ctrl+= − 0 / 后退前进 Alt+← →。
这张表是**AppWindow KeyBinding 的影子**：新加绑定必须同步加行（2026-10-01 补上
撤销/重做/主题切换三行——它们此前只在键盘上存在）。
