# UI 热更新 / Live Preview

这个项目里 **改一行 `.slint` 就能看到效果** —— 这不是我们自研的能力，而是
Slint 1.18 自带的 **live preview**。本文记录它是什么、怎么开、在这个仓库
里怎么用，以及哪些限制是必须知道的。

状态：**已调研，未接入**（截至 `quire` 0.1.23）。本文是接入前的评估，不是
已生效的流程。

## 为什么要关心这件事

这个 shell 的视觉工作几乎全部由 agent 驱动，而 `.slint` 是**提前编译**进
Rust 产物的：

```
ui/*.slint
  ↓ slint-build (build.rs)
生成的 Rust 代码
  ↓ rustc
libquire rlib
  ↓ link
quire.exe
```

于是「改 3 行 UI → 看效果」的最短路径是：

```
编辑 .slint → cargo run → 等 Slint 编译 + 等 rustc 重编整个 quire crate
→ 链接 → 启动 quire.exe → 才能看
```

实测这个循环不短：`[profile.dev] debug = 1` 之下，一次
`cargo build --bin quire`（只改了 `main.rs`）是 **~4m20s**；改动
`.slint` 触发 `build.rs` 重跑时，Slint 编译加整包重编更长。视觉工作本来
是秒级反馈的事，在这里是分钟级。

代价不只是等待，还有**判断力**：agent 改 UI 之后无法快速自证，只能攒一堆
改动再编译一次，一次错几个地方就得再来一轮。`docs/DECISIONS.md` 里那些
「看着像、看不出来」的判断（焦点、命中区域、控件位置）本质上都是因为没有
快速回路。

## Slint 提供的答案：live preview

