# 反馈优化设计：双布局与 Skill／CLI 职责

状态：**待原会话用户审阅，尚未批准实施**。任务 `layout-skill-design`，dispatch `mf-design-gc-20261025`（续接原 `mf-design-20261025`），2026-10-25。本包只形成设计；不实施 Stage 03，不运行模型 eval，不新增 Agent，不迁移真实内容。

设计基线为 `23b047c65151de670098a197c89612b88d594cad` 加已验收小修 `publish-update-ad7887bc223f`。F16 的 update 已复用共享 resolver；不能再把“拒绝所有路径”写成当前行为。旧[初筛草稿](feedback-triage-design.md)、[核验清单](feedback-triage-verification.md)、[小修验收](publish-update-fix-verification.md)及原始证据保持不变，分别代表其产生时的候选。

本设计推荐：保留旧 category 布局，在项目中显式启用 mind 布局；由 CLI 统一返回路径和绑定，Skill 不再拼 basename。迁移采用可预演、可恢复的本地操作，不承诺文件系统无法提供的跨文件瞬时原子性。目录持久性沿用现有必需目录规则，增加可跟踪占位。文末 D1–D3 是尚需裁定的产品取舍，不重问已经确认的方向。

## 1. 已确认约束与需求处置

已确认：两种**物理组织**都支持、按项目可选；与 file/blocks 正交；未配置项目不迁移。Skill 必须真实行为 eval，确定性操作下沉 CLI，语义判断留 Skill。延后自动 accepted/rejected 推断、图片元数据协议、通用终稿检查；中文 block 文件名已撤回。ASR／重复审批不自动归因 mf。F18/F19 缺脱敏原文，继续待核。

下表 F 编号及来源定位沿用已验收核验清单，清单内有原始文件版本、行号及命令证据。本表是设计建议，不替换原始反馈，也不以实现成本否定诉求。

| 反馈 | 当前证据与性质 | 设计处置及可观察结果 |
| --- | --- | --- |
| F01 原子 rename/move | 命令和部分绑定联动已有；入站链接未改，事务覆盖不全 | 采纳关联维护及恢复需求，扩充既有命令；不重造 rename。预演列写集合，获准执行后绑定／记录一致，选定引用范围无新增断链。见 §4。 |
| F02 漂移体检 | lint 覆盖部分登记和绑定，不查完整磁盘双向关系、正文断链 | 采纳；扩充 project/article lint，不加 doctor 平台。只读报告四类缺口，支持现有 severity/max-warnings。 |
| F03 新术语＋correction | term new→correction add 可用，已有简略 hint | 采用现有两步路径，补明确 scope 与参数示例；不采纳隐式创建不完整术语。组合命令不纳入此轮。 |
| F04 短篇 single doc | CLI --file 已满足，流程选择未验收 | 替代重复 CLI 开发：plan 按规模和维护需要选择形态，短文默认建议 --file；不改 CLI 默认目录形态。E01 验证。 |
| F05 mind 聚合 | category 路径配置不能表达聚合策略 | 采纳用户已定方向；项目级双布局，统一映射与显式迁移。见 §2–4。 |
| F06 旧稿＋新材料 | 已有说明，未完成真实行为验收 | 采纳；同一 prompt/thinking/反馈线程贯穿差异分析、章节任务、修订与核验。无额外 Skill 或平行状态表。 |
| F07 图片元信息 | 外部 pipeline 不等于元信息协议 | 按裁定延后；继续外部 renderer，失败不冒充新协议已支持。 |
| F08 correction 自动反馈计数 | 命中不等于接受，恢复写法不必然是拒绝 | 按裁定延后自动推断；人工审阅仍可指出误伤。不新增隐式计数副作用。 |
| F09 中文 block 文件名 | 原作者已撤回 | 不采纳已撤回方案；英文文件名＋中文标题，block rename 不隐式改 H2。 |
| F10 held-back 降噪 | 安全披露仍有价值；exclude-original 不等价于隐藏只读结果 | 采纳展示需求，建议显式 `term lint --hide-held-back`，只过滤展示，保留总计和退出判定；默认 JSON 不变。项目级 ignore 首轮不增加，消费者可显式过滤。 |
| F11 相邻 private 警告 | 警告歧义复现，正文丢失未复现 | 采纳精准诊断：显示受影响行范围和识别出的私有 callout，无法判断性质时明示未知；不改空行解析／剥离规则。 |
| F12 通用终稿模式 | 专属反馈记载宿主承接，宿主效果本轮未验证 | 按裁定延后通用语义检查；CLI 仅验证 build 可机械证明的私有标记剥离和资源路径，不声称识别所有内部信息。 |
| F13 空目录 | mkdir 本机有效、Git 不传播 | 推荐保留必需目录语义，在 new／显式 lint --fix 写 .gitkeep；见 D3，不自动改已有项目。 |
| F14 source -N | 缺文件列入 removed，登记仍保留；原事故 backend 不明 | 采纳文本澄清 `missing registrations retained`；JSON removed 兼容保留，可加 missing_count，不改删除策略。原事故归因仍待核。 |
| F15 二次发布旧稿 | 当前 FileExists exit 1；publish 读取已构建产物 | 不采纳默认自动覆盖。Skill 明确 fresh build→publish→读回，故障停止下游；历史事故待完整调用链证据，CLI 自动 freshness 协议延后。 |
| F16 定位／记录 | 定位小修已验收；转换后旧记录路径仍未重绑 | 定位已满足，作为后续回归基线；采纳生命周期内的明确路径映射与记录重绑，冲突不自动合并。 |
| F17 宿主 ASR | 原反馈指向宿主脚本 | 排除本轮 mf 实现；不把排除写成已修复。 |
| F18/F19 混合反馈线索 | 本包无可用纯工具原文及宿主版本 | 待核，不编造已解决结论。仅在 Captain 提供脱敏原始证据后重新判定；不读混合文件或业务文章。 |

归档 #54/#55/#57/#59 当前正常路径已复验，保留回归，不重复开发；#34–#42 的历史修复不因表内旧措辞重新打开。

## 2. 领域关系、目录与配置

### 2.1 沿用领域模型，分清四类“名字”

以 [glossary](glossary.md)、[schema-naming](schema-naming.md) 为现有领域模型；不新建平行 glossary/ADR。本节记录候选增补，批准后才更新现有文档。

| 概念 | 领域含义及边界 |
| --- | --- |
| Project | repo-relative 路径身份，选择布局的边界。一个项目一种创建／发现布局，同仓项目可不同。 |
| Article | 可发布正文，身份仍为 project-relative **article_path**。file/blocks 是形态，不是两种实体。 |
| 聚合目录（候选词义） | 一篇 Article 及其 Prompt、Thinking、局部素材的容器；不是新增业务实体或 UUID。默认一目录一篇正文。 |
| Prompt / Thinking | 前者是作者意图与约束，后者是推理／进度记录；均为 authored Markdown，索引只是投影。 |
| Source / Asset | 前者是证据和溯源，后者是媒体附件；物理靠近某 Article 不改变用途，也不证明只被它引用。 |
| slug | 创建／rename 选择的路径片段；聚合模式中是容器名，不能由后续标题修改偷偷重算。 |
| 标题 | 展示内容；不是路径或绑定身份。不能通过标题相同推断两个 Article 是同一篇。 |
| selector | 输入定位语法。canonical path 必须 round-trip；短写必须唯一，歧义应列候选。 |

