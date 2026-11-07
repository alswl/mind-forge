# mind-forge 专属反馈核验清单

> 公开副本说明：历史记录中的用户目录、内网审阅地址及私有修订标识已脱敏；示例 loopback URL 不表示当前有服务。本文记载原运行结果，本次发布未据此宣称重新验收。

核验日期：2026-10-25。任务 `feedback-triage`，dispatch `mf-triage-20261025`。本文件是 Stage 01 首项的证据交付，不是 Stage 02 设计或修复批准。原有 `feedback-triage-design.md` 原样保留；其中 19 组仅作为线索，本表结论重新核对来源、当前契约及实际运行。

结论：确认 `publish update` 的路径／目录文章定位 bug；同时复现了转换后发布记录保留旧路径、目录修复不随 Git 传播、source 缺失项计数歧义、private callout 诊断歧义。后四项的现象成立，但处理契约不同，不能都当作无需设计的小修。当前版本未复现“二次本地发布静默成功”；它明确失败退出 1。两种布局仍是有效产品需求。

## 范围、版本与证据入口

- 业务隔离树：`/home/example/dev/github.com/alswl/mind-forge/.worktrees/feedback-plan-20261025`，detached HEAD `23b047c65151de670098a197c89612b88d594cad`，未 fetch。
- M：`/home/example/knowledge/research/.worktrees/mf-triage-20261025`，HEAD `[redacted-private-revision]`。
- L：`/home/example/knowledge/journal/.worktrees/mf-triage-20261025`，HEAD `[redacted-private-revision]`。
- 五份专属来源的逐文件最后提交、SHA-256、行数、共享树状态及内容比对见 [inputs.json](feedback-triage-evidence/inputs.json)。本次读取时五份文件与各共享树完全相同，限定路径无未提交修改；没有遗漏这些路径的工作树版本。
- M1 = M `docs/mf-feedback.md`；M2 = M `docs/mf-feature.md`；M3 = M `docs/mf-bug.md`；M5 = M `docs/mf-bug-archived.md`；L1 = L `docs/mf-feedback.md`。以下行号针对上述固定版本，文件原文未改。
- 时间窗：2026-09-13 至 10-25，加当前未结项。归档中 #54、#55、#57、#59 纳入核验；#34–#42 在 08-30 已归档，读取修复总览供去重，不重新作为未结任务。
- 按 Captain 缩小范围的指示，**混合技能反馈中的非 mind-forge 内容未采集**。不再读取混合 skills/lbkit feedback，不引用或复制其业务内容，不读其关联文章。F18/F19 不具备本包可用原始证据，标记排除／未判定，而非“已完成”。M3 中非 mf 的 ASR 条目仅确认归属后排除。

运行证据：

- [reproduce.py](feedback-triage-evidence/reproduce.py)：从空白临时目录创建中性 fixture，68 次命令调用；[runs.json](feedback-triage-evidence/runs.json) 保存逐条 argv、cwd、退出码、stdout/stderr 和关键内容观察。
- [supplement.py](feedback-triage-evidence/supplement.py)、[supplement.json](feedback-triage-evidence/supplement.json)：10 次补充命令，核 held-back、dry-run 零写入、断链及未登记文件。
- 同目录 `publish-update.json`、`publish-run.json`、`after-convert.json`、`after-rename.json`、`after-move.json`、`source-before-missing.json`、`source-after-missing.json`、`final-fixture.json` 为合成文件快照。所有证据和重建脚本保留本仓；临时 fixture 路径在 environment 记录中。
- `runs-initial.json`、`runs-second.json` 保留前两轮原始输出，不作为最终成功凭据：首轮 remove 被引用保护拒绝、move 的相对目标未解析；第二轮未限定项目的 lint 扫到了另一个项目的 orphan。最终明确 `--force`、绝对目标、`--project .` 后重测；没有掩盖失败。
- 执行环境 macOS arm64；安装二进制 `/home/example/.cargo/bin/mf`，`mf version --json` 返回 `0.4.2-dev+23b047c`、构建日 2026-10-06、rustc 1.96.0；二进制 SHA-256 在 environment 记录中。未重编译，结论针对该已固定安装候选及一致的源码 commit 标识，不声称独立重建可复现性。
- 未执行模型 evals、真实仓 source sync、远端发布、迁移真实内容、commit/push/PR。合成发布仅写 fixture 内 local 目标。

## 逐项判定

### F01：原子 rename/move

来源 L1 Issue 1（L8–21；反馈环境 `0.4.1-dev+d5353bb`，2026-10-01）。原始期望是“一命令完成文件、三段索引、绑定和可选入站引用重写”，并非仅要一个命令名称。

