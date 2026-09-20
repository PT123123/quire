# Quire 项目规格（原始需求文档）

> 本文档是项目的初始需求与长期约束说明，随项目保留存档。README 中为其浓缩版。

---

你现在要从零开始开发一个现代、轻量、GPU 加速的 Notion-like 本地文档应用。

这是一个长期项目，不要把它理解成一次性的 Demo。第一目标平台是 Windows，后续再考虑 Android。项目核心要求是：

1. 界面必须好看，达到现代桌面生产力软件的视觉质量，参考 Notion、Linear、Obsidian、Craft、Raycast、Arc 等产品的设计理念，但不要直接复制任何品牌 UI、Logo、图标或视觉资产。
2. Windows 下必须尽量低内存、低 CPU。
3. UI 必须使用 GPU 加速渲染。
4. 不使用 Electron、Chromium、WebView2、Tauri、React、Vue、Web 技术作为主要 UI 运行时。
5. 使用 Rust + Slint。
6. UI 与核心业务逻辑严格分离，但默认保持单进程，不要把 Frontend 和 Backend 拆成两个进程。
7. 代码必须非常适合 Coding Agent 持续修改和扩展，避免形成像输入法/TSF 那种平台耦合极深、修改成本极高的架构。
8. 第一版不要做同步、协作、云端、插件市场、AI、实时多人编辑等大型能力。
9. 优先把“漂亮的桌面 UI + Block Editor + 本地存储 + 极低资源占用”做完整。
10. 所有架构选择都要优先考虑长期可维护性，而不是为了短期功能量堆复杂度。

==================================================
一、项目技术底座
========

不要直接 fork 一个完整的第三方 Notion-like 项目。

以官方 Slint Rust Template 为工程起点：

https://github.com/slint-ui/slint-rust-template

这个仓库只作为工程脚手架，不要把里面的 Todo 示例逻辑继续发展成最终架构。

参考以下官方仓库，但只学习架构和 UI 写法：

1. Slint 主仓库：
   https://github.com/slint-ui/slint

2. 官方 cargo-ui：
   https://github.com/slint-ui/cargo-ui

3. 官方 node-editor-cpp：
   https://github.com/slint-ui/node-editor-cpp

特别参考 node-editor-cpp 对以下问题的处理方式：

* `.slint` 负责视觉、布局、交互
* backend 负责 model
* UI callback 与 backend API 有明确边界
* 动态模型通过 Slint model 接入
* 复杂交互状态不要散落在 UI 各处

不要复制它的具体项目结构，只吸收架构思想。

当前 Slint 版本优先使用 1.18.x 或当前稳定的 1.x 小版本，不要无理由锁死过旧版本。2026-09-16 发布的 Slint 1.18 已经包含更好的大文本渲染/编辑性能、FlexboxLayout、运行时 z-order、拖拽、改进的代码生成等功能，这些能力对本项目有直接价值。

==================================================
二、项目总体架构
========

不要采用：

UI → JS → API → DB

不要采用：

UI → WebView → Web Backend

不要采用：

多个进程通过 IPC 通信

推荐：

```
                 Application
                      |
        ┌─────────────┴─────────────┐
     Slint UI Layer              Rust Core
        |                             |
   ┌────┼────┐            ┌──────────┼──────────┐
Sidebar Editor Command   Document/Page/Block   Model → DB   Search
   |                        Services
   |                     Import / Export
```

整个应用默认保持单进程。

UI 层：

* `.slint`
* 视觉
* 布局
* 动画
* 交互
* 当前 UI 状态
* Focus
* Selection
* Popup
* Menu

Rust Core：

* Document Model
* Page Model
* Block Model
* Undo/Redo
* Persistence
* SQLite
* Search
* Import/Export
* Application state
* Command dispatch
* 后台任务

原则：

UI 不直接碰数据库。

UI 不直接执行磁盘 IO。

UI 不自己管理业务数据的一致性。

Rust Core 不负责视觉细节。

Rust Core 不应该知道具体某个按钮是什么颜色。

==================================================
三、目录结构
======

从第一天就建立清晰目录。

推荐：

src/
main.rs

```
core/
    mod.rs
    document.rs
    page.rs
    block.rs
    inline.rs
    command.rs
    history.rs
    selection.rs

storage/
    mod.rs
    database.rs
    migrations.rs
    repository.rs

services/
    mod.rs
    document_service.rs
    search_service.rs
    import_service.rs
    export_service.rs

platform/
    mod.rs
    windows.rs

app/
    mod.rs
    state.rs
    controller.rs
```

ui/
AppWindow.slint
Theme.slint
Colors.slint
Typography.slint
Icons.slint

```
components/
    AppShell.slint
    Sidebar.slint
    SidebarItem.slint
    PageTree.slint
    Editor.slint
    EditorBlock.slint
    BlockHandle.slint
    BlockMenu.slint
    SlashMenu.slint
    CommandPalette.slint
    SearchPanel.slint
    TopBar.slint
    Button.slint
    IconButton.slint
    Tooltip.slint
    ContextMenu.slint
    Dialog.slint

blocks/
    ParagraphBlock.slint
    HeadingBlock.slint
    TodoBlock.slint
    BulletBlock.slint
    NumberedListBlock.slint
    QuoteBlock.slint
    CodeBlock.slint
    DividerBlock.slint
```