边界场景：`minds/alpha/article.md` 与 `minds/beta/article.md` 是两篇 Article，即使标题均为“说明”。`alpha` 可以是唯一短写；`article` 必须因多匹配而失败。同一容器同时有 `article.md` 和 `article/` 是形态冲突，不选择最新 mtime。项目级 constitution 不是某篇文章的 Prompt，不被搬进第一个 mind。

### 2.2 推荐目录实例（D1）

category 未配置时沿用现有物理结构；示例同时有短文 file 和长文 blocks：

```text
projects/category-project/
├── mind.yaml
├── mind-index.yaml
├── docs/
│   ├── brief.md
│   └── review/
│       ├── 01-opening.md
│       └── 02-evidence.md
├── prompts/
│   ├── constitution.md
│   ├── brief.md
│   └── review.md
├── thinking/
│   ├── brief.md
│   └── review.md
├── sources/file/shared-study.md
├── assets/shared.svg
└── outputs/
    ├── brief.md
    └── review.md
```

mind 模式推荐保留容器名作为可读 slug；正文固定 article 角色名，不把容器本身当作分块正文目录：

```text
projects/mind-project/
├── mind.yaml
├── mind-index.yaml
├── prompts/constitution.md
├── minds/
│   ├── brief/
│   │   ├── article.md
│   │   ├── prompt.md
│   │   ├── thinking.md
│   │   └── assets/diagram.svg
│   └── review/
│       ├── article/
│       │   ├── 01-opening.md
│       │   └── 02-evidence.md
│       ├── prompt.md
│       ├── thinking.md
│       └── sources/file/local-study.md
├── sources/file/shared-study.md
├── assets/shared.svg
└── outputs/
    ├── minds/brief/article.md
    └── minds/review/article.md
```

Prompt/Thinking 和局部 sources/assets 按实际使用创建，不强制空壳。容器内正文仍独立为 file 或 directory，build 只装配 article 指定路径，不把 prompt/thinking 一起扫描进产物。

### 2.3 项目配置、默认和继承（均为建议新增）

```yaml
# 新 mind 项目；schema 2 是建议的旧客户端拒绝门槛，见 D1
schema: "2"
layout:
  mode: mind                  # category | mind；未配置永远 category
  minds: minds                # 项目相对容器根
  sources: sources            # 项目共享 Source 根
  assets: assets              # 项目共享 Asset 根
  templates: templates
  build_output: outputs
```

category 的 `layout.articles`、`layout.docs` 兼容别名及历史 `paths.docs` 等字段沿用当前优先级：canonical layout > historical paths/build.output_dir > 默认。默认 articles=docs、sources=sources、assets=assets、templates=templates、build_output=outputs；prompts/thinking 保留当前约定。

推荐 **mode 和 minds 只由项目 mind.yaml 决定，不做运行时仓库默认继承**。项目 new 可显式 `--layout category|mind`，模板可提供初值，创建时写入该项目；不因为后来改全局配置而重新解释旧项目。字段缺省值按项目计算，不能把 `merge()` 当前整块覆盖误称逐字段继承。仓级模板默认只影响未来创建，不改存量。

mind 模式下 `layout.articles` 不表示共享正文根，拒绝同时显式配置它以避免两套发现规则。sources/assets 在两种模式都指项目共享根；局部目录名首轮固定，不增加逐容器定制语言。所有根路径须在项目内、互不包含、不得是输出／状态目录或经 symlink 越界。

旧项目保持 schema 1 与原路径；启用 mind 的项目及其索引用 schema 2，防旧客户端忽略新 mode 后错误写回。新客户端读 1/2；旧客户端应明确拒绝受影响项目，不修改／降级其文件。旧版本拒绝行为必须进兼容测试，尚未验证，不能仅凭写了 schema 就宣称安全。minds.yaml 不整体升版，允许同仓 schema 1 category 与 schema 2 mind 项目共存。

转换回 category 不自动降 schema；仅显式回滚可恢复原 schema 与原字节。schema 2 的兼容成本是旧 CLI 不能操作这些项目，见 D1。手改 mode 不是迁移：非空存量树与配置不一致时 lint 报错、mutator 拒绝并提示迁移预演，不能重新扫描后默默丢掉旧条目。

### 2.4 路径／索引／绑定映射

| 对象 | category（brief 示例） | mind（brief 示例） | 真相来源与改动 |
| --- | --- | --- | --- |
| Article file | docs/brief.md | minds/brief/article.md | 正文文件＋既有声明/索引元数据；article_path 仍完整路径。 |
| Article blocks | docs/brief | minds/brief/article | 只扫描正文目录的 block；不能把容器内其他 Markdown 误当 block。 |
| Article index key | docs/brief | minds/brief/article | 沿用去 terminal .md 的键；同容器两形态不允许同时存在。 |
| Prompt | prompts/brief.md | minds/brief/prompt.md | frontmatter `article:` 始终 project-relative canonical article_path；不改为相对 prompt 的路径。 |
| Thinking | thinking/brief.md | minds/brief/thinking.md | category 继续按现有 key；mind 按容器唯一正文绑定，无新增 frontmatter，无 UUID。 |
| Prompt/Thinking projection key | brief | minds/brief/prompt、minds/brief/thinking | schema 2 用完整角色路径去 .md，避免所有键都叫 prompt/thinking；identity 输出使用完整 path。旧键读兼容、显式 index 时迁移投影。 |
| 发布记录 | path=docs/brief.md | path=minds/brief/article.md | composite key 仍 `(path,target_name)`；URL/status/published_at 不因路径变化重置。 |
| build 输出 | outputs/brief.md | outputs/minds/brief/article.md | 由统一 layout 映射产生；容器隔离同 basename，保留旧 category 输出名。 |

代码现状：`index::derive_store_key` 取 file_stem，`article_output_stem` 只处理 docs/outputs 前缀，prompt/thinking index 只读固定目录，mf-plan 取最后路径组件。实施需统一为内部 WorkspacePaths/解析服务（内部结构名非产品实体），供 create/show/index/lint/build/publish/rename/move/convert/RAG 使用；Skill 只消费返回结果。

`article show --json` 建议增加 `workspace`，返回 layout_mode、article_path、content_shape、container（category 为 null）、prompt/thinking 的实际路径与建议创建路径、共享／局部资料根、build artifact path、绑定状态。不得把“建议位置”伪装成已存在或已绑定。list 保持原 identity，增加 layout_mode 可选字段；既有顶层字段和类型不改。

canonical path／精确 index key 优先。短写收集全部候选后判唯一；目前 resolver 有先命中分支，Stage 03 需补全歧义检查，不能再用 basename 第一条。显示标题即使旧个别命令接受，也只作显式兼容入口：多匹配失败且不据标题生成路径。所有命令用同一结果；F16 不回退到拼 `.md`。

category 中嵌套或声明文章也可能具有同 basename：精确 article_path 仍可独立定位，但旧 stem 对齐不能为它们安全产生两个同名 Prompt/Thinking。首轮保留旧 category 文件约定，冲突时 workspace 明示 `binding_conflict`、不建议覆盖同名文件；要求显式唯一的 Prompt 路径和 `article:` 绑定，Thinking 无法按旧规则唯一对齐时拒绝自动创建并保留缺口，不假称已支持任意绑定。迁往 mind 的预演必须先消除这一歧义，不能把一个旧 thinking 自动复制给两篇文章。新的 mind 路径化投影没有这一 stem 碰撞；C02 同时验证 category 冲突拒绝与 mind 独立成功。

显式声明和 template-origin 文章继续按声明路径识别，不强制搬入 minds。项目内“正常创建布局”只有一种；声明的例外须在 show 标明 origin，迁移预演列入 `excluded_declared` 或由显式映射纳入，不跳过后宣称全项目已迁移。同仓混用至少覆盖两个项目、两形态；不默认支持同项目随意混放两套自动发现树。

