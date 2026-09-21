# 并行 worktree 的实际坐标与踩坑（2026-09-22）

`docs/AGENT_HANDOFF.md` 第 8 行写了「四条同时开就必须各给一个 `git worktree` + 独立
`CARGO_TARGET_DIR`」，但没人落成文档，也没人真开过。现在开了一个，把**实测出来的**
 recipe、数字和两个反直觉的坑记在这里。四条 track 的 agent 都该读这一页。

本节所有数字都是本机（`C:` 953 G / **已用 96 %**，46 G 剩余）在 commit `78ddf35`
上量到的，不是估的。

---

## 1 · 为什么要开，以及它到底隔离了什么

一个 clone 四个 agent 同时改，共享的是**同一个工作树目录**：任何人 `git add`、任何
一次 `cargo fmt`、任何一次 `cargo build` 都会盖到别人正在编辑的文件上。worktree 让每
条 track 有**自己的目录和自己的 `target/`**，只共享 `.git` 里的对象库和 refs。

已经踩到的实例：主工作树在 `78ddf35` 之后同时挂着 T2/T3 的未提交改动（`PLAN.md`、
`docs/DECISIONS.md`、`docs/SPEC.md`、`src/core/mod.rs` + 新文件 `src/core/database.rs`）。
icon 切片收口时，`git add` 前必须先把别人的段落裁掉再还原。**在 worktree 里这类事情
不会发生**：`.scratch/wt/t1` 里 `grep -c "pub mod database" src/core/mod.rs` = 0，
`git status --short` 为空。

注意两点它**不**隔离：

- **refs 是共享的**。分支名全仓库唯一，且**一个分支不能被两个 worktree 同时 checkout**。
  主工作树此刻在 `track/3-database`，所以那条分支你不能再开。
- **`.gitignore` 是共享的，被忽略的东西不是**。`/.scratch`（`.gitignore:4`）里的基线截图、
  种子数据库、既有 worktree，都在**主仓库**的 `.scratch/` 下面，新 worktree 里根本没有。

## 2 · 开一个（照抄即可）

```bash
# 在主仓库根目录
git worktree add .scratch/wt/<tN> -b track/<N>-<slug> master
```

放在 `.scratch/` 下面是故意的：那一层被 gitignore，所以 `git status` 里看不见它，别的
agent 不会误 stage 它。目录名和分支名按你的 track 换。

用完删掉（**先确认自己的活已经提交或推走了**）：

```bash
git worktree remove .scratch/wt/<tN>
git worktree list          # 看还挂着谁
```

别手写删目录；`git worktree prune` 只在目录已经没了的时候清 ref。

## 3 · 坑一：**不要**把 `CARGO_TARGET_DIR` 指到 worktree 外面

`benchmarks/scripts/sweep.ps1` 的两条路径都是**相对当前工作目录**的，不是相对脚本：

- `sweep.ps1:23` — `$shot = "target\debug\quire-shot.exe"`
- `sweep.ps1:2` — `$OutDir = ".scratch/sweep"`

所以一旦 `export CARGO_TARGET_DIR=...` 指到外面，`cargo build --features software
--bin quire-shot` 建出来的东西就不在 `./target/debug/` 下，sweep 会打印
`build it first: cargo build ...` 然后 `exit 1`。看着像没编译，其实是路径断了。

**结论：worktree 里什么都不设，就用它自带的 `target/`。** 这本身就是「独立
`CARGO_TARGET_DIR`」——每个 worktree 天然有自己的 `target/`，不需要额外变量。
（同理，`justfile` 的 `shot` recipe 用 `.\target\debug\quire-shot.exe`，也一样要求默认位置。）

sweep 的输出目录建议显式给：`-OutDir .scratch/sweep-<slice>`。它落在 worktree 自己的
`.scratch/` 下，和主仓库的基线互不干扰。要拿旧基线做 diff，**-Baseline 必须给绝对路径**
（`<主仓库根>/.scratch/sweep33`），因为相对路径会从 worktree 根算起，那里没有 `sweep33`。
`<主仓库根>` 就是你 clone 的那个目录；本文不写死它的绝对路径，下同。