tests/
fixtures/
integration/

benchmarks/
documents/
scripts/

docs/
ARCHITECTURE.md
UI_ARCHITECTURE.md
EDITOR_ARCHITECTURE.md
PERFORMANCE.md
ROADMAP.md
DECISIONS.md

PLAN.md
README.md

不要把所有 UI 写进一个 `main.slint`。

不要把所有 Rust 逻辑写进 `main.rs`。

==================================================
四、第一阶段：工程初始化
============

目标：

M0：能够稳定编译、启动，并确认 GPU renderer。

任务：

1. 从官方 slint-rust-template 初始化项目。
2. 替换项目名称。
3. 使用当前稳定 Slint 1.x。
4. Windows 使用 MSVC。
5. 建立 Debug / Release 两种构建。
6. 配置 Slint LSP。
7. 确认 VS Code 能正常获得：

   * 语法高亮
   * 补全
   * 跳转
   * live preview
8. 建立最基本的 CI：

   * cargo check
   * cargo test
   * cargo build --release
9. 创建最小主窗口。
10. 创建 GPU renderer 验证 Demo。

不要一开始就堆各种依赖。

尤其不要因为“以后可能需要”提前安装几十个 crate。

每个依赖都必须回答：

* 为什么需要？
* 是否能够用标准库解决？
* 是否会增加 runtime memory？
* 是否会增加后台线程？
* 是否会增加 build complexity？

==================================================
五、GPU Renderer 选择
=================

必须实际验证 renderer，不允许只根据网上印象决定。

Slint 当前 Winit backend 可以选择：

* FemtoVG
* FemtoVG + WGPU
* Skia
* Skia OpenGL
* Software

Windows 项目重点测试：

A. winit-femtovg-wgpu

B. winit-skia

必要时额外测试实验性的 Vello renderer，但不要在第一版直接使用实验 renderer。

优先关注：

1. 文本清晰度
2. 滚动流畅度
3. CPU
4. GPU
5. RAM
6. 首次启动时间
7. 空闲时 CPU 是否接近 0
8. 是否存在持续 frame
9. 长文本表现

注意：

GPU 加速不是目标本身。

目标是：

“用 GPU 渲染 UI，同时避免因为无意义持续重绘导致 CPU 很高。”

如果静止界面一直有动画、timer 或 render loop，必须查清楚原因。

==================================================
六、性能基线
======

在真正开发 UI 之前，建立 benchmark。

必须至少记录：

1. Startup time
2. Idle Working Set
3. Idle Private Bytes
4. Idle CPU
5. Simple document RAM
6. 1000 Block document RAM
7. 5000 Block document RAM
8. 10000 Block document RAM
9. Scroll CPU
10. Typing CPU
11. Search CPU
12. Page switching latency

必须区分：

Debug

和：

Release

所有正式 benchmark 以 Release 为准。

不要使用一个“看起来很漂亮”的页面作为唯一 benchmark。

建立四种固定场景：

Benchmark A：

空白窗口，只包含 App Shell。

Benchmark B：

100 个简单 Paragraph Block。

Benchmark C：

5000 个简单 Paragraph Block。

Benchmark D：

10000 个简单 Paragraph Block。

Benchmark E：

持续输入文本。

Benchmark F：

连续滚动。

Benchmark G：

快速切换 100 个页面。

每次关键架构修改后重新测试。

把结果写入：

docs/PERFORMANCE.md

==================================================
七、第二阶段：视觉系统
===========

这一步非常重要。

不要先做数据库。

先做一个“看起来已经像产品”的桌面 UI。

目标：

即使所有数据都是 mock data，截图也应该具有商业软件的完成度。

视觉参考：

Notion：

* 极简
* 大量留白
* 文档感
* 清晰层级

Linear：

* 高信息密度
* 精细 hover
* 很好的快捷操作
* 克制动画

Obsidian：

* 桌面知识库结构
* Sidebar + Document

不要直接复制这些软件。

建立自己的视觉语言。

---

## 主题系统

创建：

Theme.slint

Colors.slint

Typography.slint

Icons.slint

统一定义：

* background
* surface
* surface-hover
* surface-selected
* text-primary
* text-secondary
* text-muted
* border
* accent
* danger
* success
* code-background

定义：

* 标题字号
* 正文字号
* Sidebar 字号
* Command Menu 字号
* 行高
* 字重
* 圆角
* spacing scale

例如：

spacing-xs
spacing-sm
spacing-md
spacing-lg
spacing-xl

不要在 100 个 `.slint` 文件里各自写：

`8px`

`9px`

`11px`

`13px`

然后最后无法统一修改。

---

## 颜色

默认设计至少：

Light

Dark

但不需要第一阶段支持用户自定义颜色。

浅色模式优先：

* 接近白色的主背景
* 非纯黑文字
* 极轻的 border
* 低对比度 sidebar

深色：

* 不使用纯黑
* 不使用大面积强对比
* 文本层级清晰

---

## 动画

动画原则：

“有反馈，但不持续运行。”

允许：

* Hover fade
* Sidebar selection transition
* Menu enter
* Command Palette enter
* Drag feedback
* Popup scale/fade