## 3. Source／Asset 与共享策略

1. 共享资料默认留项目 sources/assets，两个布局相同。不复制、不 hardlink/symlink 镜像到每个 mind；相对链接引用同一份文件。跨项目共享继续引用登记所在项目，不以迁移暗中重新登记多份。
2. 新的局部资料可显式创建在聚合容器下。建议扩充 `source new --scope article --article <canonical-path>` 与 `asset new --scope article --article <canonical-path>`；省略 scope 仍 project。**现有 source new --article 只表示 imported_by 溯源，不等于所有权／独占，不据它自动搬文件。** category 的 article scope 落入相应共享根下的 slug 子目录；它是归属位置约定，不是访问控制。
3. 由 category 迁往 mind，既有 sources/assets 全部保留共享位置；已确认的独占资料可在预演中显式映射后移动原件，不复制。从 mind 迁回 category，局部资料映射为 `sources/<slug>/…`、`assets/<slug>/…`，所有引用重写。Source 类型／channel／URL／时间戳及 imported_by 全量保留，只有被明确映射的路径和 provenance article 路径变化。
4. 容器局部文件若有容器外入站引用，它就是被共享使用的文件。跨项目移动 Article 前，计划必须显示影响；推荐拒绝自动搬出该共享依赖，先通过扩充后的 source/asset 路径映射能力将其提升到项目共享根再规划 move（现有 rename 仅在当前布局支持范围内可直接复用）。不能为了“全带走”复制源文件，也不能只移动正文后遗留错误相对链接。
5. Source 的权威登记可能在 Lance，YAML 是兼容投影；迁移不能仅改 YAML。Source path／imported_by 映射必须同时更新权威登记与投影，复用现有 dual-write 接口；terms 不得被 source 写回覆盖。无可用权威存储时对涉及该映射的迁移拒绝 apply，预演可返回具体阻塞。仅 embeddings/chunks 作为派生检索缓存另行标记 stale，不能丢失 Source 登记。
6. 共享资料不参与聚合目录的正文 build；private 内容仍可参与作者 RAG，但公开产物只来自 Article 装配和允许资源。搜索结果继续带 project_identity、content_kind、relations、imported_by；新布局不得返回旧路径为已解析关系。迁移后在完成 source sync 前，受影响检索结果显式标记不可用／需同步，不把旧 context 当最新证据。

## 4. 生命周期、预演、引用与恢复（D2）

### 4.1 复用生命周期入口

| 操作 | 计划必须覆盖 | 成功与重复调用 |
| --- | --- | --- |
| category↔mind 迁移 | Article、Prompt、Thinking、显式局部资料、配置、索引投影、发布记录、入站／内部链接、RAG 路径状态 | 完成目标布局后再次请求同一目标返回 changed=false；不再次重写时间戳。双向迁移恢复语义和内容，字节精确恢复仅由 rollback 保证。 |
| article rename | category 改正文 slug 和绑定文件；mind 改容器 slug（正文角色名不变）；发布记录随新 article_path | 标题不改；旧 selector 消失后再次用旧名应 not_found，不伪装成功；通过同一 operation id 重试按 journal 幂等返回。 |
| article move | 同仓两个项目、按目标项目布局重算路径、绑定及记录；项目字段改为目标；检查共享依赖和 target 定义 | 跨仓／跨文件系统首轮不支持。目标 slug 冲突失败；目标 publisher 同名而配置不同／缺失时阻止搬运发布记录，不擅自等价重绑。 |
| article convert | 只改正文形态和路径、Prompt binding、发布记录、相对链接；mind 容器不变 | 继续遵守已有 convert 的可转换条件；已是目标形态 no-op，不能隐式把多 block 合并。 |
| Source/Asset rename/move | 已有命令纳入相同 path mapping 和引用检查，保持登记身份契约 | 只移动明确资料；不从 usage/引用次数猜所有权。 |

聚合模式 article rename 的新 slug 改容器，不把 `article.md` 改名；旧 category 行为保持。rename/move/convert 的默认绑定和发布记录映射是操作组成部分，不要求 Skill 手改索引。

全布局迁移必须改写管理范围内已识别的入站引用。既有 rename/move/convert 增加 `--update-references`：显式开启才改写其他文章，默认保留当前文本处理边界，并返回受影响引用及 `references_updated:false` 警告；不开启不能宣称“全仓无断链”。此为兼容现有调用的方案，用户想要一命令完整维护时使用该标志。

发布记录迁移：逐条用 old→new path map 重绑；目标同 target 已有记录时，同一记录内容完全一致才幂等保留一条（删除重复必须列入计划），URL/status/published_at 任一不同则冲突拒绝，不按更新时间猜。既有历史孤儿键的人工修正建议扩充 `publish update <new-article> --rebind-from <old-path>`，只移动明确旧键、保留字段；必须明确 target、dry-run 和冲突检查。正常 update 不顺带猜测旧路径。独立任意 record delete 首轮不增加。

### 4.2 预演是可审阅计划，不是授权或隐式写入

建议入口：`mf project layout migrate <PROJECT> --to mind|category --dry-run --json`。输出稳定的 `data.plan`，包含：

- plan_version、源／目标布局、old/new path 映射、每个读／写文件的 project-relative 路径、操作种类、前置 SHA-256、预期后置摘要、冲突及 exclusions；元数据变更字段和引用改动片段。
- 配置／声明来源、待重绑发布记录、Source 权威登记变更、缓存失效范围、目标磁盘空间需求；source/assets 未迁移的原因。
- plan_digest 为有序规范化计划摘要，无随机时间导致同输入漂移。它是临时操作校验，不是 Article 稳定 ID。

dry-run 不创建 journal、锁文件、缓存、目标目录，不刷新 index；可用进程内读取一致性检查。需要持久计划时由调用者把 stdout 保存到指定文件，这属于调用者显式重定向，不是 CLI dry-run 隐藏写入。

apply 可使用 `--apply-plan <JSON文件>`；读前置哈希后重新核当前配置／索引／引用扫描清单，变化则拒绝 stale plan。也可直接调用非 dry-run 命令，由它在锁内计算相同计划；无新增工具级人工确认门禁，遵守用户已有授权。`--force` 不能绕过迁移冲突、stale plan 或 schema 不兼容。

### 4.3 引用扫描与可见写集合

扫描本 worktree 内已登记项目的 Article、Prompt、Thinking 及 Markdown Source；包含指向移动路径的 inline/reference links、Markdown image 引用、可解析的本地 HTML src/href。按引用所在文件解析相对路径，再重写至同一目标；保留 query/fragment，不擅改标题锚点。code fence、inline code、普通文本、URL、绝对外部路径保持原字节；wiki-link、自定义模板字符串和动态链接首轮仅报告，不正则猜改。

扫描不会遍历仓库外、Git 内部、输出、缓存或任意未登记业务目录。对于已知入站引用不在授权写范围内、只读文件、语法不能安全重写、symlink 越界，完整迁移必须先报阻塞，不修改这些文件后再说成功。外部仓／网页入站链接不在可证明范围，计划列 `external_references_unverified`；用户不能据本地通过推导全互联网链接不变。

本轮实施验收只用中性 fixtures；未来对真实仓操作必须由其授权范围允许读取扫描区域，否则给出 coverage gap。并发防护对全扫描清单作目录项及内容摘要复核，防止预演后新增引用悄悄漏掉。

### 4.4 可承诺的原子性与恢复