当前实际：`rename`、`move` 都已存在。`rename` 将 `docs/old-name.md`、`prompts/old-name.md`、`thinking/old-name.md` 改为 new-name，Prompt article 绑定和投影同步；`docs/referrer.md` 中 `[Target](old-name.md)` 原样保留，成为断链。跨项目 move 搬移正文及两种绑定文件，目标索引可见。证据 `rename`、`move` 与两份 after 快照。

判定：**部分已满足＋需求**。“全手工／缺命令”前提在当前候选不成立，重复开发命令是假需求候选；引用维护和原子性诉求仍有效。源码 `src/service/article/rename.rs:118` 起是顺序操作，投影刷新是 best-effort；move 有部分回滚，不能据此声称完整事务。没有注入中途写失败，崩溃恢复仍待核。当前替代是使用已有命令后人工核引用，而非宣称它已满足原子迁移。

### F02：索引漂移体检

来源 L1 Issue 2（L22–35）。期望四类检查、只读输出及可区分问题的退出码。判定：**部分覆盖的需求**，不能简单新造 doctor，也不能用已有 lint 存在否定缺口。

| 原始检查 | 当前证据与差异 |
| --- | --- |
| 登记路径存在 | `drift-lint` 报 stale article/source，默认 warning 仍 exit 0；可用 `--max-warnings 0` 提升门禁。`project/lifecycle.rs:170` 检查 articles/assets/sources。不是“每个 entry”全覆盖。 |
| Prompt/Thinking 双向绑定 | 已有 orphan/duplicate/missing_thinking；依赖索引投影。新写 orphan 未 index 前不报，`article index` 后 `dot-lint` 报 error、exit 1。源码检查投影，未完整逐文件核双向存在。 |
| 正文 article 相对链接 | 补充 fixture 的 `[Absent](absent.md)` 经 index 后，project lint 和 article lint 都 exit 0、零 issues。需求未覆盖。 |
| 磁盘孤儿／未登记文件 | `not-indexed.md` 不被两种 lint 报出；article index dry-run 能列 added，但不是统一只读漂移诊断。 |

证据：`drift-*`、`dot-lint`、补充 `link-and-unindexed-lint`、`link-article-lint`、`unindexed-preview`。当前组合替代是 lint＋article index dry-run＋人工链接核对；仍不能满足统一完整检查。

### F03：新术语与 correction 组合路径

来源 L1 Issue 3（L36–49；2026-09-30）。期望提示完整命令序列或可显式创建术语。

`correction-unknown` exit 1，已有 hint `use mf term list or mf term new`；不是完全无提示。`term-new` 后 `correction-add` 均 exit 0。**易用性需求，部分满足**：拒绝不存在术语符合前置约束；两步路径有效，尚无完整带参数示例或 create-term 组合入口。无需把自动创建当作唯一方案，也不能用多一步成本否定需求。是否新增命令留设计。

### F04：短篇 single doc

来源 M1 L3–11，2026-10-19。原文已明确“不是新增 CLI 能力需求”。`file-create` 实际生成 `docs/single.md`，`dir-create` 生成 `docs/chunked/01-opening.md`，shape 分别 file/directory。

**CLI 已满足；流程选择需求有效**。重复开发单文件能力是假需求候选，短文形态选择不是假需求。mf-write 已要求尊重现有形态；mf-plan 的创建步骤尚未显式建立“短文优先 single doc”的选择准则。行为是否改善须后续真实 Skill evals，本次不凭文字断言通过。

### F05：按项目选择聚合布局

来源 M1 L13–40，2026-10-19。**有效产品需求，用户已裁定采用双布局**。

`src/model/config.rs:181` 的 LayoutConfig 配置 articles/sources/assets/templates/build_output 等类别目录，不是逐 mind 聚合策略。`skills/mf-plan/SKILL.md:15` 从 canonical path 最后组件取 key，并固定拼 `prompts/<key>.md`；聚合布局若各 mind 使用相同正文 basename，会冲突或错定位。此为设计约束，不是已支持布局中的已复现 bug。

必须保持按 category 与按 mind 两种模式项目级可选、与单文件／分块正交。对象职责和路径身份不能随布局被悄然改变；共享资料归属与迁移契约交 Stage 02。本次不预定目录名、配置字段或默认值。

### F06：旧稿＋新材料按变更规格修订

来源 M2 L5–40，当前进度注明 09-13 只追加说明、未验收。期望含差异分析、反馈分流、章节状态、减少决策负担、转写证据及真实产物检查六项。