禁止：

* 无限 loop 动画
* 每帧更新 UI state
* 无意义的呼吸灯
* 背景动画

---

## 窗口

Windows 应用应该考虑：

* 自定义 title bar
* WindowMoveArea
* 最小化
* 最大化
* 关闭
* window resize
* system tray 后续可选

但第一版不要花大量时间做 Windows 原生窗口特效。

==================================================
八、第三阶段：App Shell
================

先做：

┌─────────────────────────────────────────┐
│             Top / Title Bar             │
├──────────┬──────────────────────────────┤
│          │                              │
│ Sidebar  │          Document            │
│          │                              │
│ Workspace│       Editor Area            │
│ Pages    │                              │
│ Search   │                              │
│ Settings │                              │
│          │                              │
└──────────┴──────────────────────────────┘

Sidebar：

* Workspace
* Favorites
* Recent
* Page Tree
* Search
* Settings

顶部：

* 页面标题
* breadcrumb 可选
* Search
* 更多菜单

不要第一版做得像传统 IDE。

应用整体应该像现代生产力软件。

---

## Sidebar 行为

要求：

* hover
* selected
* right click
* collapse
* expand
* drag
* context menu

Page Tree：

支持：

Page

Sub Page

无限层级理论上允许，但视觉上需要限制展开复杂度。

不要一开始制作 10 万节点测试。

==================================================
九、第四阶段：Document Model
=====================

这是 Rust Core 的核心。

不要：

Page = 巨大 String

推荐：

Document

包含：

Page

包含：

Block Tree

示例：

Page
├── Heading
├── Paragraph
├── Paragraph
├── Todo
├── BulletList
│    ├── Paragraph
│    └── Paragraph
├── Quote
└── Code

每个 Block：

id

parent_id

order

type

properties

children

不要使用数组 index 作为永久 ID。

ID 必须稳定。

推荐 UUID 或适合本项目的轻量 ID。

排序必须支持插入和移动。

---

## Block Type

第一期：

paragraph

heading_1

heading_2

heading_3

bullet

numbered

todo

quote

code

divider

后续再添加：

image

bookmark

callout

table

toggle

database

不要第一期做这些。

2026-09-20 更新：上面这批已排进 §三十七（媒体 / 结构 / 嵌入三批）与 §三十九（database），不再是「以后再说」，按 §二十九 的 M10–M14 执行。第一期（M0–M8）约束仍然有效，不得借这条提前动块模型。

==================================================
十、Inline Model
==============

Notion 的正文不是单纯字符串。

因此预留：

Inline Text

Inline Code

Bold

Italic

Link

Strike

Highlight

但第一阶段不要一次把全部实现。

推荐：

Block

↓

Text Content

↓

Inline Span[]

每个 Span：

text

marks

link

style

---

## 重要限制

Slint 目前没有一个现成的完整 Notion-grade Rich Text Editor。

因此不要假定：

TextEdit = Notion Editor

不是这样的。

需要自己建立：

Editor Model

Selection

Caret

Block insertion

Block deletion

Block splitting

Block merging

Inline mark

Document command

==================================================
十一、第五阶段：Block Editor MVP
========================

这是第一阶段真正的核心。

必须先做稳定，而不是先做富文本。

必须实现：

1. 输入文字
2. Enter 新建 Block
3. Backspace 删除
4. Backspace 合并 Block
5. Delete
6. Up / Down
7. Left / Right
8. Home / End
9. Ctrl+A
10. Ctrl+C
11. Ctrl+V
12. Ctrl+Z
13. Ctrl+Shift+Z 或 Ctrl+Y
14. 鼠标选区
15. 鼠标点击定位
16. Todo
17. 切换 Block 类型
18. 删除 Block
19. 移动 Block

要求：

输入不能明显卡顿。

不要每输入一个字符就：

serialize document

write SQLite

rebuild all UI

refresh all block

正确做法：

用户输入

↓

Editor Controller

↓

更新当前 Block

↓

局部 UI 更新

↓

debounced persistence

---

## 中文输入

这是硬性要求。

禁止自己实现：

TSF

IME engine

Candidate Window

Composition Framework

输入法协议

Text Service

不要重复做你之前输入法项目的复杂基础设施。

优先使用 Slint 的文本输入设施和 Windows 系统 IME。

只有遇到 Slint/Windows 的实际 bug 时，再做最小的平台 adapter。

==================================================
十二、第六阶段：Block Rendering
=======================

Block Editor 必须考虑 Virtualization。

不要：

10000 blocks

↓

10000 个复杂永久 UI Item

↓

10000 个 TextEdit

正确目标：

viewport

↓

visible blocks

↓

nearby blocks

↓

复用/创建必要的 UI

核心原则：

不可见内容尽可能不占据昂贵的 UI 资源。

不要把每一个 Block 都永久绑定一个复杂 editor。

特别是 TextEdit。

因为：

TextEdit 是交互控件。

不是纯文本渲染器。

考虑将：

非编辑状态

和：

编辑状态

分开。

普通 Block：

轻量展示

当前编辑 Block：

进入 editor mode

例如：

普通：

ParagraphBlockView

当前：

ParagraphEditor

只有焦点 Block 使用真正 TextEdit。