当前 `util::atomic_write` 是单文件 write-rename；`atomic_write_directory` 只解决新建目录；rename 的投影刷新为 best-effort，move 有局部回滚。这不是本节承诺已实现的证据。

建议新增一个**本地文件操作 journal**，仅服务布局／生命周期，不扩成任务编排平台：

1. 单 worktree 的 repo 级 advisory exclusive lock（锁需放在 journal 之外的独立路径，避免只读检测把锁文件误认未完成操作）；所有涉及路径和索引的 mf mutator 遵守它。两个项目 move 使用同一仓锁，避免锁顺序死锁。只读命令检测未完成 journal，返回需恢复而非读半状态。锁与 journal 位于 `.mind-forge/operations/`，不提交 Git。
2. 锁内重新校验计划和目标缺失、大小写／Unicode 规范化碰撞、权限、设备号、所需空间及既有未跟踪文件。无关 dirty 文件可存在；计划涉及文件按其当前字节备份，绝不 stash/reset/clean。目标有未登记同名文件也必须失败。
3. durable journal 状态：prepared→applying→committed 或 recovery_required；先记录备份与计划、fsync，再逐文件同文件系统 atomic rename/write、fsync 父目录，最后提交标记。涉及 Source 权威登记时 journal 同时记录前值／后值与幂等键，校验权威 store 可独占、可恢复；不能让 YAML 完成而 Source 丢映射。
4. 成功只在正文／绑定／配置／索引／发布记录及权威 Source 路径映射完成后返回。派生 RAG 内容索引可以 pending，但必须明确 stale 状态，禁止以旧关系回答“已同步”；离线 source sync 成功才消除该状态。
5. 中途 IO／进程退出后，保留 journal 和备份，返回非零及精确恢复命令；下次 mutator 拒绝继续。建议 `mf project recover <PROJECT> --operation <id> --action resume|rollback`，支持 dry-run，不能强行恢复覆盖人类后续改动。每项仅当当前 hash 等于 before 或 after 才可重放／还原；其他 hash 冲突转人工。committed 的同 operation 重试返回原结果，不重复写时间戳。
6. rollback 恢复此次操作前已有文件和配置、移除仅由本操作新建且 hash 匹配的文件；不删除外部新增内容。备份仅在明确成功核对并由恢复入口显式清理后释放；没有定时自动删历史。

边界：对协作的 mf 进程保证冲突串行化、成功时一致、失败可检测且可恢复；**不保证**原始编辑器／git／旧 mf 在操作中看到瞬时全局快照，不保证磁盘损坏后恢复，也不保证不遵守锁的进程不会在最后检查与写入之间竞争。人类编辑需暂停受影响文件；哈希校验只能缩小而不能消灭这一竞态。跨设备、跨仓、网络文件系统及多 worktree 共用且不能独占的 Source store，首轮 fail closed，不做悄悄 copy-delete。

成本：预演需要扫描受管 Markdown 并计算摘要，复杂度按受管文件总字节量；apply/rollback 需要至多本次改写字节量的备份与暂存（计划估算并预留空间），仓锁在 apply 期间串行化相关写操作。不承诺固定毫秒时延。相较目录整体 swap，本方案保留项目外入站引用和跨项目索引恢复能力，但不包装成数据库 ACID。

### 4.5 空目录持久性（D3）

推荐沿用目前 docs/sources/assets 的必需目录规则：category project new、mind project new 的正文根／共享 sources/assets，以及显式 lint --fix 创建必要目录时写中性 `.gitkeep`。prompts/thinking 和容器局部资料目录仍可选；不为了通过 lint 创建空 Prompt/Thinking。所有扫描器排除 `.gitkeep`，不得把它注册为 Source/Asset/Article。

已存在项目只在显式 new／fix／迁移时处理，不在只读 lint 隐藏写入。不删用户自建占位文件；目录开始有内容后可以保留 `.gitkeep`，不需要自动清理协议。验收用 Git 提交合成 fixture 再检出，lint 不再报目录缺失，且没有新资源登记。

## 5. CLI 确定性能力与 Skill 边界

以下是拟实施契约，不是当前帮助。CLI 命令现状以本仓 `skills/mf-cli/SKILL.md`、生成的 `docs/manual.md` 和已验收证据为准。后续先固定 CLI 候选，再测 Skill；不引入第二个工作流平台。

### 5.1 复用与补缺清单

| 能力 | 当前入口及限制 | 建议变化／确定性边界 |
| --- | --- | --- |
| 文章创建及定位 | `article new <TITLE> --file`、`article list/show --json`；默认 blocks，已有共享 resolver | 复用；new/show 返回 §2.4 workspace，统一双布局发现、短写歧义及产物映射。标题不参与路径重算。 |
| 项目配置 | `project new <PATH> --template`；`config show/schema/default/generate`；project update 当前只改 description | 扩充 new `--layout`，config schema/show 展示项目有效布局及来源；不把 update description 偷换成迁移入口。 |
| 绑定／漂移 | `article index` 对 articles/prompts/thinking 协调；`prompt list/show`、`thinking list/show` 只读；`project lint`、`article lint` | 扩充扫描到真实磁盘、双向绑定、未登记文件、断链；只读 lint 不隐式 index。投影冲突报告所有路径，不自动选择一个。 |
| 生命周期 | `article rename/move/convert`、`source/asset rename/move` | 扩充映射、引用选项与恢复；convert 当前项目级、没有 article positional selector，保留这一点，不在 Skill 中生成虚构单篇语法。 |
| 布局迁移 | **无现有入口** | 新增 §4 的 `project layout migrate` 和 `project recover`，共用生命周期计划／journal；仅处理本地受管文件。 |
| Source／Asset | `source new/index/update`、`asset new/index/update`；`source new --register-only` 当前限项目 sources 内；`source new --article` 是 provenance | 扩充 article scope 和局部根验证，register-only 的允许范围随布局明确增加；保持共享源不复制、Lance 登记与 YAML 投影一致。 |
| 检索 | `source sync --offline`、`source status`、`search` | 复用；更新发现及 context 路径映射，标记缓存失效。sync 不是布局迁移，也不自动写作者判断。 |
| 反馈结构和显式操作 | private Feedback/ Suggestions 目前由 Skill 维护，**无统一反馈状态 CLI** | 新增下节最小 `article feedback` 子树；结构检查纳入 article lint，不新增语义验收引擎。 |
| 术语 | `term new`→`term correction add`、`term lint/fix` | 复用；补 unknown term 的带 scope 示例；增加显式 held-back 展示过滤，不自动采纳 correction、不改 quotes 保护。 |
| 构建 | `build <ARTICLE> --dry-run` 与真实 build；`article lint` 已检查 private 标记 | 复用；产物路径统一、增强相邻 private 诊断行范围；fixture 校验装配／剥离／资源映射。此轮不新增通用终稿命令。 |
| 发布 | `publish target list/show`、`publish run`、`publish update --set`（已含 F16 resolver 修复） | run 消费已构建产物且不等于记录登记；update 增加显式 `--rebind-from`，迁移自动重绑明确映射。保持覆盖默认；不自动根据 mtime 发布／覆盖。 |

lint 建议新增稳定 issue kind：`unindexed_file`、`missing_binding_file`、`broken_local_reference`、`layout_conflict`、`invalid_feedback_structure`；既有 kind 保留。磁盘未登记为 warning，缺目标引用／冲突绑定为 error，无法解析的动态引用为 info 并给 coverage gap。任何自动 fix 都必须有单义修复；歧义绑定、未知链接目标及反馈语义不修。既有 warning 退出规则保留，调用方可用 `--max-warnings 0`。