**有效 Skill 需求，行为待核**。当前 plan/write 已共读 prompt、sources、thinking、正文和反馈；write 明示 partial/ambiguous/blocked 保持 open（L72），已知基线才归纳作者 delta（L118），发布前须 fresh build（L129 起）。未见足以证明“旧稿＋演讲稿”六项全部兑现的运行证据，不能把已有说明当交付。

仅静态确认已有 plan 3 例、write 4 例 eval 配置；未运行模型。缺：安全合成旧稿／新增材料、同候选真实调用记录、逐项语义评分与文件差异。确定性定位／状态操作适合 CLI，结构与事实判断留 Skill；不新增平行台账。

### F07：图片元信息协议

来源 M2 L42–57。**有效需求，按用户裁定延后**。现有 build pipeline 可调用外部渲染器，并有依赖／失败规则；mf-cli build 章节描述的是 sibling-asset 转换，不构成文章元信息传递协议。不能以已有 renderer 宣称需求满足。底层手工叠加负担有来源描述，本次未验证复用频率或参数协议；当前替代为外部工具处理稳定 asset。协议、失效和失败策略均未设计。

### F08：correction used/accepted/rejected

来源 M2 L59–78。**规则质量反馈需求有效；自动状态推断方案待核且延后**。当前 term 模型／命令没有该三种持久聚合反馈协议。lint 命中只能证明规则命中，fix 后 Git 变化不能天然证明接受或拒绝；原文观察窗口与存储位置本来就待确认。没有原始用户裁定，不自动猜状态。现有替代是显式审阅和记录误伤；不因实现成本判假需求。

### F09：中文 block 文件名

来源 M2 L80–97，L82 明确 09-14 superseded。**已撤回，不纳入**。底层中文章节可读性诉求仍在，但原作者选择“英文文件名＋中文 H2”。已有 block rename 明确不改 H2；当前替代是英文命名与中文正文标题。不是待实现中文命名功能，也不重开已撤回方案。

### F10：held-back 降噪

来源 M2 L99–109。**有效诊断需求**。`docs/term-lint.md:54` 起明确 short-CJK 是 advisory，不是可自动修正项；安全抑制自动替换与抑制重复展示是两回事。

补充中性 fixture `机器 → Device`：普通 lint 与 `--exclude-original 机器` 都 exit 1，JSON 均保留一个 held-back finding。排除参数参与 fix selection，不能当成满足原文“只读输出静默”的证据。现有 JSON `held_back`/计数支持消费者过滤，但没有项目级 ignore 的等价体验。默认隐藏或配置过滤涉及输出契约，留设计；不删除安全护栏。

### F11：相邻 private callout 的警告

来源 M2 L111–120。原文已说明正文无损，期望自然分隔或准确诊断。

`private-build` exit 0，出现 `merged across a bare blank line — content after the blank line was stripped along with it`；产物保留 PUBLIC_BEFORE/PUBLIC_AFTER，排除 SECRET_A/SECRET_B。**诊断歧义已复现，正文丢失未复现**。现有剥离结果符合私有内容契约；若仅准确说明合并内容／位置，可列诊断小修候选；改变空行解析语义需设计。证据 `private-artifact`，未用真实文章验证。

### F12：通用终稿模式

来源 M2 L122–139，09-15 更新称宿主 style 已承接。**宿主承接的可选增强，按用户裁定延后**。不再读取宿主技能或业务文章，本次不声称其行为已通过；仅保留专属反馈记录的历史处置。制作指示泄漏是实际风险，但词表命中不是语义验收。当前替代为宿主终稿审阅和真实构建检查；重复做同一宿主流程是假需求候选，跨仓通用能力仍需价值与契约证据。

### F13：lint --fix 空目录不可传播

来源 M2 L141–158，2026-10-04。**现象已复现，属于 Git 协作需求／修复持久性缺口**。

独立 empty 项目删除 sources/assets 后，`project lint --project .` exit 1、2 errors；加 `--fix` exit 0、2 fixed；再 lint exit 0。目录内没有文件，`git ls-files --others --exclude-standard` 只有 mind.yaml/mind-index.yaml，没有 sources/assets 占位，Git 无法带走空目录。源码 `project/lifecycle.rs:139` 仅 create_dir_all。

不能说本机 mkdir 修复失败；原需求是其他检出也能保持有效。加占位文件、重新定义可选目录或标明本机修复是不同契约选择，留设计，不自动写 .gitkeep。证据 `empty-*`、`fixed-directory-files`。

### F14：source index 的 -N

来源 M2 L160–178，2026-10-04，原事故未记录精确 mf commit/backend。**计数诊断歧义已复现；“删数据”推论反证，原事故归因待核**。

