# additional-cli 验证报告

任务：`mf-additional-cli-20261025` / `additional-cli`。实现与验证完成，待 Captain 验收；未提交。

候选以 [final-candidate.json](../.agents/additional-cli/final-candidate.json) 的 canonical JSON payload SHA-256 为准。
父候选 `a54bbba58dcce86c9f0d5f70948eced58dced8270cc8eff876955e7878f38138`；
首包 `ff4e90518d03faf6324fdddc36fa4aac9ae2ecccc27bb5356b89820b90f45fc4`。
开始时逐文件核对父候选，全部匹配；HEAD 保持 `23b047c`。

## 实现

- `mf additional list/show/rename/remove`：未知类别无需登记，文件或整个目录作为一个资源；精确身份可往返，短写歧义返回候选。
- 两布局、根项目及嵌套项目共享归属扫描；核心路径优先，隐藏、工具、构建根和子项目隔离。结构冲突、未知归属、大小写冲突及越界明确拒绝。
- 具体资源 rename/remove 复用原 planner、引用解析、快照、journal、apply 和恢复。category 只移动所选 mind 的文件/目录，不重命名全类别。默认维护受管引用；入站引用阻止删除；dry-run 零写入。
- `mf mind index --check` / `mf output index --check` 比较磁盘、outputs/additional 投影及明确绑定，报告稳定 kind/candidates；有差异 exit 1，一致 exit 0。普通 index 只重建两项投影，不删除磁盘文件或修复权威绑定。
- additional 不自动参与 build/publish/RAG/执行，不推断 Feedback 状态或自动判交付。帮助、手册和当前使用指南同步。

## 最终同源码验证

| 实际命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo build` | exit 0 |
| `cargo test` | 2532 pass / 0 fail / 2 既有 ignored，119 个 test targets |
| `python3 .agents/additional-cli/reproduce.py final-3` | 76 条实际 CLI 命令符合预期 |
| `git diff --check` | exit 0 |

相对父包 2520 pass 新增 11 个真实 CLI 集成测试和 1 个逐持久阶段故障恢复测试。
新增恢复测试覆盖两布局、rename/remove、resume/rollback 及幂等重试；前包转换/恢复、mind/output 和 F16 回归保留且通过。
两个 ignored 仍是 `e2e_boundary_term_lint_rule_filter_actually_filters` 和
`e2e_boundary_term_lint_severity_filter_actually_filters`，未新增 ignored。

CLI 矩阵包括：两布局 × 根/嵌套 × 文件/目录，身份及路径选择、单资源改名的跨 mind 隔离、
Markdown/reference/HTML 引用维护、拒删与零写入、未注册类别、核心/工具/隐藏根排除、
目录不递归生成类别、冲突候选、大小写、未知归属、外部符号链接、过期计划、只读子项目隔离、
投影缺失/过期/不匹配、重复绑定/错属/错角色/缺目标/越界及默认 output 缺失。
合成资料明确包含 `passed: false`，不会据此阻止通用资源操作。

原始日志、逐条命令/exit/stdout/stderr、合成 fixture、源码测试哈希、脚本和历史失败均保留在
[.agents/additional-cli/](../.agents/additional-cli/)（已忽略目录）。
重点入口：[results.json](../.agents/additional-cli/results.json)、
[synthetic-runs.json](../.agents/additional-cli/synthetic-runs.json)、
[tested-files.json](../.agents/additional-cli/tested-files.json)。
首轮新增测试的两处失败是测试对既有 `usage` kind 和 completed transaction 禁止 rollback 的预期不符；
已改正测试以遵守父包契约。首次 clippy 检出测试模块后的函数定义，已调整位置并重跑通过；未隐藏失败日志。

## 本包全部文件

- `README.md`
- `docs/additional-cli-verification.md`
- `docs/additional-cli.md`
- `docs/layout-lifecycle.md`
- `docs/manual.md`
- `docs/mind-output-layout.md`
- `src/cli/access.rs`
- `src/cli/additional.rs`
- `src/cli/lifecycle.rs`
- `src/cli/mind.rs`
- `src/cli/mod.rs`
- `src/error.rs`
- `src/output/mod.rs`
- `src/service/additional.rs`
- `src/service/content_check.rs`
- `src/service/mind.rs`
- `src/service/mind_lifecycle/inventory.rs`
- `src/service/mind_lifecycle/planner.rs`
- `src/service/mod.rs`
- `tests/cli_additional.rs`
- `tests/snapshots/cli_help__top_level_help.snap`

共 21 个文件。父包累计交付文件及各 SHA-256 也全部列入 final-candidate.json 的 `files`。

## 保护绑定与边界

[protection.json](../.agents/additional-cli/protection.json) 绑定 438 个未改动基线文件、
所有允许修改文件的 before/after 哈希，以及父包原始候选/测试/结果/保护绑定文件。
原源稿、历史验证报告、feedback-triage 原证据、F16 实现和测试保持原哈希；当前使用指南的必要更新单列在本包文件清单。
未写 minds/logbooks，未迁移真实资料，未 reset/stash/clean/commit/push/PR/外部发布。

无本包范围内未完成项。既有约束保留：生命周期修改只保证受支持的本地同文件系统和 YAML 权威；
Lance 权威仍明确拒绝，仓外入站引用未核验。只读发现跳过工具与子项目子树；事务仍取完整仓库快照，
所以无关不安全条目也可能阻止事务。目录内隐藏内容不成为新类别，但随资源事务保留。
未实现全项目类别重命名、HTML、Skill 改进或 Feedback 状态 CLI。

实际技能使用：未调用技能；直接使用仓库现有 Rust/CLI 实现与合成测试。未启动子 Agent 或新模型。