Slint 官方的 [Live Preview](https://docs.slint.dev/latest/docs/slint/guide/tooling/live-preview/)
解决的就是这条链路。开启后：

- Slint 编译器**不再提前把组件编译成原生代码**，而是生成一个走 Slint
  解释器（`i-slint-interpreter`）的接口；
- 运行中的应用监视磁盘上的 `.slint`，**改动即自动重载**；
- **业务逻辑保持连接** —— properties、callbacks、models 在重载中被保留，
  所以可以一边改 UI 一边点真实的数据；
- `.slint` 有语法错误时，**屏幕上保留上一个版本**，直到你改好。

它和 VS Code 扩展、Slint LSP、slint-viewer 是**同一套解释器**，所以预览里
看到的和跑起来的一致。

### 怎么开（Rust）

需要 **cargo feature + 环境变量**两样：

```
SLINT_LIVE_PREVIEW=1 cargo run --features slint/live-preview
```

两个都容易踩坑：

- `live-preview` **不要**写进 `Cargo.toml` 的 `[features]`。官方明确要求用
  命令行 `--features` 传，这样它不进常规构建。
- `SLINT_LIVE_PREVIEW=1` 不设的话，`build.rs` 那边不会切到解释器路径，白搭。
  我们 `build.rs:6` 已经为 `QUIRE_PROBE` 写了
  `cargo:rerun-if-env-changed=…` 的样板；`slint-build` 自己会为
  `SLINT_LIVE_PREVIEW` 发 `rerun-if-env-changed`
  （`slint-build-1.18.1/lib.rs:553`），所以**不用改 `build.rs`**。

前提条件在本仓库已经满足：

- 我们 pin 的是 `slint = "1.18"`，而 `slint/live-preview` 这个 feature 在
  1.18 的 `Cargo.toml:130` 里就有（`live-preview = ["dep:i-slint-live-preview"]`），
  版本与 winit backend 严格一致（都是 `=1.18.1`）。
- 应用是 `slint::include_modules!()`（`src/lib.rs:19`）+ `slint_build::compile`
  （`build.rs:8`），正是官方 live preview 针对的那种形态。

代价是**构建时间更长、运行时更慢**（解释器而不是 AOT 代码），所以它只适合
开发期。

## 另一条路：slint-viewer

[`slint-viewer`](https://docs.slint.dev/latest/docs/slint/guide/tooling/slint-viewer/)
是官方独立工具，`cargo install slint-viewer`，不碰我们的代码就能开
`.slint`：

```
slint-viewer --auto-reload ui/AppWindow.slint
```

对本项目**不是**一个好方案，原因是具体的：

1. `AppWindow.slint` 依赖 `UIState` 全局、`StdWidgets`（`ScrollView` /
   `ListView`）、`@image-url` 资源和一整套 Rust 侧喂进来的 model。viewer
   没有 Rust 那些 model，渲染出来的是一个空壳 —— 恰恰是最不适合拿来判断视觉
   的那种「看」。
2. `--load-data` / `--save-data` 能传 property，但只对能序列化成 JSON 的
   public property 有效，喂不动 `ModelRc<VecModel<…>>`。
3. 它**不认识这个 app 的主题与状态机**：`Colors.themes` 的 12 套调色板、
   `Theme.theme` 的回落规则、当前打开的页面 —— 都要手写 fixture 才有一致性。

所以 viewer 适合的是「一个自包含组件长什么样」，而我们的组件几乎都不自包含。
**结论：viewer 不作为本项目的视觉回路**；要回路就用 app 内的 live preview。

## 与既有设施的关系

这个仓库已经有两个跟「快速看 UI」有关的东西，live preview 的位置要跟它们
分清：

- **`just shot`**（`benchmarks/scripts/shot2png.ps1`）：headless 软件渲染出
  PNG，供人**事后**比对。它是回归工具，不是迭代工具 —— 改完再跑，跑完再看。
- **`--quick-note`**（`main.rs`）：跑真实事件循环的验证门。它能证明一个按钮
  真的能被点开，但要看「好不好看」，还是得用眼睛看屏幕。

live preview 补的是这三者之间的空档：**改完立刻看**。

## 建议的接入方式（还没做）

如果要接，我倾向这样，而不是把 live preview 编进默认构建：

1. 加一个 just recipe，例如 `just ui-live`，只做一件事：
   `SLINT_LIVE_PREVIEW=1 cargo run --features slint/live-preview`。
   不碰 `Cargo.toml` 的 `[features]`，不碰 `build.rs`。
2. 它的 target dir 要和默认构建**分开**（比如 `target-live`），理由和
   `just shot` 用 `target-shot` 完全一样（ADR-0134）：live-preview 是另一个
   feature set，混在一个 target dir 里会各自留一份，再叠出几十 GB。
3. 明确它是开发期设施：`just check` / `deploy-workshop` / `release-publish`
   都不带这个 flag，release 绝不能开着它（官方文档：不要 ship 开着的构建）。

## 尚未验证的部分

诚实标注，避免把调研当结论：

- **没有在本项目上跑过一次 live preview。** 上面所有关于 Slint 行为的描述
  来自官方文档和 1.18 的 crate 源码；「我们的 `ui/` 在解释器下能正常渲染、
  `StdWidgets` 与 `@image-url` 都工作」是**待验证**的假设。
- 12 套主题、`always-on-top` + `no-frame` 的 速记窗口、托盘（native、非 Slint）
  这些在重载时的表现未知。托盘本来就是 native 的，理论上不受影响，但没测过。
- 重载是否保住 `UIState` 里的当前页面 / 选中行 / 滚动位置，官方说 properties
  与 models 会被保留，但我们的 model 很大，未测。
- 实测的编译时间基线只有一次 `cargo build --bin quire`（~4m20s），
  不足以代表「改一行 `.slint`」的完整循环；接进来之后应该重测并把数字记到
  `docs/PERFORMANCE.md`。

## 参考

- [Live Preview for Rust and C++](https://docs.slint.dev/latest/docs/slint/guide/tooling/live-preview/)
- [Slint Viewer](https://docs.slint.dev/latest/docs/slint/guide/tooling/slint-viewer/)
- 本仓库的编译成本讨论：[ADR-0134](DECISIONS.md)（`debug = 1`、per-feature-set
  target dir）、`docs/PERFORMANCE.md`