这样可以极大降低长文档常驻 UI 成本。

==================================================
十三、第七阶段：Selection / Caret / Focus
=================================

这是编辑器稳定性的关键。

建立明确的 EditorState：

focused_block_id

selection_start

selection_end

anchor

cursor

composition_state

active_block

editing_mode

不要让每个 `.slint` 文件自己维护一套 selection。

Rust 中维护核心状态。

UI 只展示。

==================================================
十四、第八阶段：Command 系统
==================

所有核心编辑操作都应该尽量变成 Command。

例如：

InsertText

DeleteText

SplitBlock

MergeBlock

SetBlockType

MoveBlock

DeleteBlock

InsertBlock

ToggleTodo

ApplyMark

Undo

Redo

这样 Undo/Redo 就可以天然建立在 Command history 上。

不要实现：

“撤销的时候重新读取数据库”

Undo 应该针对内存中的 Document Model。

数据库只是 persistence。

==================================================
十五、第九阶段：Slash Command
=====================

实现 Notion 风格：

输入：

/

弹出：

┌──────────────────────┐
│ Search blocks...     │
├──────────────────────┤
│ Text                 │
│ Heading 1            │
│ Heading 2            │
│ Bullet List          │
│ Numbered List        │
│ Todo                 │
│ Quote                │
│ Code                 │
│ Divider              │
└──────────────────────┘

要求：

* 键盘操作
* 上下选择
* Enter 确认
* Esc 取消
* 模糊搜索
* 自动定位
* 不抢正常输入

Slash menu 不应该在 QML/Slint 中硬编码大量业务判断。

由 Rust 返回 command descriptors。

UI 负责展示。

==================================================
十六、第十阶段：Command Palette
=======================

提供全局：

Ctrl+K

或你最终确定的快捷键。

支持：

Open Page

Create Page

Search

Change Theme

Toggle Sidebar

Go Back

Go Forward

等等。

Command Palette 与 Slash Menu 必须复用底层 command registry。

不要写两套。

==================================================
十七、第十一阶段：Page Tree
==================

实现：

Create page

Rename page

Delete page

Duplicate page

Create subpage

Move page

Collapse

Expand

Search page

Favorites

Recent

页面树操作应该是异步但不应阻塞 UI。

大型 Tree 必须考虑 virtualization。

不要一次把整个 workspace 渲染成巨大的 UI tree。

---

本阶段只管树的结构。页面自身的图标、封面、版式、锁定、版本历史、模板见 §三十八；反向链接与提及见 §四十。

==================================================
十八、第十二阶段：SQLite
===============

使用 SQLite。

数据库至少考虑：

workspaces

pages

blocks

block_children

metadata

settings

后续：

search index

attachments

不要一开始引入：

Postgres

server

cloud database

Redis

sync backend

所有数据优先本地。

---

## 事务

以下操作应该具备事务一致性：

Create Page

Delete Page

Move Page

Move Block

Merge Block

Split Block

Bulk Update

Undo/Redo persistence

不要每个 SQL 单独提交然后让中间状态暴露。

==================================================
十九、第十三阶段：自动保存
=============

自动保存策略：

内存修改

↓

dirty

↓

debounce

↓

batch persistence

例如用户连续输入时：

不要：

每字符：

UPDATE SQLite

而应该：

停止输入一小段时间

↓

批量保存

特殊情况下：

Ctrl+S

立即保存。

程序关闭：

尽可能 flush dirty state。

==================================================
二十、第十四阶段：Search
===============

第一版实现：

Page title search

Content search

Block text search

不要做复杂 AI search。

搜索必须：

异步

不阻塞 UI

结果实时显示

支持 keyboard navigation。

后续可以升级：

SQLite FTS

但第一版先留接口。

==================================================
二十一、第十五阶段：视觉质量迭代
================

到这个阶段，必须暂停功能开发，专门做视觉。

要求 Agent 自己审查：

* spacing
* alignment
* typography
* hierarchy
* hover
* active
* selected
* focus
* popup
* menu
* sidebar
* scrollbar
* empty state
* loading state
* error state

不要出现：

一个按钮 8px 圆角

另一个 6px

另一个 12px

这种没有系统的设计。

不要默认使用过多阴影。

不要让每个区域都有 border。

要让 UI 通过：

字体

间距

颜色

层级

来形成视觉结构。

==================================================
二十二、第十六阶段：性能优化
==============

这一阶段不是：

“把 UI 做丑来省内存”。

性能优化必须优先来自结构。

重点检查：

1. QML/Slint Item 数量
2. TextEdit 数量
3. Text layout 次数
4. 图片缓存
5. string clone
6. model 更新范围
7. database write frequency
8. timer 数量
9. animation 数量
10. redraw frequency

重点目标：

静止页面：

尽量没有持续 CPU 工作。

编辑：

只更新受影响 Block。

滚动：

只处理必要内容。

搜索：

后台执行。

保存：

debounced。

图片：

lazy load。

大型文档：

virtualized。

---

## 长文档测试

必须有：

1000 block

5000 block

10000 block

分别测试：

打开

滚动

输入

删除

移动

搜索

切页

退出

不得只在 100 个 Block 下判断性能。

==================================================
二十三、第十七阶段：内存诊断
==============

