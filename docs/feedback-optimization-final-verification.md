# Feedback optimization：final-e2e

> 公开副本说明：历史记录中的用户目录、内网审阅地址及私有修订标识已脱敏；示例 loopback URL 不表示当前有服务。本文记载原运行结果，本次发布未据此宣称重新验收。

本包验证通过，供 Captain 首次独立验收；未提交、未发布外部制品。
**审阅入口：[局域网示例与报告](http://127.0.0.1:62578/)**。
入口中的示例 HTML 是实际 `mf build --format html` 产物，未用 Pandoc 替代。

## 当前结果与证据层次

| 层次 | 本次结果 | 证据 |
| --- | --- | --- |
| 当前源码完整回归 | `cargo test --all-targets`，120 个目标，2536 passed、0 failed、2 既有 ignored | [完整日志](../.agents/final-e2e/full-test.log)、[汇总](../.agents/final-e2e/full-test-summary.json) |
| 冻结 binary 关键 CLI 复验 | help、mind/output、lifecycle、additional、HTML、publish update、完整 e2e：267 passed、0 failed、2 ignored | [7 个测试目标及运行参数](../.agents/final-e2e/pinned-tests.json) |
| 当前冻结 binary 持久化链路 | 128 次实际 CLI 调用、27 组断言、6 个 HTML 产物 | [可重现脚本](../.agents/final-e2e/journey.py)、[命令及响应](../.agents/final-e2e/cli-events.json) |
| 实际恢复链路 | 81 次 CLI 调用；5 个准备阶段 × resume/rollback，另含后续编辑拒绝覆盖 | [脚本](../.agents/final-e2e/recovery.py)、[结果](../.agents/final-e2e/recovery.log) |
| 实际本地 RAG | 18 次 CLI 调用，offline Lance sync、正例命中、additional 哨兵无命中、Lance 转换拒绝且零写入 | [脚本](../.agents/final-e2e/rag.py)、[完整响应](../.agents/final-e2e/rag-events.json) |
| 实际浏览器 | 7/7 场景；Chrome 154，经 agent-browser 独立 profile | [DOM/资源观察](../.agents/final-e2e/browser-observations.json)、[浏览器汇总](../.agents/final-e2e/browser-summary.json) |
| 文档/帮助静态核验 | 9 个真实 help；生成手册逐字匹配；当前指南及仓内三项 Skill 已核 | [核验记录](../.agents/final-e2e/help-check.json) |
| 继承的真实 Skill evals | 78 执行/78 评分；candidate 39/39 全 8 分；baseline 28 pass/11 fail；本次未重跑模型 | [原验收报告](skills-evals-verification.md)、[本次只读完整性复核](../.agents/final-e2e/integrity.json) |

完整回归包含本轮四包已实现的测试，不以这些包的历史通过记录替代此次运行。
F16 原 17 项 publish-update 与 publisher E2E 已重新通过；历史失败/修复证据原样保留。
两个 ignored 是既有 term lint rule/severity 语义延后项，本次仍未执行，未改成 pass。

## 覆盖范围

- 同仓根 category 与子项目 bundle；有效零 output mind 的创建/改名/删除，零 output build 拒绝；多 output 歧义拒绝；文件与分块、多份产物隔离。
- 实际创建 → mind 改名/跨根与子项目移动 → 引用读回 → Markdown/HTML build → fixture 内 local publish → publish update/读回。Markdown 默认；private callout/整块剥离，源稿保留；未关闭的合成 Feedback 不触发额外发布门槛。
- 共享资料仍归属一个 mind；跨 mind 链接随 rename/move 更新；入站引用阻止删除。完整及冻结 binary 的 lifecycle 测试另覆盖 output rename/move/remove、Source/Asset YAML 权威登记、发布记录与受管引用形式。
- additional 无预注册类别的发现、展示、具体路径改名/删除；两布局的真实 build/RAG 不纳入 additional 哨兵。没有自动执行扩展资源；目录形态、碰撞与排除规则由本次重跑的 additional 测试覆盖。
- 两项目 category↔bundle 往返；dry-run 比较路径、字节、权限和时间戳，零写入；重复转换返回 `layout_unchanged` 且零写入；目标冲突和新增文件导致 stale plan 的拒绝路径均实跑。
- 五个准备阶段故障注入、恢复重试、后续编辑保护均经真实 CLI。每个持久写入阶段的中断、rollback 再中断、权限/字节保护由本次完整回归内已有 transaction/lifecycle/additional 单元矩阵覆盖，不冒称每个底层阶段都有独立 CLI 注入入口。
- 本地 Lance 实际启用后，转换返回 `source_backend_unsupported`；没有暗中降级 YAML。搜索走未配置 embedding 的关键词降级，`chunks_embedded_count=0`，不宣称语义向量召回已验证。

## 浏览器与用户入口

实际浏览器读取 LAN URL 下 category/bundle × file/blocks 四个成功页面，
确认 Mermaid SVG、本地 PNG 的 naturalWidth=180、表格、代码、自定义模板及无 private 哨兵。
另读取两布局的 Mermaid 语法失败页，以及 HTTP CSP 拒载 CDN 的模拟失败页：
均显示可读错误、保留源码，图片仍可用。模拟拒载不代表 CDN 真实宕机。
脚本文本未执行。已实际查看正常、自定义模板、语法失败、CDN 拒载截图。

Mermaid 版本为允许的 CDN `11.12.0`。四个成功页 DOM 内有 SVG，不以静态 HTML 搜索代替渲染。
上一包 9 个浏览器场景仅作为历史证据；本次是独立的 7 个场景。

服务 PID **32978**，绑定 `0.0.0.0:62578`，仅暴露
`.agents/final-e2e/review/` 中筛选后的合成产物、图片和审阅说明；禁用目录列表。
配置、源稿、journals、原始评测 transcript 和浏览器 profile 不在服务根目录。
浏览器已关闭并清理自己的合成 profile；HTTP 服务保留给用户。
Captain/用户审阅后负责关闭：核对 [server.json](../.agents/final-e2e/server.json) 后执行 `kill 32978`。
旧设计预览 `:54085` 未关闭，也未作为 mf 原生能力证据。
本机经 LAN 地址读回已通过；没有第二台物理设备的网络实测。

## 候选与输入绑定

HEAD：`23b047c65151de670098a197c89612b88d594cad`。
本次继承的 Skills handoff **payload** SHA-256：
`7ea865eff493d13ef95555841cd3f83bdac06fd034ddd41dfef31bbb84ccb6d7`；
其 JSON 文件本体 SHA 为 `0e39d77a11c7b99f3d16a7feb31ec06864ebf1f02d9eedaed68c5fa849f38455`。
两者口径不同，均已核验。

冻结评测/本次 E2E binary SHA-256：
`e6b28cf014ecb358689ada4c1bfeb43332308319e6740f45eb3f4ac655e809f6`。
`cargo test` 对相同源码重新编译后 binary SHA 为
`80ab4f9a748de1417957c1cff314c0d0c87d9e0df744ff29e1070b92a3e7fc66`。
不宣称二者字节一致或推测差异原因；前包全部受测源码 hash 仍一致。
完整回归绑定新编译结果；267 项关键 CLI 测试通过 assert_cmd 的实际
`CARGO_BIN_EXE_mf` 环境变量显式绑定冻结 binary，持久化链路也直接调用该 binary。

[最终 handoff 与逐文件摘要](../.agents/final-e2e/handoff.json) 绑定当前业务文件、
设计输入、技能 snapshot、binary、报告和本次不可变过程证据。
[输入初始绑定](../.agents/final-e2e/initial-binding.json) 与
[源码/binary 关系](../.agents/final-e2e/source-binary-binding.json) 保留前后关系。
67 个原 handoff 文件、181 个冻结条目、1020 个原始证据及历次 GC 绑定均通过本次复核。
三项仓内 Skill 全树与实际受测 snapshot 一致，78 个协议结果与 candidate 评分已核。
历史辅助/失败记录未改写，不把它们计作新测试。

## 修改、失败记录与边界

本包只修正文档时态错配：`README.md` 和 `docs/mind-output-layout.md`
原写 Skill 工作待后续实施，现链接当前指南及已完成评测。新增本报告。
其余 515 个已有业务文件保持起始字节，包括全部 Rust、测试和 Skills。
没有实现/Skill 改动，因此不重跑无变化的 78 轮，也没有把旧评测冒充新候选评测。
生成手册无需修改；本次未额外重复 fmt/clippy，继承记录与本次真实测试分列。
全树 `git diff --check` 仍报告前包已记录的 build-help 快照对齐尾空格（exit 2）；
该快照字节未变。本包三份文档的空白检查通过，不将全树检查记作通过。

主链路 harness 前三次尝试分别发现：根项目建立后 CLI 拒绝创建嵌套项目、
重复转换返回 `layout_unchanged`、覆盖已有本地发布需要 `--force`。
均为 harness 预期/初始化修正，无产品修复；原记录保留在
`.agents/final-e2e/attempt-{1,2,3}/`，未抹除或计为通过。修正后的完整主链路通过。

实际使用：仓内 `mf-cli` 操作与命令核验、`mf-plan`/`mf-write` 流程/证据核验；
`skill-creator`/`skill-eval` 用于评测口径与继承证据审核，未启动新模型/评分轮次；
`agent-browser` 用于本次真实浏览器读取、DOM 断言和截图。全局旧版 mf Skills
仅作入口发现，当前流程以仓内冻结 Skill 为准，未全局安装或修改。

仅合成 fixture、本地发布目的地和独立审阅 HTTP 服务；未迁移真实资料，未写 minds/logbooks，
未使用真实平台/凭据，未 reset/stash/clean/commit/push/PR/合并，未启动实施子 Agent。
不覆盖远端/跨设备迁移、Lance 权威迁移成功路径、真实 embedding、离线 Mermaid、公式/插件。
原始过程全部位于 ignored `.agents/final-e2e/`，不作为业务提交文件。
