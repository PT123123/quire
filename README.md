# Quire

A local, GPU-accelerated, Notion-like document workspace.
Rust + Slint. Single process. No Electron, no WebView, no web stack.

```
just build                    # cargo build --release (default FemtoVG renderer)
just run                      # cargo run (debug)
just check                    # local CI replacement: check + test + release
just shot menu                # headless visual shot (scene name optional)
just clean                    # cargo clean + remove skia/wgpu benchmark target dirs
```

Plain cargo still works as before:

```
cargo run                     # debug, FemtoVG renderer
cargo run --release           # release
cargo run --no-default-features --features skia --release   # Skia build
```

- Full requirements (original spec, zh-CN): `docs/SPEC.md`
- Status & roadmap: `PLAN.md`
- Why it looks like this: `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`
- Measured numbers: `docs/PERFORMANCE.md` (`benchmarks/scripts/bench.ps1`)

## 核心目标（浓缩版）

现代、轻量、GPU 加速的本地 Notion-like 文档应用。第一平台 Windows，之后 Android。
优先级：UI 视觉质量 > 编辑体验 > 低 RAM > 低 CPU > GPU 渲染 > 可维护性 > 功能数量。

**技术底座**：Rust + Slint 1.18.x（MSVC），单进程；UI 层（`.slint`：视觉/布局/交互）与
Rust Core（文档模型、Undo/Redo、SQLite 持久化、搜索、命令分发）严格分离。
UI 不直接碰数据库或磁盘 IO。Renderer 需实测对比 FemtoVG·wgpu 与 Skia。

**架构原则**：
- Block Tree 文档模型（稳定 ID + order），不用巨型字符串存页面
- 所有编辑操作走 Command 系统，Undo/Redo 基于内存模型而非数据库
- Block 渲染虚拟化：只有焦点 Block 使用真正的 TextEdit，非可见块不占昂贵 UI 资源
- 自动保存：dirty → debounce → 批量写 SQLite（事务），杜绝每字符写库
- 中文输入依赖 Slint 文本设施 + 系统 IME，禁止自研 TSF/IME
- 动画“有反馈但不持续运行”，静止界面 CPU 应接近 0

**明确不做（第一版）**：云同步、协作、AI、插件市场、Electron/WebView/任何 Web 栈。

## Milestones

| | 内容 | 状态 |
|--|------|------|
| M0 | 工具链、GPU renderer 验证、benchmark 基线 | ✅ |
| M1 | 设计系统（Theme/Colors/Typography/Icons）+ 产品级 App Shell | ✅ |
| M2 | App Shell：Sidebar / Page Tree / 编辑区 / 搜索 / 设置 | ✅ |
| M3 | 本地文档：SQLite、自动保存、重启恢复 | 待办 |
| M4 | Block Editor MVP（Enter/Backspace/合并/拆分/Undo…） | 待办 |
| M5 | Notion 交互：Slash 菜单、Command Palette、块拖拽 | 待办 |
| M6 | 富文本 inline marks（bold/italic/code/link…） | 待办 |
| M7 | 性能：虚拟化、懒加载、后台搜索（1000/5000/10000 blocks） | 待办 |
| M8 | Windows RC：crash recovery、打包、导入导出 | 待办 |
| M9 | Android（共享核心模型，UI 重新设计） | 待办 |

First release (Windows): block editor, local SQLite storage, command
palette, Chinese input via the OS IME — nothing cloud, nothing sync, yet.
