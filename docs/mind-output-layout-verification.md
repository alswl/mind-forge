# mind/output 布局实施包验证

任务：`mf-model-layout-20261025` / `mind-output-layout`。工作树：
`feedback-plan-20261025`。本包实现已落盘，未 commit/push/PR，供 Captain 独立验收。

## 候选与边界

基线提交：`23b047c65151de670098a197c89612b88d594cad`。
候选 SHA-256：`ff4e90518d03faf6324fdddc36fa4aac9ae2ecccc27bb5356b89820b90f45fc4`。
摘要覆盖 35 个源码、测试、快照及产品文档文件（包含原已验收 F16 修改）；
不包含本报告和受保护历史文档。逐文件 hash 和计算输入保存在本地忽略目录
`.agents/mind-output-layout/candidate.json`。

新增 `mind` / `output` 创建、列表、展示和索引，配置采用
`mind.yaml.content_layout` 与 `minds` 登记；精确身份为
`<project>::<mind>[::<output>]`。category/bundle 共用发现与定位，输出不登记成
独立 mind。Markdown 构建和发布共用定位；构建产物为
`build/<mind>/<output>.md`，源稿不会被构建覆盖。实现契约见
[mind/output 布局](mind-output-layout.md)。

现有未明确归属的 Article 项目不能通过新命令隐式迁移；新项目需显式选择布局。
新布局项目内的旧 Article 操作被拒绝，避免两套对象模型混写。

## 验证矩阵

| 观察点 | 实际测试 |
| --- | --- |
| 零/一/多 output，两布局、两形态 | `tests/cli_mind_output.rs`：创建、默认选择、缺失默认、零 output 构建失败 |
| 精确身份往返、中文名字、同仓混用、嵌套项目 | CLI list→show、精确 identity→build/publish/search；项目配置与索引保留注册路径 |
| 歧义与冲突 | 稳定 `selection_ambiguous` + candidates；同名文件/目录、大小写、保留名、路径越界、短名称遮蔽拒绝 |
| 无写入预览/查询 | 对整个合成仓库的路径及文件字节快照比较；包含空目录、锁、缓存新增检查 |
| 源稿与 private | 两布局×两形态实际 build；callout 和非标题 private block 排除，源稿字节保持 |
| 发布定位 | 本地临时目录实际 publish；mind/output 双层目标名；publish update 保存实际源路径 |
| RAG 定位 | CLI 实际 metadata search；单元验证两布局源稿 registration + acquisition，包括 private 源稿保留、不索引 build |
| 路径安全与发现 | 单元覆盖符号链接 block/build 父目录、工具根排除、仅登记的零 output mind |
| F16 | 原 17 项 `cli_publish_update` 及完整 publisher e2e；目录路径解析仍使用既有 resolver |

高级语义检索仍依赖既有 embedding/backend 配置；本包没有连接真实 embedding 服务，
没有执行真实资料迁移或外部发布。本地 publish 仅使用测试临时目录中的合成数据。

## 命令与结果

最终命令结果见下表；原始 stdout/stderr 和耗时保存在
`.agents/mind-output-layout/command-results.json` 及同目录 `accepted-*.log`。

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked --quiet --no-fail-fast` | exit 0；117 个测试目标，2479 passed、0 failed、2 既有 ignored |
| `cargo test --locked --quiet --test cli_help --test cli_mind_output --test cli_publish_update --test e2e` | 最终帮助文字调整后复测：exit 0；236 passed、0 failed、2 既有 ignored |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo build --locked` | exit 0 |
| `scripts/generate-manual.sh` | exit 0；手册已重新生成 |
| `git diff --check` | exit 0 |

完整测试后仅调整 CLI 帮助文字、对应快照和生成手册；最终源码的四个相关测试目标
再次通过，fmt/clippy/build 也再次通过。该轮记录见 `post-help-results.json`。
新增覆盖为 4 个单元测试、6 个 CLI 场景和 1 个混合布局本地发布 e2e；它们包含
循环运行的两布局×两形态及多 mind/output 场景。

首轮完整测试的两项失败仅为新增命令/帮助说明导致的预期快照差异；已逐项检查并更新。
两项既有 ignored 测试均为 term lint rule/severity 语义延后项，不属于本包。
生成手册使用 `scripts/generate-manual.sh`，没有手工修改 `docs/manual.md`。

## 变更文件清单

| 分组 | 文件 |
| --- | --- |
| 新模型与配置 | `src/model/mind.rs`、`src/model/config.rs`、`src/model/mod.rs`、`src/service/config.rs` |
| 发现/定位/索引 | `src/service/mind.rs`、`src/service/mod.rs` |
| CLI、输出和错误 | `src/cli/mind.rs`、`src/cli/mod.rs`、`src/cli/project.rs`、`src/cli/article/mod.rs`、`src/cli/build.rs`、`src/cli/publish.rs`、`src/error.rs`、`src/output/mod.rs` |
| 构建/发布/布局检查 | `src/service/build.rs`、`src/service/publish.rs`、`src/service/project/lifecycle.rs` |
| 检索接入 | `src/service/source/advanced/catalog.rs`、`retrieval.rs`、`sync.rs` |
| 新测试与帮助快照 | `tests/cli_mind_output.rs`、`tests/e2e/mind_output_layout.rs`、`tests/e2e/main.rs`、`tests/snapshots/cli_help__{article_help,build_help,publish_help,publish_run_help,top_level_help}.snap` |
| 产品文档 | `README.md`、`docs/glossary.md`、`docs/schema-naming.md`、`docs/manual.md`、`docs/mind-output-layout.md`、本报告 |

原有 `tests/cli_publish_update.rs`、`tests/e2e/publisher.rs` 的本轮前未提交修改原样保留。
`src/service/publish.rs` 的 F16 分支继续保留，并增加受 `content_layout` 控制的有界适配。
所有 `docs/feedback-*.md`、`docs/feedback-triage-evidence/` 和
`docs/publish-update-fix-verification.md` 与本轮开始时的 hash 一致。

## 未做与交接

未实现后续包的迁移/转换、rename/move/delete、关联引用维护、恢复 journal、
additional 管理、HTML、Skills 改进/eval。未操作真实知识库、未写 logbooks/minds、
未读取旧 report.html 或混合业务反馈、未提交或发布外部制品。

实际使用技能：`mf-cli`。未调用其他模型或子 Agent。用户提供的 wrapper 标识为
`codex-glinkap-astra`，配置 digest 为
`410d03fabfeacf0219c57353baf710862027d12ddd7e4030f377523e33d8a0cc`；
本包未重启或改变会话模型配置。独立验收交由 Captain 接续。
