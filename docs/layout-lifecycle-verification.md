# layout-lifecycle 验证报告

任务 `mf-layout-lifecycle-20261025`，基于已验收首包 `ff4e90518d03`，续接事务层断点
`e60a1dd19aed`。本包实现及本地验证已完成，独立验收由 Captain 接续。
没有迁移真实知识库、修改外部 logbook/设计、commit、push、PR、发布或启动子 Agent。
使用 `mf-cli` 技能核对命令契约与生成手册。

用户契约及命令见 [layout-lifecycle.md](layout-lifecycle.md)。公开入口为
`mf lifecycle convert/mind-rename/mind-move/mind-remove/output-rename/output-move/output-remove`，
配套 `apply/status/resume/rollback`。additional 本包仅发现并随所属 mind 保留，未增加独立管理命令。

## Captain 验收返修

历史候选 `ad300fef9a0f` 及其 2513 项通过证据保留在
`.agents/layout-lifecycle/accepted-review-ad300fef9a0f/`，原始 final 文件亦未覆盖。
本次新增结果及完整文件绑定在 `revision-1/`；当前候选以该目录 manifest 为准。

- 移除根项目的未经批准限制：`-p .`、`::mind` 身份支持两布局转换、mind/output
  改名/移动/删除。根与子项目互移维护引用及 YAML 权威；子项目不变成根项目的 mind。
- 默认 `convert --yes` 在独立私有目录完成备份后原子发布 journal。
  准备缺失/截断时普通 resume/rollback 保留证据并安全终止准备；完整备份可 resume。
  不依赖外部保存计划，不在业务零变化时永久阻塞；后续编辑保持或明确冲突拒绝。
- 首包 [固定映射契约](mind-output-layout.md) 中 `layout.*` 属于旧 Article 配置，
  不是新模型核心角色覆盖。已测试仅配置自定义旧根可双向转换；含未分配旧内容明确拒绝，
  不偷偷跳过。隐藏占位往返修复保持。

## 同一候选与保护绑定

候选完整文件摘要及 SHA-256 存于本地忽略目录
`.agents/layout-lifecycle/revision-1/final-candidate.json`；摘要随最终回传提供。
`.agents/layout-lifecycle/revision-1/final-tested-files.json` 绑定全部源码、测试和 Cargo 文件，
检查前后摘要一致，见 `final-test-binding.json`。报告文件不反向嵌入自己的摘要。

首包基线 56 个文件中，48 个逐字节未变，8 个为必要增量修改：
`src/service/mind.rs`、`docs/manual.md`、`src/cli/mod.rs`、`src/error.rs`、
`src/model/config.rs`、`src/service/config.rs`、`src/service/mod.rs`、
`tests/snapshots/cli_help__top_level_help.snap`。无文件丢失。F16 的源码、原测试和
历史验证文件保持基线 hash。完整绑定见 `final-baseline-binding.json`。

本包文件清单（含续接的事务层修改）：

| 用途 | 文件 |
| --- | --- |
| 事务及计划 | `src/service/mind_lifecycle/{mod,transaction,inventory,references,planner}.rs` |
| CLI 与统一锁 | `src/main.rs`、`src/cli/{mod,access,lifecycle}.rs` |
| 配置与接入 | `src/model/config.rs`、`src/service/{config,mind,mod}.rs`、`src/error.rs` |
| 依赖 | `Cargo.toml`、`Cargo.lock`（事务层直接使用 libc 查询文件系统） |
| 测试 | `tests/cli_layout_lifecycle.rs`、`tests/snapshots/cli_help__top_level_help.snap` |
| 文档 | `docs/manual.md`、`docs/layout-lifecycle.md`、`docs/layout-lifecycle-progress.md`、本报告 |