如果 RAM 高，不能简单说：

“Slint 就是这么高。”

必须定位：

Rust heap

Slint runtime

text cache

image cache

model

document

database

OS graphics memory

分别分析。

尤其注意：

Task Manager 中：

Memory

GPU Memory

Private Working Set

可能不是同一个东西。

记录时至少区分：

CPU-side memory

GPU-side memory

shared GPU memory

不要把 GPU 显存直接当成普通 RAM。

==================================================
二十四、第十八阶段：Release Packaging
===========================

Windows Release：

* release profile
* LTO
* strip/symbol strategy
* panic strategy 根据需求决定
* debug artifacts 分离

但不要为了让 exe 数字看起来小，而牺牲启动性能或者可维护性。

优先优化：

runtime memory

runtime CPU

startup

而不是：

exe 文件大小

==================================================
二十五、第十九阶段：Crash Recovery
========================

必须考虑：

突然退出

崩溃

断电

数据库写入中断

不能因为最后 1 秒没保存导致整个页面损坏。

至少实现：

atomic transaction

safe migration

backup / journal 策略

启动时 database integrity check。

==================================================
二十六、第二十阶段：Import / Export
=========================

第一版：

Markdown import

Markdown export

Plain text

后续：

HTML

JSON

不要求：

Notion API

Word

PDF

第一版不要做。

==================================================
二十七、第二十一阶段：Windows UX
=====================

Windows MVP 稳定之后再增加：

* native context menu
* file association
* system tray
* global-ish shortcut 中可行的部分
* startup options
* drag/drop files
* clipboard rich content

Slint 1.17 已经加入 drag and drop、system tray、tooltips、model row two-way bindings，可以优先利用这些现成能力，而不是自行造轮子。

==================================================
二十八、第二十二阶段：Android
==================

Windows 稳定以后才开始。

共享：

Document Model

SQLite schema

Command

Undo/Redo

Search

Import

Export

尽可能共享：

Slint UI components

但不要把 Windows UI 强行缩成手机 UI。

Android 需要：

底部/侧边导航重新设计

触摸交互

虚拟键盘

编辑器移动优化

safe area

touch target

但核心模型保持相同。

注意：Slint 当前 Android TextInput 仍有公开 issue/历史兼容性问题，所以 Android 文本编辑必须作为独立测试阶段，不能假设 Windows 文本编辑能力可以 100% 原样搬过去。对 Android 的 IME、光标、选区、删除、Enter、粘贴必须专门测试。

==================================================
二十九、MILESTONE
=============

M0 — Toolchain

完成：

* Rust
* Slint
* Windows build
* Release build
* GPU renderer
* Benchmark baseline

验收：

能启动。

GPU renderer 可确认。

性能数据可记录。

---

M1 — Design System

完成：

* Theme
* Typography
* Colors
* Icons
* Sidebar
* Toolbar
* Popup
* Button
* Context Menu
* Dark/Light

验收：

没有真实数据库也可以截图。

视觉必须达到产品原型级。

---

M2 — App Shell

完成：

* Sidebar
* Page tree
* Main editor area
* Search
* Settings
* page navigation

验收：

可以浏览 mock pages。

---

M3 — Local Document

完成：

* SQLite
* Workspace
* Page
* Block
* Auto save
* Load on restart

验收：

关闭应用再打开，数据完整恢复。

---

M4 — Block Editor MVP

完成：

* paragraph
* heading
* todo
* bullet
* numbered
* quote
* code
* divider
* Enter
* Backspace
* Delete
* Arrow
* Copy
* Paste
* Undo
* Redo

验收：

连续编辑 1000 blocks 不明显卡顿。

---

M5 — Notion Interaction

完成：

* Slash Command
* Command Palette
* Block menu
* drag/drop
* block reorder
* keyboard shortcuts
* selection/focus

验收：

核心交互连贯。

---

M6 — Rich Text

完成：

* bold
* italic
* inline code
* link
* strike
* basic marks

验收：

保存/加载不丢失格式。

---

M7 — Performance

完成：

* virtualization
* lazy rendering
* lazy loading
* debounce saving
* background search
* render optimization

测试：

1000

5000

10000 blocks

验收：

不存在明显的线性资源爆炸。

---

M8 — Windows RC

完成：

* crash recovery
* installer/package
* settings
* import/export
* performance regression
* keyboard shortcuts
* native Windows behavior

验收：

可以作为日常个人笔记软件长期使用。

---

M9 — Android

Windows 稳定以后才开始。

---

M10 — Block 扩充（§三十七 批次 A + B）

完成：

* image（粘贴 / 本地插入 / 附件落盘 / 宽度档）
* file + PDF 缩略图
* table（简单表格）
* toggle
* columns

验收：

图片页与表格页在 10 000 块场景下的 RAM 不劣于现有基线的 1.2 倍；每种新块六处接线齐（types / storage / md io / Turn into / slash / 截图场景）。

---

M11 — Block 扩充（§三十七 批次 C）

完成：

* code 高亮
* bookmark
* embed 卡片
* math
* TOC

验收：

不新增 JS/WASM 运行时；高亮不得让长代码块的输入延迟可感知。

---

M12 — Page 外观与属性（§三十八）

完成：

