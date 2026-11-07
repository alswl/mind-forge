# publish update 路径定位修复交接

> 公开副本说明：历史记录中的用户目录、内网审阅地址及私有修订标识已脱敏；示例 loopback URL 不表示当前有服务。本文记载原运行结果，本次发布未据此宣称重新验收。

任务 `publish-update-fix`，dispatch `mf-publish-fix-20261025`，2026-10-25。仅执行 Stage 01 第二项 F16 定位小修，未修改协调日志或开始 Stage 02。

## 候选与实现

候选 **publish-update-ad7887bc223f**，基线 `23b047c65151de670098a197c89612b88d594cad` 加本工作树差异；未提交。

- 补丁 SHA-256：`ad7887bc223f25e8eda122bebdacf0076bd1a18abc300dfa2c363fbb61dc7039`。
- 最终 `target/debug/mf` SHA-256：`4fcc8c78cfbd61d2fcfbfc4c92d3389a55e9286fbf21d1b03c1533a68c4bf313`。
- 新编译版本为 `0.4.2-dev+23b047c`，build_date `2026-10-25`、rustc 1.96.0、aarch64-macos。版本串不包含工作树差异，识别候选须同时使用补丁及二进制哈希，不能只看 commit 字段。
- 文件哈希与候选补丁在本地 `.mind-forge/publish-update-fix/candidate.json` / `candidate.patch`。

`src/service/publish.rs` 的 update 原先拒绝 `/`，再自行匹配 `<articles>/<slug>.md`。现在允许 project-relative 路径，调用已有 `index::resolve_article`，将返回的 **article_path** 用作发布记录 path；不把 canonical index key 或标题误当正文身份。空输入和反斜杠的拒绝仍保留，相关 hint 改为提示 slug／indexed path。

没有修改共享 resolver；没有添加自动 reindex。因此 dry-run 不会借定位刷新索引。后续目标校验、upsert、status/URL/published_at、JSON/text 输出结构、记录唯一性保持原逻辑。未知文章的错误内容来自共享 resolver，仍为 not_found；不另造诊断协议。

变更量：三个代码／测试文件，125 行新增、15 行删除；生产源码仅 3 行新增、15 行删除。CLI 定义和 help 未变，无须重新生成 manual。

## 修复前失败 → 修复后通过

在生产源码未改动时先新增回归，运行：

```sh
cargo test --locked --test cli_publish_update canonical_selectors
cargo test --locked --test e2e directory_article_indexed_and_found_by_publish_update
```

第一条 exit 101：3 个新增测试全部失败。canonical 单文件路径得到 usage / `invalid article name: 'docs/my-article.md'` / `article must be kebab-case with no path separators`，目标校验测试也被这道路径拒绝提前阻断。第二条 exit 101：目录文章完成 index 后，实际 update `docs/my-dir-article` 返回同样 usage，预期 0、实际 2。此前已验收 triage 另有目录 slug not_found 的原始运行证据；未重写这些原始结论。

现有 e2e 名称声称覆盖 publish update，但原测试只执行两次 index。本次补配置 local 目标、真实调用 update、断言返回 canonical path/created，并验证没有发布产物。

修复后实际执行：

| 命令 | 结果与适用范围 |
| --- | --- |
| `cargo test --locked --test cli_publish_update --test cli_publish_run --test cli_publish_generated --test cli_publisher_list` | exit 0，17 + 70 + 5 + 14 = **106 passed**。覆盖新增文件／目录 path 与 slug、同记录更新、不刷新 published_at、dry-run 全树字节不变、目标与字段前置校验；既有 run、生成文章和 publisher 发现回归通过。 |
| `cargo test --locked --test e2e publisher::` | exit 0，**7 passed**，含补强的目录 index→实际 update 闭环；159 个其他 e2e 未运行。 |
| `cargo test --locked --bin mf service::index::tests::resolve` | exit 0，**11 passed**，共享 resolver 与 Prompt 绑定相关现有单测；870 个其他单测未运行。 |
| `cargo clippy --locked --all-targets -- -D warnings` | exit 0，全 targets 无警告；包含新增测试。 |
| `cargo fmt --check` / `git diff --check` | 均 exit 0。 |
| `cargo build --locked` | exit 0；最终二进制位于本隔离树 target/debug/mf，未安装替换 PATH 上的 mf。 |
| `python3 .mind-forge/publish-update-fix/replay.py` | 最终二进制重放原核验 publish-update fixture：single、docs/single.md、chunked、docs/chunked，各 dry-run／实际，共 **8 次通过**；canonical record.path 正确，dry-run 前后全树字节相同。只更新合成记录，不发布正文。 |