## 4 · 坑二：磁盘只有 46 G，而一个能跑像素闸的 worktree 就要 4.9 G

实测，全新的 worktree：

| 步骤 | 耗时 | 该 worktree 的 `target/` |
|------|------|--------------------------|
| `cargo check --all-targets`（冷） | 2 m 03 s | 1.4 G |
| `cargo build --features software --bin quire-shot` | 3 m 30 s | **4.9 G** |

两步都 `exit 0`、零警告。对比：主仓库的 `target/` 是 **43 G**（`debug` 37 G + `release`
5.9 G）。一个走完整 `just check`（check + test + `build --release`）的 worktree 大概
10–12 G，**四个全开就是 40–48 G，正好超过剩余空间**。所以：

- 只跑 `cargo check` 的 worktree 是便宜的（1.4 G），别在没收口前就 `build --release`。
- 一条 track 一个 worktree，**用完删**；`just clean` 只清当前目录的 `target`。
- 空间紧张时先 `du -sh .scratch/wt/*/target` 看谁最肥，再决定删哪个。

## 5 · 像素基线是跨 worktree 可复现的（已验证，不是假设）

在 `.scratch/wt/t1` 里跑 `default` + `page-icon` 两场，和主仓库的基线逐字节相同：

```
7c4e872e264f2e5d43459f9693c19db9  .scratch/sweep33/default.png
7c4e872e264f2e5d43459f9693c19db9  .scratch/wt/t1/.scratch/sweep-smoke/default.png
e311ea18f180c2c7a2e09a62c938973e  .scratch/sweep33/page-icon.png
e311ea18f180c2c7a2e09a62c938973e  .scratch/wt/t1/.scratch/sweep-smoke/page-icon.png
```

`78ddf35` 这一版的 headless 软件渲染是确定性的。**含义**：你可以把主仓库的 `sweep33`
当 control，在 worktree 里跑 new，两边交替，RAM/像素闸的对照成立；但 `sweep33` 那份
基线本身不会被 clone 进 worktree，需要绝对路径引用。

## 6 · 提交回 master 的规矩（共享工作树里仍然要）

worktree 里提交是干净的（只有一个 HEAD、只有你的改动）。**push 仍然要过 master**：

- 在自己的分支上 commit，然后 `git branch -f master <sha>` + `git push origin master:master`
  把 master 挪过去。**不要在主工作树 `git checkout master`**——那会碰别人正在改的文件，
  brief §2.1 明令禁止 checkout/switch/reset/stash/rebase。
- 快进不了（master 已被别的 track 推前）就 `git fetch` 后 rebase **在自己的 worktree 里**，
  不要 rebase 主工作树。
- 主工作树里那四个整合者文件（`PLAN.md` / `CHANGELOG.md` / `docs/ROADMAP.md` /
  `docs/DECISIONS.md`，brief §2.5）如果四个 track 都往里追加，提交前用「备份 → `sed -n
  '1,Np'` 裁到只含自己那段 → `git add` → 校验 `git diff --cached` 里没有别人的标记 →
  commit → 还原备份」这一套；还原后别人的未提交内容原样还在。
- ADR 号：四条 track 一律**追加在 `docs/DECISIONS.md` 末尾**（§2.4），只有整合者把收
  过来的 ADR 归位到文件头部。

## 7 · 现在的登记

| 位置 | 分支 | 归谁 | 状态 |
|------|------|------|------|
| 主工作树（仓库根本身） | `track/3-database`（HEAD） | 共享，四个 agent 都往里写 | 脏：挂着 T2/T3 的未提交改动 |
| `.scratch/wt/t1` | `track/1-page-appearance` | Track 1（icon 切片收口的这一位） | 干净，`78ddf35`，`target/` 4.9 G，sweep 已跑通 |
| （尚无） | — | Track 2 / 3 / 4 | 建议按 §2 各开一个，别在主工作树里建目录 |

master 与 `origin/master` 都在 `78ddf35`。