* icon / cover
* font（default / serif / mono）/ full width / small text
* lock
* version history
* 模板按钮 + 模板库

验收：

schema migration 从旧库升上来不丢数据；锁定页任何输入都不被静默吞掉。

---

M13 — 引用层（§四十）

完成：

* @page mention、@date
* 反向链接区
* synced block

验收：

索引增量维护，打开页面的反向链接计算不随全库规模线性变慢。

---

M14 — Database（§三十九）

按 §三十九 的视图顺序内部再切里程碑：先 table 视图 + 属性模型，再 board / list / calendar，再 gallery / timeline / form / chart，最后 relation / rollup / formula 与 linked database。

验收：

10 000 行库的 RAM、视图切换耗时、公式编辑面板耗时全部进 docs/PERFORMANCE.md；filter/sort 在 SQL 侧。

---

M10 与 M9 的先后：M9 已按 2026-09-20 的决定暂停，所以 M10 起可与 M8 尾巴交错排入。每个里程碑仍须走 §三十 的同一套流程：读架构 → 最小改动 → 编译 → 测试 → 检查 UI → 记录性能影响。

==================================================
三十、Agent 工作规则
=============

你不是一次性写完所有代码。

严格按照 milestone 推进。

每一次修改都应该：

1. 阅读现有 architecture
2. 找到相关模块
3. 修改最小范围
4. 编译
5. 测试
6. 检查 UI
7. 记录性能影响

不要：

一次生成 5000 行代码。

如果某个阶段变得复杂，先拆成子任务。

==================================================
三十一、Agent 的 UI 开发规则
===================

UI 是这个项目的重要竞争力。

你必须愿意多次迭代 UI。

不要因为：

“功能已经能用”

就停止。

要不断检查：

视觉层级

间距

字体

颜色

hover

focus

selected

动画

响应速度

窗口 resize

长文字

中文

英文

高 DPI

125%

150%

200%

---

## 视觉风格

整体目标：

Modern minimal productivity app

而不是：

传统企业软件

也不是：

Material Demo

也不是：

Fluent UI Demo

也不是：

“把 Notion 原样复制”。

---

## 三十二、Agent 的性能规则

任何新增功能都要问：

“它是否会长期占用 CPU？”

“它是否会增加常驻 memory？”

“它是否造成额外 render pass？”

“它是否导致整个 document rebuild？”

“它是否创建大量 UI Item？”

“它是否增加 timer？”

如果答案是“会”，必须说明原因。

---

## 三十三、特别禁止

禁止：

Electron

Tauri

WebView

Chromium

React

Vue

HTML UI

CSS UI

Node runtime

大规模 JS

Qt WebEngine

任何不必要的后台轮询

无限动画

每帧业务逻辑

10000 个永久 TextEdit

每字符 SQLite transaction

整个 Document rebuild

UI 直接 SQL

UI 直接文件 IO

自己实现 Windows IME / TSF

第一阶段实现云同步

第一阶段实现实时协作

第一阶段实现 AI

第一阶段实现插件系统

第一阶段实现发布为公开站点

第一阶段实现评论、讨论与表情回应

---

2026-09-20 澄清：以上六项（云同步、实时协作、AI、插件、发布站点、评论）是唯一被明确排除的能力。§三十七 至 §四十 的块扩充、页面属性、Database、引用层都是要做的，不因 §三十五「功能数量排最后」而被无限推迟——该条只约束「不得为堆功能牺牲 UI 质量与 RAM/CPU」，不构成砍范围的许可。

==================================================
三十四、文档维护
========

必须维护：

PLAN.md

ARCHITECTURE.md

UI_ARCHITECTURE.md

EDITOR_ARCHITECTURE.md

PERFORMANCE.md

DECISIONS.md

每个 Milestone 完成后更新。

DECISIONS.md 记录：

为什么选 Slint

为什么选某个 renderer

为什么不用 WebView

为什么不用 Flutter

为什么 Block Editor 采用当前模型

为什么采用 virtualization

为什么某个依赖被加入

为什么某个依赖被移除

这样以后 Agent 接手项目时，不需要重新推断整个架构。

==================================================
三十五、最终产品目标
==========

最终产品应该具备以下感觉：

打开软件：

启动快。

窗口响应快。

静止状态：

CPU 很低。

页面滚动：

流畅。

长页面：

不会因为 Block 数量增加而发生明显资源爆炸。

输入：

即时响应。

中文输入：

正常。

UI：

现代、克制、漂亮。

Sidebar：

精致。

Command Palette：

快速。

Block：

有良好 hover / selected / focus feedback。

页面：

像成熟知识管理产品，而不是工程 Demo。

最终不要追求：

“功能最多”。

而要追求：

“核心体验非常完整”。

整个项目最重要的优先级是：

1. UI 视觉质量
2. 编辑体验
3. Windows 低 RAM
4. Windows 低 CPU
5. GPU 渲染
6. 架构可维护性
7. 功能数量

不要为了增加功能数量破坏前五项。

==================================================
三十六、第一轮执行要求
===========

现在不要直接开始实现完整产品。

先只完成：

M0 + M1。

也就是：