`term lint --hide-held-back` 建议仅过滤人类文本，仍显示 hidden 数量；`--json` 保持完整 findings，并加 `display_filter` 元数据说明文本过滤请求。summary、退出码、fix selection 不变。这样满足降噪且不破坏 `docs/term-lint.md` 的“advisory never silently discarded”。

### 5.2 反馈下沉的最小接口（全部为建议新增）

复用 Article 的 canonical selector，不建立反馈数据库。article-local `FB-NNN` 是已有线程编号，不能外推成 Article UUID。

- `mf article feedback list <ARTICLE> --json`：读取 private Feedback 与遗留 `mf-feedback`，输出 id、位置、当前 checkbox、原文、回复、延期去向／结构问题。不会猜遗留项已完成；遗留项没有 ID 时按路径＋区间＋内容 hash 定位，仅作一次操作选择器。
- `mf article feedback apply <ARTICLE> --operations <JSON文件> --dry-run --json`：同一明确操作文件去掉 dry-run 才执行。每项含 action、id 或遗留定位、expected_hash、显式 reply/actor/date；action 首轮仅 `migrate-legacy`、`append-reply`、`resolve`、`reopen`、`defer`。未知 action 拒绝。新增反馈正文仍可由 Skill 创作，但 ID 唯一性与结构由 lint 验证。
- `resolve` 必须携带核验说明和证据引用、追加回复再改 checkbox；`reopen` 必须理由；CLI 只验证字段存在、引用格式及历史未改，不判断证据足以解决问题。只有 Skill 对具体内容核验后才能请求 resolve，未解决或部分完成不得提交该动作。此标记也不代表人类最终验收。
- `defer` 必须提供用户明确延期语句／会话出处及原因，不从“没做完”推断授权。将**整个线程**（ID、首行、所有日期回复和先前解决／重开历史）移至现有 Prompt 的 Open Loops，附原锚点；当前 private 区留下无 checkbox 的去向引用。Open Loop 为此线程当前唯一所在地，Thinking 只引用 ID，不复制状态。需要撤销错误延期时，可在没有后续编辑的前提下使用 §4 journal rollback 恢复完整线程；有后续变化则报告冲突并由人审阅修正。首轮不新增通用延期恢复命令，也不把 `reopen` 当跨文件搬回命令。
- `migrate-legacy` 先生成包含逐字原文的新线程或完整规划处理记录，验证写入成功后才移除旧注释；混合规划／写作内容保存完整原文，分别附处理说明／子任务，不丢上下文。同一旧 hash 重复调用返回既有映射，不重复分配 ID。

操作前展示受影响区间、线程前后差异和可见文件写集合，使用 §4 的 hash 前置条件、锁、journal；同一 operations digest 重试可幂等返回。不得覆写首行或已有回复，不提供 `delete-history`／“全部已解决”开关。重复 ID、解析不完整、Prompt 多绑定、expected_hash 过期一律零业务写入失败。dry-run 连 journal 也不写。成功后再次 list 读回，不以模型口述完成代替。

这是小范围 Markdown 状态操作，非通用审稿／审批系统。延期原因和验收证据的真假仍依靠实际会话与审阅；JSON 字段本身不能证明人的授权。是否需要更丰富的线程操作由真实 eval 暴露后另议。

### 5.3 JSON、错误与 dry-run 契约

保留现有成功 envelope `{status, command, data}`，data 为对象；现有错误实际是 `{status:"error", command, error:{kind,message,hint}}`，**不是**成功 data 对象。扩充不改旧字段类型；机器判断依靠 kind/退出码，不能解析自然语言 message。列表 identity 必须被对应 show 和 mutator 原样接受。

新计划的 data 增加 `dry_run`、`changed`、`plan`；实际执行增加 `operation_id`、`recovery_required`、`references_updated`、`cache_state`。跨项目文件定位必须是 `{project_identity,path}`（path 为 project-relative）；仓级配置／journal 用明确 `scope:"repo"` 的路径，不能把两个项目的同路径混为同一文件。`plan_version:1` 标记操作格式，未知版本拒绝；不是业务 schema 或稳定实体 ID。

建议新操作错误在既有 error 中增加可选 `details`（候选列表、冲突路径、operation id、恢复状态），保留 kind/message/hint。不复用 message 作为机器协议：

| 情况 | 退出／kind 建议 | 写入保证 |
| --- | --- | --- |
| 非法输入、未知计划版本、越界配置 | 2 / `usage`；schema 不支持仍 `incompatible_schema` | 无业务写入 |
| 找不到 Article／目标已存在 | 保留 1 / `not_found`、`file_exists` | 无业务写入 |
| 多候选、映射冲突、陈旧计划、锁冲突 | 1 / 新 `ambiguous_selector`、`mapping_conflict`、`stale_plan`、`operation_busy` | apply 前校验拒绝，无业务写入 |
| apply 中 IO／存储失败 | 1 / 原 IO 类 kind；details 指 recovery_required | 可能有部分写入，必须有 journal，不伪装零写入 |
| 未完成操作阻止后续访问 | 1 / 新 `recovery_required` | 不继续业务写入，给明确恢复入口 |
| 成功无变化 | 0 / success，changed=false | 不重写文件、不刷新无关时间戳 |

新 mutator 必须共享 preview/apply 的解析和校验，dry-run 显示同一逻辑计划；环境在随后变化时 apply 拒绝 stale plan，不能承诺脱离时间的绝对同结果。预演冲突仍返回非零并在 error.details 携带计划／冲突，供 UI 审阅。零写入以目录项、文件 bytes/hash、权限等受管状态验证，不能只看 Git diff（Git 看不见所有未跟踪文件）。现有命令未扩充前，不声称已具备这些新增字段。

### 5.4 CLI 契约测试矩阵（未来执行，当前未运行）

| 编号 | 中性 fixture／调用 | 核心断言和失败注入 |
| --- | --- | --- |
| C01 | 未配置 category + 显式 mind，各含 file/blocks，同仓两项目 | new→list/show→index→build→本地 publish→update identity 一致；旧配置输出兼容；私有内容只留源码。 |
| C02 | alpha/beta 的同 basename、同标题、大小写／Unicode 碰撞 | 精确 path 成功、短写歧义列全候选；两个 prompt/thinking 索引项独立，产物不覆盖。F16 selectors 回归原样保留。 |
| C03 | 自定义旧 layout/paths、声明与 generated 文章、旧 CLI | 配置优先级不变，例外文章仍可见；schema 2 旧端拒绝且 bytes 不变；不通过则阻止布局功能发布，不降级掩盖。 |
| C04 | category→mind→category，含共享及局部资料、双 store | 共享文件数/hash 不变；映射、Source provenance、发布 URL/status/time 保留；重复请求 no-op。 |
| C05 | rename/move/convert，跨布局跨项目，含 Markdown/HTML/reference links | 内部与入站链接指向同一目标；不开引用选项有覆盖警告；锚点、代码、外部 URL 原样；target 配置冲突拒绝。 |
| C06 | 所有新 mutator dry-run；目标已存在／只读／symlink／跨设备 | 扫描全 fixture 前后状态一致；无锁、journal、缓存生成；非零错误可解析，写集合无越界。 |
| C07 | plan 后改正文、加引用文件、删索引、并发 mf apply | stale plan／busy 明确失败，不丢新内容；非协作编辑竞争只在已声明边界内验证，不声称全局锁。 |
| C08 | apply 每个持久阶段断进程、Source 权威写失败、磁盘 IO 失败 | 下一读写检测未完成；resume/rollback 幂等，恢复前后 hash 核对；人类后改内容冲突时不覆盖。 |
| C09 | 绑定文件丢失、磁盘未登记、断链、投影孤儿、形态冲突 | lint 分类和严重性准确；只读不 index；不能自动修歧义。 |
| C10 | private 相邻块、title private、资源相对路径、pipeline 失败 | 准确行范围；保留既有剥离语义；标题／非法值拒绝；构建失败不得执行 publish/update。 |
| C11 | feedback migrate/append/resolve/reopen/defer，重复及混合旧注释 | 首行与全部回复逐字保留；部分操作不改其他线程；重复 ID/过期 hash 拒绝；延期完整线程可找回；结构正确不等于语义通过。 |
| C12 | 空项目 Git fixture 创建→提交→新检出；held-back；source missing | `.gitkeep` 不登记、lint 可复现；过滤不改总计/退出码/自动替换；source missing 保留登记及旧 removed 字段。 |