文件存在时 `source-index-stable` 输出 `+0 =1 -0`；删除合成磁盘文件后 `source-index-missing` 输出 `+0 =0 -1`，JSON removed 包含 alpha，但 source list 仍有该登记，前后索引字节相同。不是 skipped 误映射的证据。`src/service/source/index.rs:185` 起将不在 disk_paths 的登记列入 removed，并明确保留原登记；CLI 共用 -N 展示了该数组。

现有契约就是“报告缺失、不自动删除”，文案容易误导。可优先考虑文本澄清，不能未经兼容性设计直接重命名 JSON removed。缺原事故源文件可达性、backend 与二进制证据，不能断言它就是这条分支；Lance 未实跑。

### F15：二次本地发布与旧产物

来源 M3 L6–11，2026-10-08，精确事故版本未给出。**“当前 CLI 静默成功”未复现；旧产物风险成立但调用链归因待核**。

`build-v1` / `publish-v1` 成功后修改正文并 `build-v2`：不带 force 再 publish 的文本／JSON 两次均 exit 1，stderr 为拒绝覆盖／file_exists，目标仍 VERSION_ONE、build 已 VERSION_TWO。带 force 后 exit 0，目标更新为 VERSION_TWO。源码 `publish.rs:418` 与 `error.rs:211` 一致。

再只改源文为 VERSION_THREE_NOT_BUILT、不 build，带 force 的 publish exit 0，仍拷贝 VERSION_TWO。这证明 publish 消费已有 build artifact，不负责自动重建；`locate_build_artifact` 仅检查存在／size，非 freshness。mf-write 的发布门禁要求 fresh build，当前替代为真实 build→publish --force→核读产物，并正确处理 exit code。

不能用该实验证明旧事故调用方忽略了失败，也不能改 overwrite 默认。缺：事故二进制 hash、完整 argv/退出码、构建目标路径、下游调用是否在 mf 失败后继续、源与产物版本。新鲜度保护如需增强，须另定契约。

### F16：publish update 目录定位与记录修正

来源 M3 L13–19，2026-10-12。拆为三项，防止把路径 bug 与记录管理功能合并。

1. **已复现 bug：update 定位违反 help。** 单文件 slug `single` 创建记录 exit 0；`docs/single.md`、`docs/chunked`、尾斜杠、block 路径在 dry-run 和实际调用均 exit 2 usage；目录 slug `chunked` exit 1 not_found。help 明示 project path。`publish.rs:85` 拒绝 `/`，L108 起只匹配 `<articles>/<slug>.md`。至少 canonical 文件／目录路径及目录 slug 应经共享 resolver 定位；block 是否支持以共享定位契约为准，不自行扩展选择器。
2. **转换后的关联缺口已复现。** 已登记 single 经 `article convert --to-directory` 后，article_path 是 `docs/single`，publish_record 仍为 `docs/single.md`（after-convert）。保留 URL／published_at 的重绑／去重能力未由现有 update 提供。事实确认；新管理命令、自动迁移历史记录和冲突处理留设计，不混入定位小修。
3. **“目录文章不能发布”的泛化不成立。** build chunked 后 `publish run docs/chunked --target local` exit 0，生成本地产物。不过 run 不写 publish_records；因此它不能替代“只登记目录文章发布记录”的需求，后者当前确实受 update bug 阻断。

最小复现和预期见下节。失败 dry-run 前后 fixture hash 完全相同。未修复任何源码。

### F17–F19：范围排除与证据缺口

- F17：M3 L21–26 明确故障来自宿主 ASR Python 脚本，不是 mf CLI。**宿主问题，排除**；未运行脚本，不裁定是否已修复。移入 mf 修复队列的归属前提不成立，不代表底层故障不存在。
- F18：草稿线索为“已有只读授权但重复审批”。当前不读取混合原文，**排除／未判定**；如需后续归因，缺经内部执行者脱敏的授权时点、宿主门禁与实际调用记录。不能要求 CLI 绕过权限。
- F19：草稿线索为执行授权、Source 同步边界、绑定缺失等。**排除／未判定**；缺逐项纯工具原文与当前宿主版本，不将泛化线索视作已验收或 mind-forge bug。混合文件中的唯一必要工具证据由 Captain 另行提取，本包不追读。

## 时间窗内归档反馈

