# build-html 验证报告

> 公开副本说明：历史记录中的用户目录、内网审阅地址及私有修订标识已脱敏；示例 loopback URL 不表示当前有服务。本文记载原运行结果，本次发布未据此宣称重新验收。

任务 `mf-build-html-20261025` / `build-html`。实现和验证完成，待 Captain 验收，未提交。
父候选 `b3b745f8d3a3f1c622fbe054c282f5363506098fc8b65bc03a92f40f0a6dedde` 已在开始时逐文件核验。
本包摘要与完整累计文件哈希见 [final-candidate.json](../.agents/build-html/final-candidate.json)。

## 交付行为

`mf build --format markdown|html` 默认 Markdown，内部参数 ID 与全局 `--output json|text` 分离。
HTML 在现有装配、private 剥离、资源路径重写之后生成；不同 output/格式使用独立默认产物路径。
`build.html_template` 支持安全过滤的静态 body 模板，必须有一个独立文本节点 `{{content}}`，
可用 `{{title}}`。原文 HTML 显示为文本，模板/Markdown 及占位符替换后内容均过滤；CSP 限制脚本。
固定官方 Mermaid 11.12.0、strict 安全级别、拒绝图表配置覆盖；图表源码始终保留。
CDN 加载/超时与语法失败分别提示，并展开源码；不自动 build 其他内容、发布或起服务。
使用契约见 [HTML output](build-html.md)。

## 同一最终源码验证

| 命令/验证 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo build` | exit 0 |
| `cargo test` | 2536 pass / 0 fail / 2 既有 ignored |
| `python3 .agents/build-html/prepare-browser.py` | 8 条真实 HTML build 命令通过 |
| `python3 .agents/build-html/browser-verify.py` | 9 个真实浏览器场景通过 |

最终源码仅执行一轮完整适用回归，测试前后源码哈希一致；此前仅针对性测试。
相对第三包新增 3 个 CLI 集成测试和 1 个模板 URL 安全测试；前三包与 F16 回归全部保留并通过。
两个 ignored 仍是既有 term lint rule/severity 边界用例，未新增忽略。
完整命令/exit/log 与测试源码绑定见 [results.json](../.agents/build-html/results.json)、
[tested-files.json](../.agents/build-html/tested-files.json)。

## 真实浏览器观察

技能：实际使用 [agent-browser SKILL.md](/home/example/.codex/skills/agent-browser/SKILL.md)，
读取 CLI 提供的 core 工作流。agent-browser 0.39.1；浏览器实际可执行版本
**Google Chrome for Testing 154.0.8037.57**（UA HeadlessChrome/154.0.0.0）。
先检查进程和 session 所有权：起始 agent-browser session 为空，人类 Chrome 未复用或关闭。
本包 session `mf-build-html-20261025` 使用 `.agents/build-html/browser-profile` 独立 profile。

正常服务 `http://127.0.0.1:64205/`，拒载服务 `http://127.0.0.1:64728/`；均仅服务本包合成 site。
所有实际 URL、DOM、SVG、图片 naturalWidth、资源请求及状态在
[browser-observations.json](../.agents/build-html/browser-observations.json)。

| 场景 | 浏览器观察 |
| --- | --- |
| category 根项目 / bundle 嵌套项目 × 单文件 / 分块 | 4 份真实 SVG，表格/代码正常，图片 naturalWidth=180 |
| bundle 项目覆盖模板 | CUSTOM PROJECT TEMPLATE 在浏览器可见 |
| 两布局语法错误 | `parse-error`，可读提示，源码展开，无错误 SVG 冒充成功 |
| 两布局受禁 Mermaid 配置 | `parse-error`，未降低 strict 安全配置 |
| CDN 拒载 | `load-error`，提示和源码可见，表格/代码/本地图片继续可用 |
| 原文脚本与 private | 脚本文本可见但未执行；DOM/HTML 无 private 哨兵 |

已实际查看正常页面和两种失败页面截图，未用静态断言替代浏览器。
截图：[模板/正常 SVG](../.agents/build-html/bundle-single.png)、
[语法失败](../.agents/build-html/category-syntax-error.png)、
[CDN 加载失败](../.agents/build-html/cdn-failure.png)。
CDN 拒载由独立测试 HTTP 响应 CSP 阻止外部脚本，产品 HTML 字节不变；它验证真实浏览器加载失败路径，
不宣称 CDN 实际宕机。初次 browser route 未阻断缓存模块的失败记录原样保留。
本包 browser session 和两个临时服务已关闭，清理记录见 cleanup.json；未删除锁或复用登录态。

## 本包全部文件

- `Cargo.lock`
- `Cargo.toml`
- `README.md`
- `docs/build-html-verification.md`
- `docs/build-html.md`
- `docs/manual.md`
- `docs/mind-output-layout.md`
- `src/cli/build.rs`
- `src/model/config.rs`
- `src/service/build.rs`
- `src/service/build/html.css`
- `src/service/build/html.js`
- `src/service/build/html.rs`
- `tests/cli_build_html.rs`
- `tests/snapshots/cli_help__build_help.snap`

共 15 个本包文件；累计交付文件及全部 SHA-256 在候选 manifest。
[protection.json](../.agents/build-html/protection.json) 绑定 451 个未改动基线文件、
允许修改的 before/after 哈希及前三包继承证据。历史报告、F16、源稿和原 evidence 均保留。
未写 minds/logbooks；没有迁移真实资料、reset/stash/clean、commit/push/PR 或外部发布。

## 限制及过程记录

范围内无未完成项。HTML 依赖 CDN；本地资源沿用相对路径，不做离线打包。
静态模板不提供自定义脚本/插件接口；公式、脚注、多主题和 Skills 不在本包。
未启动子 Agent 或新模型。
初轮发现 Clap 内部 `format` ID 冲突，已改为 `artifact_format` 并通过后续全部回归。
`git diff --check` 唯一提示来自自动生成的 build 帮助快照 `--out` 空说明行的对齐空格；
保留与真实 Clap 输出一致的快照，fmt/clippy 均通过。其余文件 whitespace 检查通过。
原始日志、失败尝试、合成站点和浏览器 profile 全留已忽略 `.agents/build-html/`，不作为业务文件。