C04/C08 必须涵盖实际受支持的 Source 权威 backend；仅 YAML fixture 通过不能宣传 Lance 迁移可用。若暂不支持某 backend，预演须明确 fail closed，设计验收记录该缺口，不将整个迁移验收为通过。测试仅使用临时中性仓、隔离 store、local target，绝不搬动真实知识库。

## 6. mf-plan／mf-write 改进与真实行为 eval 协议

### 6.1 保留职责，修改交接与调用方式

`mf-plan` 保留问题定义、研究比较、判断、结构与用户选项；规划只产纲要，不因资料够用而写全文。`mf-write` 保留表达、有限范围修订、证据核实、反馈实现和交付；目标／比较逻辑变化交回 plan。两者共享既有 Prompt／Thinking／Source／Article，不新增工程计划 Skill，也不用它们代替本设计的工程规划。

建议改动如下，原 Skill 本轮未修改：

1. 两者先用 list/show 返回的 canonical identity 和 workspace 找文件，去掉取 basename 后拼 prompts/thinking 的规则。缺 CLI workspace 能力时明确兼容限制，不能猜 mind 路径。多绑定先列冲突，不能挑第一个；只有已授权维护且唯一缺失时按 CLI 建议位置创建，随后 index/show 读回。
2. plan 创建新短文时按用户需求和独立维护章节的必要性选择 file；简短说明默认建议 single file，只有明确分块需要或用户指定才选 blocks。已有正文不因“看着短”自动 convert；布局由项目控制，不靠 Skill 临时选另一棵树。
3. 对旧稿＋新材料先作差异映射：新增事实／冲突、受影响章节、作者原判断、已授权改动范围及未决选择，记录到同一 Thinking；Prompt 只存持久约束，Source 登记材料，正文保留作者有效原句。plan 输出修订纲要；write 执行已选章节，不整篇重写或新增第二份 prompt。
4. Suggestions 的选择不因完成而清除，未选项不展开。Feedback 状态由实际实现与核验决定；CLI 操作只执行该明确结论。部分完成仍 open，新增回复写明剩余条件。延期必须有用户明确理由，按 §5.2 移动完整历史，不能用“暂时没做”消除发布阻塞。
5. 用 CLI 完成定位、索引、绑定校验、rename/move/convert、反馈机械修改和 build；Skill 仍直接写作者授权的正文、Prompt 意图、Thinking 判断。不要为了避免所有文件编辑把语义写作也下沉成大而全 CLI。
6. 读取既有授权及范围，已授权且目标已知的动作直接继续；CLI 的 `--yes` 等按真实契约传入。调整当前 write 中笼统“每次 fix 先确认”的表述，改为先预演检查范围、仅在授权缺失或实质变化时询问。不得以 Skill 惯例要求用户重复确认同一授权；已有授权也不能越过本任务明确 Stage03 人工关口。
7. 工具非零即读实际 error，按错误性质修复可恢复前提或报告阻塞。RAG 不可用可以明确降级使用已验证材料；定位／build／publish 失败不能靠降级编造成功。发布有独立明确授权，必须 fresh build→run→验证目标内容；需要登记时再 update 并 show 读回，不能只见 update 成功就说外部发布成功。
8. lint 只证明结构、语法及机械约束。引用来源可信度、反馈满足程度、原文保护、研究判断及编辑质量仍须语义审阅。未决关键意见继续阻塞发布；一般 private aside 无工作流状态。

### 6.2 baseline 与 candidate：同条件运行

此处设计的是后续 Stage04 的**真实模型使用工具完成任务**评测。本次状态为 `not_run`，没有任何模型结果、评分或通过率。已有 mf-plan 3 例、mf-write 4 例保留并纳入；静态 eval JSON、lint、脚本通过不算行为 eval。

冻结方式：

- baseline = 修改前仓内 mf-plan／mf-write（含 assets、references 和已有 evals）的完整快照与 SHA-256；不是禁用 Skill，也不是宿主安装的其他版本。candidate = 改后相同范围快照与 hash。每轮固定两份；再次迭代保留旧结果，不覆盖失败。
- 主对照两臂使用**同一已通过契约测试的 CLI 候选**（含 F16），相同 mf-cli 文档、工具、权限、依赖、系统／宿主提示、上下文及会话授权。这样差异主要归于 plan/write；若另测旧 CLI 对新 CLI，单列端到端产品对照，不能混算为 Skill 收益。
- manifest 固定完整 model ID/revision、reasoning effort、temperature/seed（服务不支持则写 unavailable）、token/时间上限、工具版本、操作系统、CLI binary hash、源码基线与 patch hash、Skill hash、fixture hash、用户消息序列及 grader 版本。模型实际版本在运行前登记；本设计不虚构一个已经使用过的模型。
- 每一 case 的两臂从独立的相同 fixture 拷贝、新上下文开始，不共享上轮答案／文件或记忆。模型允许读本 case 的完整授权记录。样本顺序按事先保存的顺序表交错，禁止只对 candidate 提供额外解释。
- 每个 case 每臂至少 3 次独立运行；固定输入但不声称随机模型逐字复现。触发方式与用户真实调用相同，至少记录是否实际读到 Skill。这里评测任务行为，不另启 description 优化或大规模触发实验。
- 后续运行由已获准的评测执行环境启动，记录真实 transcript、tool calls、stdout/stderr、exit codes、前后文件树与 diff、终稿／产物及最终答复。不要求本会话新 Agent；**本阶段禁止启动任何模型或 Agent eval**。

### 6.3 行为 eval 矩阵

下列提示语是要冻结的中性样本骨架，实施评测时补齐所指 fixture 和 exact prompt；先冻结再运行。不能为了 candidate 结果改题或降低断言。旧 7 例保留：新纲要、重规划去重、旧反馈分流、建议幂等、反馈解决、重开、延期；重复覆盖可以共享 fixture，但旧断言不得悄悄删除。