## 实际命令结果

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo build --locked` | exit 0 |
| `cargo test --locked --quiet` | exit 0；118 suites，2520 passed、0 failed、2 既有 ignored |
| `git diff --check` | exit 0 |
| `scripts/generate-manual.sh` | 使用当前构建产物生成手册，成功 |

本包 23 项事务/清单/引用/计划器单元测试、1 项命令访问策略测试及 17 项 CLI
合成测试通过。完整回归同时包含首包、原 17 项 `cli_publish_update` 与 e2e/F16。
历史 ad300 候选的 12 次实际 CLI 调用的参数、退出码、完整 JSON、合成根路径及二进制摘要保存在
原目录 `command-evidence.json`，包含预演、转换、改名、引用删除拒绝和状态查询。

## 覆盖矩阵

- category→bundle→category，零/多 output、单文件/分块、prompt/thinking、二进制
  Asset、Source、动态 additional、隐藏角色文件及用户占位文件保留。
- mind/output 改名、跨项目 mind 移动、output 跨 mind 移动与删除；标题保持；默认
  output 跟随改名，移动/删除时清除；Source/Asset YAML 登记和发布记录随路径移动。
- Markdown 链接/图片/reference definitions、HTML src/href 默认维护；代码、链接
  title、CRLF 和末尾换行保留；有入站引用拒绝并报告引用者；整个删除集合内部互引豁免。
- dry-run 字节级零写入；新增入站文件、新 additional、未知旧稿归属、目标冲突拒绝。
- 各写命令族受同一根目录独占锁保护；普通读写遇到 prepared journal 拒绝；恢复入口可用。
- 实际创建 Lance 表的 fixture 验证明确拒绝，未更改业务快照或创建生命周期 journal。
  不宣称支持 Lance 权威迁移/恢复。

## 故障注入与恢复

完整转换 fixture 有 19 个变更路径；备份后及每个变更后共 20 个阶段，分别执行
resume/rollback，共 40 组。该 fixture 同时包含源稿、引用、Source/Asset 登记和发布记录。
事务原语另覆盖最终 journal 标记后的故障、恢复重试幂等、rollback 再中断、后续编辑/
新增文件保护、错误 worktree、计划篡改、完整 `.next` 镜像恢复。

新增根项目全转换 fixture 的每个持久变更阶段 resume/rollback 幂等验证。
默认 CLI 不保存外部计划，分别注入 namespace、directory、partial、backup、publish
五个准备阶段，并执行 resume/rollback（10 组）；覆盖空准备、截断备份、完整备份、
发布后恢复及原命令重试。另测不完整准备后的编辑保留、完整备份后的编辑冲突拒绝。
旧 active journal 的 `--plan` 修复测试保留，但默认新路径不再依赖该选项。

新目录先在私有区设置最终权限再原子
移入，测试覆盖遗留 staging 目录权限中间态；元数据符号/硬链接及非普通文件拒绝。

这是持久操作阶段注入，不是每个系统调用、真实断电或内核 fsync 故障的覆盖。
未覆盖远程文件系统、跨设备迁移、真实旧库归属导入、仓外入站链接和非协作外部编辑竞争。
这些不作成功保证。目录权限、未知引用语法和跨项目跨布局无法映射的 bundle 顶层
隐藏文件按文档明确拒绝，不猜测映射。运行中的服务调用若绕过 CLI，不宣称自动持锁。

## 过程中发现并修复

最初全 CLI 锁接入在参数校验失败时创建锁文件，破坏首包零副作用断言；已改为根目录
inode 锁，相关回归及最终全量通过。reference definition 的跨行扫描曾漏改前文链接，
已限制标签边界并加入回归。另补齐 bundle 顶层隐藏占位文件保留后的反向转换：
仅复用空目录或仅含隐藏项的目录，不吸收未知可见文件。非标准配置下含内容的旧核心根也明确拒绝，避免被当成保留工具根跳过。
先前事务层锁释放竞争的修复保留。

原始日志及失败过程位于 `.agents/layout-lifecycle/`，包括 `guard-regression.log`、
`reference-tests.log`、`final-results.json`、`final-full.log`、`fault-coverage.json`。
首包及旧断点证据未覆盖。

本次完整检查前 clippy 指出三处多余借用，已修正；失败日志保留在
`revision-1/initial-checks/`。本次原始日志、实际命令和退出码见
`revision-1/final-results.json` 与对应日志；检查期间源码/测试摘要不变。