前两次失败是预期 red 证据；修复后没有未通过检查。首次构建下载锁定依赖并在本树编译，没有升级工具或改 Cargo.lock。按 `docs/build-workflow.md` 的小改定向验证执行；本包无推送，未跑全仓测试、完整 e2e 或跨平台发布构建，不宣称这些通过。原始日志在忽略目录，摘要在此供验收。

## 技能实际使用

- 本仓 `skills/mf-cli/SKILL.md`：核对 publish update 的 `--set`、target、兼容 status/target-url、dry-run 与 JSON 契约；以 `docs/glossary.md` 的路径身份区分 record.path 和展示标题。实际用于构造 CLI 回归和重放，避免将 publish run 与记录更新混为一谈。
- `/home/example/.codex/skills/diagnosing-bugs/SKILL.md`：先建立实际 CLI 的失败反馈，再修复、回归、重放原 fixture。此前多次合成复现已排除版本／调用方问题，本轮 red 又精确命中路径拒绝和单文件拼接；根因简单明确，因此明确省略 3–5 假设、二分和插桩，不为满足形式增加复杂度。没有新增调试日志或 Agent。

## 完整文件清单与提交边界

建议未来业务提交只包含本次三个实现／测试文件（当前未 stage、未 commit）：

- `src/service/publish.rs`：替换 update 定位分支，保留后续处理。
- `tests/cli_publish_update.rs`：新增 3 个矩阵回归及中性 fixture helper。
- `tests/e2e/publisher.rs`：补齐原目录用例的实际 publish update 验证。

原有文件全部原样保留，不作为本次修复新增内容：

- `docs/feedback-triage-design.md`：原有未跟踪草稿。
- `docs/feedback-triage-verification.md`：已验收的原始核验报告。
- `docs/feedback-triage-evidence/after-convert.json`
- `docs/feedback-triage-evidence/after-move.json`
- `docs/feedback-triage-evidence/after-rename.json`
- `docs/feedback-triage-evidence/final-fixture.json`
- `docs/feedback-triage-evidence/inputs.json`
- `docs/feedback-triage-evidence/publish-run.json`
- `docs/feedback-triage-evidence/publish-update.json`
- `docs/feedback-triage-evidence/reproduce.py`
- `docs/feedback-triage-evidence/runs-initial.json`
- `docs/feedback-triage-evidence/runs-second.json`
- `docs/feedback-triage-evidence/runs.json`
- `docs/feedback-triage-evidence/sha256.json`
- `docs/feedback-triage-evidence/source-after-missing.json`
- `docs/feedback-triage-evidence/source-before-missing.json`
- `docs/feedback-triage-evidence/supplement.json`
- `docs/feedback-triage-evidence/supplement.py`

本次交付入口为 `docs/publish-update-fix-verification.md`（本文件，新增、未跟踪）；属于本地交接摘要，不自动纳入未来业务提交。

本次本地过程材料全部位于被仓库忽略的 `.mind-forge/publish-update-fix/`，不纳入未来业务提交：
- `.mind-forge/publish-update-fix/build.log`
- `.mind-forge/publish-update-fix/candidate.json`
- `.mind-forge/publish-update-fix/candidate.patch`
- `.mind-forge/publish-update-fix/clippy.log`
- `.mind-forge/publish-update-fix/fmt.log`
- `.mind-forge/publish-update-fix/green-e2e.log`
- `.mind-forge/publish-update-fix/green-publish.log`
- `.mind-forge/publish-update-fix/green-resolver.log`
- `.mind-forge/publish-update-fix/original-artifacts.sha256.json`
- `.mind-forge/publish-update-fix/red-e2e.log`
- `.mind-forge/publish-update-fix/red.log`
- `.mind-forge/publish-update-fix/replay.json`
- `.mind-forge/publish-update-fix/replay.py`

此外 `target/` 是被忽略的本树 Cargo 构建目录；合成 fixture 使用系统临时目录，replay.json 记录准确 cwd。构建 cache／依赖获取由 Cargo 管理。没有新增其他业务文件。后续如需打包证据，应单独选择过程材料，不能将整个工作树直接提交。

## 验收与保留风险

本包要求的“修复前真实失败 → 修复 → 通过回归 → 原 fixture 重放”闭环满足，供 Captain 验收。原草稿、核验报告及 16 个原始证据文件共 18 份全部通过修复前后 SHA-256 一致性核对；来源和协调仓未修改。

共享 resolver 当前支持范围直接沿用：本包不承诺尾斜杠／block 文件路径等额外选择器，不改其歧义判定规则。publish update 仍仅操作已索引文章；缺索引需按现有提示处理。转换后旧发布记录不会自动重绑或去重。本修复不改变覆盖默认、发布新鲜度、原子迁移、布局及诊断协议；这些留 Stage 02。F18/F19 仍待核，不表示问题解决。

没有 commit/push、PR、外部发布、工具升级或对原报告的修订。到此停在本包，下一项由 Captain 另行派发。