| 编号／被测 | 真实任务提示与 fixture | 必须观察的行为；主要反例 |
| --- | --- | --- |
| E01 plan | “给项目写一则三段以内的维护说明，先列纲要。”未配置旧项目，无现有文章 | 实际 `article new --file`，正确读取返回路径；只有纲要和必要私有选项，不扩写正文，不更改项目布局。 |
| E02 plan/write | “在该项目继续这篇说明，只处理已选小节。”category/mind 各有 file/blocks 四个固定变体 | show 定位并实际读对 Prompt/Thinking，正文形态不变；无凭 basename 拼路径、无附带迁移。四变体是覆盖要求，不无限扩笛卡尔积。 |
| E03 两者 | 两个 mind 都有 article.md、相同标题；先“继续 article”，另例明确 alpha canonical path | 歧义时列候选、未写任何文章；明确后只改 alpha 且不重复询问选择。beta 全树 hash 不变。 |
| E04 plan→write | “旧稿保留原有判断，加入这份新材料；只改证据节，先梳理变更。”后续消息明确批准该节修订 | plan 映射新证据／冲突，write 按授权只改目标节；同一线程和 Source provenance，可追到事实来源；不编造作者立场、不整篇重写。 |
| E05 write | Feedback 包含两个可核目标，其中一个依赖缺失数据；“先完成能够确认的部分。” | 已完成部分有具体差异和回复，线程仍 open，说明剩余条件；不发 resolve 或发布，不把尝试当完成。 |
| E06 两者 | “FB-002 的统计表明确延期到下月，原因是样本不足；其余继续。”有多次回复／重开历史 | 整个线程转 Open Loop，保留原话、日期和理由及锚点；其他任务继续；对照无延期授权的样本须保持 open。 |
| E07 write | 会话已有“按这份 dry-run 修正该术语，限本节”的授权，目标唯一 | 实际调用 CLI 应用并读回，不再请求同一许可；不扩展 scope／引号保护。另例只有写作授权不能据此 publish。此为通用回归，不是 F18 已归因。 |
| E08 write | “构建并发布到 fixture 内 local 目标。”注入 build IO 失败；另变体 publish FileExists | build 失败无 publish/update；publish 失败不登记成功、不把旧目标当新产物；准确报告失败与未完成项。 |
| E09 两者 | “将该文改名并保留绑定；处理这条已完成的反馈。”CLI 提供 workspace/feedback API | transcript 证明调用 rename 和 feedback apply dry-run/apply；不手改索引、发布记录或复刻路径算法。须核具体参数和最终内容，命令名出现不算通过。 |
| E10 write | 原稿含作者特色原句、未选建议、已选建议和 private aside；“只修第二节重复表述。” | 受保护句和其他节逐字不变，已选建议不重复展开、未选不扩写；仅必要删改，private 内容不流入本地产物。 |
| E11 两者 | legacy 注释含作者提供材料和指令，混合规划／写作；随后用户重开一条 | 遗留原文完整迁移，材料和指令不混写正文；重开保留旧核验回复；重复执行不产生重复 ID、不会删历史。 |
| E12 write | 唯一绑定、全反馈已核实，已知 local 目标并明确授权发布 | 实际 fresh build→publish，核目标 bytes；若任务要求登记才 update→show；不停在计划、lint 或“可以发布”，且不会远端发布。 |

E04/E06/E07/E11 使用固定多轮消息脚本，基线和候选收到相同文本；如果模型先走错分支，不临时追加救援提示帮助某一臂。需要人工选择但脚本未覆盖时记 `needs_input`／未完成，不伪造答复。每个样本都有授权写范围和受保护文件 manifest，区别“没有做错”和“实际完成了请求”。

### 6.4 断言、rubric、失败判定与证据

**机器可验证断言**：CLI argv 与退出码／先后依赖、精确写集合、未触及文件 hash、Feedback 首行和有序历史片段完整性、线程状态、引用目标、实际 artifact 内容／私有标记排除、没有未授权 publish、没有在 fail 后登记成功。使用测试脚本从 transcript 和文件取证，不由受测模型自己打分。hash 仅用于须逐字保留区域，允许编辑区按语义评分。

**语义 rubric**：每维 0–2 分并引证具体行／工具事件。0=违反或无证据，1=部分满足但缺关键步骤，2=完整且有证据。四维为：意图与范围、证据及判断、作者原文／语气保留、反馈处理及交接准确度。plan 不写正文与 write 完成授权写作按各自目标评分，不能以篇幅奖励越界。单 run 通过要求全部硬断言通过、各维至少 1 且总分 ≥7/8；发布、历史丢失、伪造事实／成功等关键违规直接失败，平均分不能抵消。

**判定分类**：`pass`；`behavior_fail`（包括遗漏、无根据 resolve、重复审批导致未执行）；`execution_error`（工具环境／模型服务失败）；`incomplete`（token/时间用尽、必要输入未获得）；`not_run`。只有 pass 计通过。样本预设的工具失败是正常测试刺激：模型正确阻断并报告仍可 pass；意外基础设施故障独立记 execution_error。保留原始失败后两臂按同样规则重跑，不能只挑 candidate 最好一次。证据不足不得给 pass。

**比较门槛**：关键保护断言在 candidate 全部重复中 100% 满足；原有 7 例不得出现新增失败；新能力相关 case 的 run 通过率不低于 baseline，至少一项预先声明的改进（短文形态、路径定位、CLI 下沉或历史保护）由失败转通过，其他语义维度无实质回退。若 baseline 已全部通过，则不能宣称行为提升，只能报告等效／新增契约覆盖；3 次运行是小样本，不声称统计显著。任何关键失败须修订候选再完整比较受影响集合，最后全矩阵验收。

**证据包结构（后续生成）**：

```text
<ignored-eval-workspace>/iteration-N/
├── manifest.json                 # 固定环境、两 Skill 快照 hash、CLI 候选、样本顺序
├── baseline-snapshot/ candidate-snapshot/
├── eval-E01-short-note/
│   ├── eval_metadata.json        # exact prompt、fixture、assertions
│   ├── old_skill/run-1/          # run-2、run-3 同结构
│   │   ├── transcript.jsonl      # 包含真实工具输入输出；不得只留最终答复
│   │   ├── before/ outputs/ diff.patch
│   │   ├── timing.json           # 实际耗时和 token；不可得则注明，不估算
│   │   └── grading.json
│   └── with_skill/run-1/
├── benchmark.json / benchmark.md
└── review.html / feedback.json
```

沿用通用 skill-creator 的 `grading.json.expectations[]` 字段 `text`、`passed`、`evidence`；证据指向 tool event 或文件行与 hash。每例保存完整判定理由、rubric 分数和未验证 claims。报告每臂每例每次结果、mean±stddev、配对 delta、time/tokens，另列关键违规，避免总通过率掩盖数据损坏。由 `eval-viewer/generate_review.py` 基于真实结果生成审阅页（需要时 static），让人同时看输出和评分；不制作装饰性“全部通过”页。

评分者对相同固定 rubric 核 transcript 和输出，可在后续获准环境独立评分或人工审阅；关键历史／越界断言机械验证后再由人复核。分析无区分度断言、波动及代价，不因“文件存在”给分。Captain／用户审阅保留最终验收权；模型 grader 和 CLI resolve 都无权取代。

## 7. 现有 docs 对照、兼容与验收边界

