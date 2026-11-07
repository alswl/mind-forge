# layout-lifecycle 当前进展

实现及本地验证完成，待 Captain 独立验收。Captain 两项契约返修已纳入，根项目支持及默认 CLI 准备中断恢复已补齐。
当前全量 2520 passed、0 failed、2 既有
ignored；fmt/clippy/build/diff 检查均通过。最终文件清单、绑定、故障覆盖及边界见
[验证报告](layout-lifecycle-verification.md)，命令契约见 [使用说明](layout-lifecycle.md)。

历史 ad300fef9a0f 与 2513 项结果完整保留；返修候选和证据在
`.agents/layout-lifecycle/revision-1/`。自定义 layout 根属于旧 Article 配置，
仅配置不会拒绝转换；含未分配旧资料则拒绝，不推断归属。

下面保留原事务层断点记录作为历史；其中“未完成/下一会话”的说明对应 e60a1dd19aed，
不代表当前实现状态。原始检查点另保留在本地忽略目录。

---

# layout-lifecycle 实施断点

任务 `mf-layout-lifecycle-20261025` / `layout-lifecycle`，基于已验收候选
`ff4e90518d03`。**本包未完成，不能作为转换/生命周期功能验收候选。**
本次在事务底层实现完成的安全边界交接，遵守 80% 上下文前换会话约定。

## 已实现

新增 `src/service/mind_lifecycle/transaction.rs`，提供本机文件事务原语：

- 只读 inventory 和 preview；列出每个变更路径、操作和 before/after hash。
- 计划绑定 canonical worktree root、完整观察范围快照与内容摘要。
- 同一文件系统检查、已知本地文件系统白名单、符号/硬链接拒绝。
- 使用 `.mind-forge/write.lock` 的非阻塞独占协作锁。
- 在任何受管文件变化前，将 before/after 内容持久化至 journal；备份权限为私有。
- 先复制目的内容，再删除原路径；保存二进制字节和 Unix 权限。
- 恢复不信任步骤计数器，只接受磁盘上的 before 或 after 状态。
- resume/rollback 幂等；后续编辑、新增文件、错误 worktree、计划/journal 篡改拒绝。
- 声明 Lance 或检测到现有 Lance store 时明确返回
  `source_backend_unsupported`；没有假称已恢复 Source 权威登记。
- 新增稳定 `MfError::Lifecycle` code，沿既有错误 envelope 输出。

事务模块尚未暴露给 CLI，暂有模块级 `allow(dead_code)`；计划器接入后应移除。
首包正常命令的行为尚未接入此锁/recovery gate，不宣称已有全 CLI 并发保护。

## 本次测试

10 项事务单元测试通过。其中单个文件集有 7 个变化操作，覆盖备份后、
每个变化后、完成标记后的注入；所有未完成阶段分别测试 resume 和 rollback，
还测试 rollback 中断后再次恢复。包含新增入站文件使旧计划失效、保护后续编辑、
写锁竞争、journal 的 `.next` 完整镜像恢复和 Lance 拒绝。

故障注入是事务操作阶段级测试，不等同于每个系统调用或断电窗口测试。
尚未完成真实转换计划器、引用维护、Source/index/publish 计划的故障注入。

本次命令与原始日志位于 `.agents/layout-lifecycle/`：
`transaction-tests.log`、`clippy.log`、`checkpoint-results.json`。
首包基线与文件 hash 已保存在同目录，未覆盖首包证据。

断点验证：`cargo test --locked --quiet --bin mf --test cli_mind_output --test cli_publish_update --test e2e`
通过，1083 passed、0 failed、2 既有 ignored；`cargo fmt --all --check`、
`cargo clippy --all-targets --locked -- -D warnings`、`cargo build --locked`、
`git diff --check` 均 exit 0。本包最终完整回归尚未运行。

首次扩大单元回归观察到一次锁释放竞争（预期 invalid_journal，实际 write_locked）；
已使用显式解锁的 RAII guard 修复，后续上述回归通过。过程见
`.agents/layout-lifecycle/intermediate-failure.md`。

## 下一会话必须继续

1. 实现角色/additional inventory 和 category↔bundle 全量路径映射；识别未知归属并拒绝。
2. 实现 mind/output rename、跨项目 move、delete；标题保持，默认 output、空 mind 登记、
   输出投影、发布记录、YAML-authoritative Sources/Assets 登记一起生成写集合。
3. 实现受管范围 Markdown links/images/reference definitions、HTML src/href 的重写。
   既有 markdown helper 不足以安全覆盖这些语法，不能直接把它当完整引用解析器。
   删除有外部入站引用要列引用者并拒绝；动态/无法确定的受影响引用拒绝。
4. 接 CLI preview/apply/resume/rollback；预演不写计划文件也不建锁，apply 重新校验。
   所有 mf 受管写命令需遵守同一锁；未完成 journal 要阻止普通读写并提示恢复。
5. 显式标记 RAG stale；保持 Lance 明确拒绝，不只改 YAML 投影。
6. 补 CLI/e2e 双向转换及生命周期矩阵、每阶段故障与新引用/新 additional 过期计划、
   后续编辑和幂等测试；运行完整回归，更新帮助/手册/契约与最终验证报告。

需审查的底层边界：初始目录已建但 journal 尚未完整写入时如何安全恢复/清理；
文件/目录权限变化中间窗口；全 CLI 锁接入；watch roots 对未知新文件、受管根新增和
项目内工具目录的覆盖。当前原语不支持文件/目录原地替换，规划时应作为冲突拒绝。
未知/网络/overlay 文件系统当前保守拒绝。

本次未新增业务命令，没有真实迁移、外部发布、commit/push/PR，也未启动其他模型或
子 Agent。实际沿用技能 `mf-cli`。独立验收仍由 Captain 在完整包完成后接续。
