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