| 现有文档原意 | 对照结论 | 本设计落点及后续文档处理 |
| --- | --- | --- |
| `docs/glossary.md`：Path Identity、Display Title、Selector；Article 独立于形态 | **符合**；聚合容器词义需**补足** | §2.1/2.4 不造 UUID，不把标题当路径；聚合目录只是容器。后续在原 glossary 增补词义，不新增 GLOSSARY/ADR。 |
| `docs/glossary.md`：当前 docs discovery／origin | **需变更／补足** | 扩充为按项目布局的自动发现，保留 declared/generated 及其例外路径；JSON origin 兼容旧值，新细节用可选字段，不静默更换旧枚举。 |
| `docs/schema-naming.md`：Prompt 意图、Thinking 工作记录；目录名不等于实体名；暂不改名 | **符合**；固定 prompts/thinking 目录示例需**补足** | 聚合不更名 Brief/Ledger，不改变 `article` 和 `duplicate_binding` 命名；schema 2 投影键变化属于显式迁移。 |
| `docs/manual.md`：当前命令／flag 自动生成 | **符合**维护方式；新增命令后**需变更** | §5 将当前与建议分开；实施后从 CLI 定义运行 `scripts/generate-manual.sh`，本轮不手改生成手册。 |
| `docs/term-lint.md`：held-back 可见且不可静默丢弃、quote 保护 | **符合默认**；显式文本过滤需**补足** | §5.1 保留完整 JSON／总计／退出码，过滤说明可见；不变更 replacement_eligible 和安全规则。 |
| `docs/build-workflow.md`：定向验证、实际 binary、完整发布前门禁 | **符合** | 本轮仅设计检查，不重复 Stage01；实施定向 C 矩阵，交付前再按实际改动跑所需全套，不把旧构建当新候选。 |
| `docs/retro-074-fix-source-term-bugs.md`：源码与实际安装版本分开、证据和范围明确 | **符合** | §6 manifest 同时固定 binary／patch／Skill；失败保留，不能以版本串代替工作树候选 hash。 |
| `docs/feedback-triage-design.md`：初筛非实现批准、四类判定、设计独立交付 | **符合并替代其候选建议** | §1 用已验收证据更新当前判断，原文不覆盖；不再沿用 F16“路径被拒”作为当前事实。 |
| `docs/feedback-triage-verification.md`：已满足、部分覆盖、待核分开；F18/F19 无证据 | **符合并补足设计** | §1 对每项说明处置及可观察效果；缺口继续待核，未新增原始反馈采集。 |
| `docs/publish-update-fix-verification.md`：F16 resolver 修复、记录重绑另议 | **符合** | §2/4/5 继承新 resolver；绑定／记录迁移是新增设计，不改已验收补丁。 |

相关非 docs 约束：仓内 mf-cli 定义 JSON 与身份 round-trip；mf-plan/write 共用四类 authored stores，Suggestions/Feedback 当前线程语义和显式发布授权继续保留。需修订 basename 推导、延期历史搬移、无条件重复审批和 CLI 调用位置，属于 §6 明确的 Skill 改动，不借此归因 F18/F19。

兼容性必须观察到：零配置 category 无自动迁移、旧 layout/paths 优先级不变、原 file/blocks 均可继续使用；同仓不同项目可异构；共享资料不复制；原发布 URL 和历史不丢。schema 2 的旧客户端拒绝是待验证的准入条件，不是已证实事实。旧客户端若不能在所有相关 mutator 写入前拒绝，必须先补可验证的客户端兼容方案并重新审阅 D1，不能仅增加 schema 数字就上线。

非目标：真实内容迁移、跨仓／跨设备事务、外部链接全网修复、通用工作流／审批平台、语义自动验收、自动 accepted/rejected 推断、图片元数据协议、通用终稿扫描、中文 block 文件名、宿主 ASR 修复、自动清除历史记录、制品发布及远端 PR。

可观察验收分层：本 Stage02 交付是可审阅契约、处置表与评测协议；Stage03 以 C01–C12 的真实 CLI/fixture 证据证明机械能力；Stage04 以冻结候选的 E01–E12＋原 7 例真实运行证明 Skill 行为，附统一候选创建至 local publish 的端到端证据。任何层未跑只能写 not_run，后续测试设计不是本次通过结果。Stage03 必须待用户在原会话确认本设计，当前不会自动开始。

## 8. 最少必要产品取舍：D1–D3（待裁定）

双布局、项目可选、形态正交和真实 eval 已定，不再列选项。下列仅列改变实际体验／兼容成本的未决部分：

| 编号 | 建议方案 | 替代及实际差异 |
| --- | --- | --- |
| **D1 聚合物理约定与兼容门槛** | `minds/<slug>/article.md` 或 `article/`，旁置 prompt/thinking；只由项目显式选 mode；schema 2 + 完整角色路径投影键；先证实旧端安全拒绝 | 使用容器内 `<slug>.md`／`<slug>/` 可减少裸 basename 冲突，但 rename 同时改容器与正文名、仍需路径化 Prompt 键；留 schema 1 可让旧端读文件，但旧端误扫描／覆盖风险不可接受，不推荐。二者都支持双布局，不以替代名义取消它。 |
| **D2 一致性与引用维护成本** | §4 同设备本地 journal、受管引用扫描、恢复及并发 mf 锁；全布局迁移强制引用维护，已有生命周期用显式 `--update-references` 兼容旧写范围；成功时一致、失败可恢复 | 所有 rename/move 默认重写入站引用更省 flag，但会扩大旧调用写集合；只提供“复制工作树后手工回滚”成本低，却不满足完整恢复与登记一致性验收，不能作为等价交付。推荐方案不声称 ACID。 |
| **D3 空目录持久性** | 保留必需目录语义，new／显式 fix／迁移写 `.gitkeep`，所有发现器忽略它；可选 Prompt/Thinking 不造空壳 | 放宽缺失 sources/assets 为合法并懒创建，可减少占位文件，但改变当前 lint 的 error 语义及工具对目录的假设；仅 mkdir 不解决跨检出问题，不推荐。 |

建议回覆格式：`D1 采纳／调整…；D2 采纳／调整…；D3 采纳／调整…`。以上建议尚未获准，不执行配置、迁移或 Skill 修改；任何选择都不隐含发布授权。

## 9. 设计交付核对

本候选续写前半内容，保留领域模型与原子性判断；补足 CLI 清单、反馈历史契约、真实行为 eval 协议、docs 对照和裁定表。前半的实质澄清有：局部资料提升到共享根须依赖扩充后的路径映射，不能把当前 rename 说成已支持新布局；category 旧 stem 绑定遇同 basename 必须显式报冲突，不假称已支持任意 Thinking 绑定；其余新增约束见 §5 的跨项目路径限定和 §7 的旧端拒绝准入条件。任务 dispatch 更新为本次 GC 续接编号。

实际技能用途：domain-modeling 用既有 glossary 压实容器≠Article、provenance≠独占归属、标题≠Path Identity，并用同 basename 与跨容器共享场景检验边界；本仓 mf-cli 用于区分当前命令和新增契约，尤其 convert 的项目级范围、publish run/update 差别及错误 envelope；通用 skill-creator 用于旧 Skill 快照基线、同条件重复运行、断言／rubric、逐例证据与人审协议。没有使用 `.system/skill-creator`，没有运行模型或新 Agent；mf-plan/write 只作为被测对象阅读。

完整本包文件清单：

- `docs/feedback-optimization-design.md`：唯一设计文档，延续已存在未跟踪候选；未来是否提交由后续获准工作决定。
- `.mind-forge/layout-skill-design/candidate.json`：本地候选标识、文档 hash、保护校验摘要及验证范围；忽略目录，不能进入未来业务提交。
- `.mind-forge/layout-skill-design/protected.json`：前任既有的 35 文件保护清单，保持原样，不属本次新增。

此前三份报告、`docs/feedback-triage-evidence/`、F16 三个实现／测试文件和 `.mind-forge/publish-update-fix/` 全部保留，不纳入本设计的写集合。候选 hash 以最终文档 bytes 的 SHA-256 为准，写在本地 candidate.json 和回传中，避免文档自含 hash 产生循环。

本轮验证只做保护文件哈希比对、F16 当前 diff 与原 patch 比对、Markdown 链接／结构及空白检查；不重跑 Stage01 测试，不把设计检查说成 CLI／Skill 通过。原协调日志已只读重读本项及验收，不修改日志、勾选状态或发送外部消息。没有 commit、push、PR、发布、真实迁移；F18/F19 和 D1–D3 继续开放，供 Captain 在原会话审阅。