1. 从 slint-rust-template 建立工程
2. 确认 Slint 当前稳定版本
3. 配好 Windows Release
4. 验证至少两个 GPU renderer
5. 建立 benchmark baseline
6. 建立目录结构
7. 建立 ARCHITECTURE.md
8. 建立 PERFORMANCE.md
9. 建立 PLAN.md
10. 建立 Theme / Colors / Typography / Icons
11. 做出完整 App Shell
12. 做出 Sidebar
13. 做出 Page Tree
14. 做出 Main Editor placeholder
15. 做出 Command Palette mock
16. 做出 Light/Dark theme
17. 完成一次视觉打磨
18. 编译 Release
19. 测试空闲 CPU/RAM
20. 输出 M0/M1 完成报告

完成这些以后再进入 Document Model 和 Block Editor。

不要跳过 M0/M1 直接写数据库和编辑器。

整个开发过程以“可运行、可测量、可回退、可持续扩展”为原则。

==================================================
三十七、第二十三阶段：Block 类型扩充
==============

§九「后续再添加」的那批块从本阶段起进入排期，不再是开放问题。

已交付，不需要再做：

callout（表情 + 底色）

page / link-to-page，共享 blocks.page_ref（ADR-0026）

toggle（批次 B 第一项，ADR-0028：折叠子树零 realized row，row→model 换算缝
`visible_block_indices`）

image（批次 A 第一项，ADR-0029：附件目录 + `attachments` 表、`MAX_EDGE` 降采样
缓存、25/50/100 宽度档位、点击预览。本 kind 唯一未做的仍是**从剪贴板粘贴**，
需要 `platform/` 里的位图读取，见 §三十七 批次 A 的 follow-up）

file（批次 A 第二项，ADR-0030：不加 schema，复用 v7 的 `attachments` 行与
`blocks.attachment` 列；`fs::copy` 流式落盘、全程不解码不设上限，所以 2 GB 附件
与 2 KB 附件占同样的工作集；名字保留扩展名、体积走 `attachment-size` 回调、
Open / Save-as 两个显式按钮（`ShellExecuteW`，不 spawn explorer）；Markdown
导出写成链接而不是图片形状，因此能过导入器往返。PDF 首页缩略图按 2026-09-20
用户指示推迟，未做）

顺序按「日常笔记撞墙的速度」排，不按 Notion 的字母表排。

---

## 批次 A：媒体

image：

* 从剪贴板粘贴、从本地文件插入（rfd 已在依赖里）
* 可选格式就是 `image` crate 特性里显式声明的那四种：png / jpeg / bmp / gif。
  gif 是静图——解码器交回首帧，编辑器里没有动画时钟，也不打算有；其余解码器
  能读的格式（webp / tiff 等）不进选择器，因为它们的行扩展名只能落到 `.img`
  兜底，那属于「能存但说不清」，不做
* 落盘到附件目录，数据库只存引用（attachments 表，§十八已预留）
* 显示宽度可调（25 % / 50 % / 100 %），点击进入预览
* 缓存与降采样计入 §二十二 的性能预算：一张 4000×3000 的原图不得以原始尺寸常驻内存

file：

* 任意文件附件，显示文件名 + 体积 + 打开 / 另存为
* 音视频交给系统默认播放器，Quire 不做播放内核
* 附件字节不得进入进程：流式复制落盘、只记长度，不设体积上限也不解码
  （§二十二 优先于功能数量）

PDF：

* 先按 file 处理 + 首页缩略图；内嵌翻页阅读器不在本阶段
* 2026-09-20 用户指示：首页缩略图这一半先跳过，PDF 目前只走上面的 file 路径，
  所以 `.pdf` 与 `.zip` 除文件名外长得一样；缩略图的渲染路线待定

---

## 批次 B：结构

table（简单表格，不是 Database）：

* N×M 单元格，Tab 跨格，最后一格 Tab 增行，增删行列
* 单元格内是纯文本 + §十 的 inline marks
* 明确不是数据库视图；schema、过滤、排序属于 §三十九

toggle（折叠块）：

* 任何块都能成为 toggle 父级，折叠状态入库
* 折叠掉的子树不得保留 realized row（对齐 §十二 的 virtualization 前提）
* 折叠是视图状态：只入库一个 folded 标志，Markdown 导出照常带上子树（§二十六），导入侧不还原
* 只有 Toggle 画三角：把一个折叠着的块 Turn into 成别的种类，必须顺手展开它，否则子树回不来

columns（分栏）：

* 2 / 3 栏，栏内为块序列
* 用 Slint 1.18 的 FlexboxLayout，不自己写排版
* 与 §十二 冲突时优先保虚拟化：分栏只在可见窗口内展开

---

## 批次 C：引用与嵌入

bookmark：链接卡片，抓标题与 favicon；离线或抓取失败退化为纯链接，且不得阻塞输入

embed：YouTube / Figma / Google Maps 一类链接转卡片。本阶段只做占位卡片 + 外部打开，不做内嵌浏览器（§二 与 §三十三 禁 WebView）

code 高亮：

* 纯词法着色，先覆盖 rust / python / js / ts / md / json / bash
* 不得为高亮引入 JS 运行时

math：

* inline math 与 math block，LaTeX 子集
* 渲染优先 Unicode 近似排版；要引入排版引擎必须先出 ADR 并附内存数字