| 原始定位 | 期望、历史处置与当前核验 | 判定 |
| --- | --- | --- |
| M5 #59 L482–505 | remove 默认删文件、keep-file 保留。历史称 09-29 `72b711d` 修复；最终 `source-remove-keep` exit 0且文件存在，`source-remove-default` exit 0且文件不存在。合成引用保护需明确 --force。 | 当前已满足，不重复开发。 |
| M5 #54 L507–541 | project-relative source rename。历史称 09-30 `d5353bb` 修复；`source-rename` 从 beta 移至 `sources/renamed/gamma.md` exit 0，path 正确。 | 正常路径已满足；错误路径诊断本轮未重测，保留历史证据级别。 |
| M5 #55 L543–577 | `--project .` 一致解析。历史称 09-30 `d5353bb` 修复；dot-index 扫描实际文章、dot-build 得到 chunked 计划、dot-lint 报真实 orphan 而非 project not found；empty 项目显式 . lint 正常。 | 本轮未复现历史解析故障。 |
| M5 #57 L579–616 | block --after 接受 filename/stem/slug。三种 `01-opening.md` / `01-opening` / `opening` dry-run 均 exit 0且 order 相同；absent exit 1并有 article show hint；help 正确解释 --start。 | 已满足，不重复开发。 |

#34–#42 仅引用归档总览（M5 L6–22）的已修复状态去重；不把 09-13 前已关闭条目中的旧“待确认”措辞重新打开。未跑 Lance 重建／激活／恢复用例，不声称这些历史修复在本轮全量复验。

## 关键最小复现与契约证据

在 [reproduce.py](feedback-triage-evidence/reproduce.py) 建立的空白仓内，cwd 为 `projects/probe`，local target 只指向临时目录。以下命令的完整结果均在 runs.json，可按 id 查找。

```sh
mf article new single --file --json
mf article new chunked --json
mf publish update single --target local --set status=published --json
# 0；对照：支持单文件 slug 登记
mf publish update docs/single.md --target local --set status=published --json
# 2 usage；期望按 help 解析 canonical 文件路径
mf publish update chunked --target local --set status=published --json
# 1 not_found；期望解析已存在的目录文章
mf publish update docs/chunked --target local --set status=published --dry-run --json
# 2 usage；期望给出同一文章的登记预览，不写入
mf build chunked
mf publish run docs/chunked --target local --json
# 0；目录文章产物可正常发布，run 本身不登记发布记录
```

版本／设计核对：

- `docs/glossary.md` 的 Path Identity、Selector、Content Shape：路径是身份，标题是展示；目录／单文件是独立形态。支持 F16 统一定位，而不是额外拼 `.md`。
- `docs/schema-naming.md`：Article/Source/Asset/Prompt/Thinking 职责独立，目录不是对象本身。支持 F05 双布局设计，不能靠复制资料维持兼容。
- `skills/README.md`、plan/write：两技能共用状态，研究判断与表达交付各有职责；F06 需要真实行为 eval，不把 CLI 检查冒充语义完成。
- `docs/term-lint.md`：held-back 是安全披露，F10 需要降噪而非重新自动替换。
- `docs/build-workflow.md`、`docs/retro-074-fix-source-term-bugs.md`：先核二进制，按范围验证。本轮固定安装候选版本/hash，未改源码，因此未跑全仓测试。
- publish 源文件最近修改 `dd60e5c`（2026-09-30），source index `fd7b661`（08-30）；仅作为历史定位，不据提交日期推测事故机器安装版本。

## 交接与首项验收

在 Captain 收窄后的专属反馈范围内，本项交付已满足：五份来源有固定版本与定位，F01–F16 与时间窗内四项归档均给出期望／实际／分类／证据或缺口，F17–F19 明确排除而非关闭。尚未证实的历史归因、原子故障恢复、Lance 行为和 Skill 效果已分别留缺口，未伪装成通过。最终日志勾选由 Captain 处理。

边界明确的小修候选仅优先推荐 **F16 update 复用既有 article resolver**：覆盖单文件 slug、canonical 单文件路径、目录 slug／路径及 dry-run，保留既有目标校验与记录字段语义。F11/F14 可列文本诊断候选，但须保留解析／JSON 行为。转换记录重绑、原子迁移、目录持久性、发布新鲜度及布局不纳入该定位小修。

mf-cli 实际作用：按技能帮助选择创建、index、build、local publish、term correction、rename/move 和 dry-run；用 JSON 与退出码将“报告成功”“已生成产物”“已登记发布记录”分开。它帮助反证了二次发布静默成功与完全缺少 rename 的泛化，也暴露 update 与 help 的不一致。未安装或升级 CLI。

交付只新增本清单与 `docs/feedback-triage-evidence/`，原草稿保留；未改代码／技能／来源／协调日志，未 commit/push、开 PR 或发布到外部。到此回传，不开始修复或 Stage 02。