TOC（目录块）：由当前页 heading 实时生成，点击跳转，派生数据不入库

synced block：依赖 §四十 的引用基础设施，排在它之后

---

## 硬性约束

每加一种块必须同时改到：core/types.rs 的 BlockKind、storage 的 kind 字符串、Markdown 导入导出（§二十六）、⋮⋮ 的 Turn into、slash 菜单、截图场景。

少一处即视为未完成。

两种块还要多改两处，因为它们的行是动态的（2026-09-20 实现 toggle 时确认）：

* projection（`project_blocks`）必须真的把隐藏的子树从 rows 里删掉，不是留一个 `visible: false` 的 delegate
* 凡是拿 row index 当 model index 用的地方（现在是 §八 的拖拽落点）都要做一次 row→model 换算；折叠一发生，这两个编号就不再看同一个位置

==================================================
三十八、第二十四阶段：Page 外观与属性
============

§十七 的 Page Tree 只管结构，不管页面本身长什么样。本阶段补上。

数据前提：pages 表加列（icon / cover / font / layout / locked），走 §十八 的 migration，schema 版本 +1，旧库必须能无损升上来。

## 图标与封面

icon：emoji 选择器 + 本地图片；未设置时用标题首字符占位，侧边栏与页面标题同步显示

cover：本地图片，可换图 / 移除；封面之上的标题对比度必须过 §二十一 的可读性要求，不得用最弱配色

## 页面版式

font：default / serif / mono 三档，按页生效

full width：页级开关

small text：页级开关

三者只作用于当前页的排版 token，不得下沉成 per-block 字号。

## 锁定与版本历史

lock：只读开关。TextInput、slash 菜单、拖拽、⋮⋮ 的编辑项全部关闭，并且给出可见的锁定状态，不能静默吞输入。

version history：

* 复用 §二十五 的 snapshot 机制，不另造一套存储
* 用户可见：命名版本、与当前版本对比、恢复
* 保留策略必须给出磁盘与 RAM 数字，不接受无限增长

## 模板

页面内模板按钮 + 新建页面时选模板

workspace 模板库：预置若干本地模板，导入导出走 §二十六 的 Markdown 通道

模板的表示必须是「块序列的副本」，不得引入第二套内容格式。

## 本阶段不做

依赖账号/成员的属性语义（Person 的协作含义、权限）；评论；发布为站点。

==================================================
三十九、第二十五阶段：Database
=============

这是 Quire 与 Notion 差距最大的一层。

Database 不是 §九 块类型清单里的一行，它自带 model / storage / UI 三层，因此独立成阶段，阶段内再切里程碑。

## 对象模型

database：一组 record + 一份 schema（列定义、视图定义）

record：一行，可以同时是一个 page（页面即行，这是 Notion 的核心而不是装饰）

view：同一份数据的一个投影（过滤 + 排序 + 分组 + 可见列 + 布局）

property：列，带类型

record 与 page 的关系必须可逆：删 record 与删页面的行为都要有明确定义，且都进 undo。

## 属性类型

必做：title / text / number / select / multi-select / status / date / checkbox / url / email / phone / files / created time / last edited time

降级处理：person —— 没有账号体系，退化为工作区内本地成员名单，纯字符串

需计算：formula / rollup / relation（含双向关系）

公式引擎的限制：纯词法 + 自写解释器，不引入 JS / WASM 运行时；表达式必须有限求值；relation 环检测在保存时做，不在渲染时做。

## 视图

table → board → list → calendar → gallery → timeline → form → chart

顺序即实现顺序。chart 放最后，且不得为此引入图表库：先用现有绘制 primitive 做 bar / line / pie 三种。

## 操作

filter / sort / group by / 视图内搜索 / 行内编辑 / 列宽与隐藏列 / 视图切换器；视图与 schema 一起持久化

linked database：引用另一个库的某个视图，不复制数据

数据库模板：新建 record 时的预填

## 性能红线

Database 是本规格里唯一会自然长出「大量行 × 大量属性」的功能，§二十二 / §二十三 的规则在这里最容易破：

* 10 000 行的库不得全量 realize；视图先算可见窗口再取行
* filter / sort 在 SQL 侧完成，不在 UI 侧过滤
* formula / rollup 必须可增量重算，禁止每次输入全库重算
* 数字进 docs/PERFORMANCE.md：10 000 行的 RAM、切换视图耗时、打开公式编辑器的耗时

## 排期前提

本阶段在 §三十七 批次 B（table / toggle / columns）与 §四十 的引用基础设施之后开始，否则简单表格和 relation 会各造一遍轮子。

==================================================
四十、第二十六阶段：引用、提及与反向链接
=============

@page mention：§十 的 Inline Model 新增一种 span（存目标 page id，不存标题）。输入 @ 弹页面选择器，复用 §十五 slash 弹窗的第三种模式（ADR-0026 已验证这条弹窗可复用）。

@date：落 date 型 inline span。

反向链接区：页面底部列出所有引用本页的块。派生数据，不双写入库。

反向链接索引与 §二十 的搜索索引一起增量维护，不得每次打开页面全库扫描。

页面别名：因为引用存的是 ID，重命名后所有引用自然显示新标题。

synced block 建在这一层之上：一个块被多处引用，编辑任意一处全部生效。
