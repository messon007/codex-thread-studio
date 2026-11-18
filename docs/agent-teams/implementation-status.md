# Agent Teams 开发状态

2026-11-18，分支 `feat/agent-teams`。本批交付可独立编译、测试的 Rust 领域核心和通用队列组件，**尚不是可在 Studio UI 使用的团队功能**。不合入 main，不接触用户 Codex 配置或现有会话。

## 已实现

独立私有 crate [studio-teams](../../crates/studio-teams/README.md)，只依赖现有 SQLite/Serde/UUID 版本，不引入 GTK、阅读器或网络运行时。

- 2～5 个既有 backend/thread 成员，唯一绑定、Router 排除登记、可选组内协调者字段。
- 委派、子任务/依赖与环检查；任务级 waiting、独立执行 claim、问答、用户决定 blocker、显式提交结果、逐条件验收和返工。等待一个任务不锁会话。
- revision/attempt 校验、组权限与宿主归属、SQLite 事务、确定性业务拒绝回放和幂等冲突；逻辑 effect 与增量审计共同落盘。
- 提交 Turn 和验收 Turn 屏障；同轮续接只豁免当前成员刚验收的直接子任务屏障，不豁免自身返工或普通依赖。原生完成不会自动完成业务任务。
- 精确到问题/blocker 的通知撤回；暂停保留当前工作回执、通知 held。重启/新 Turn 使旧活跃 claim 待核对。
- 独立持久队列：稳定入队 ID、顺序、保留、scope fence、一次发送意图、原生排队/开始/结束回执、确认未提交、unknown 不重发、显式放弃未知项。

原生不确定项的“放弃”仅释放普通人工排队，不证明原生已撤回；迟到回执继续核对，该项未确定前仍阻止严格监督和原 scope 重新启用。

## 已验证

`cargo test -p studio-teams --offline`：30 项集成测试通过；`cargo clippy -p studio-teams --all-targets --offline -- -D warnings` 通过。覆盖跨连接消费互斥、发送前后崩溃、丢确认、明确拒绝、取消/保留先后顺序、迟到回执、权限、错误批次、依赖环、回执屏障、返工、旧 Turn 和写入失败回滚。

磁盘回归使用 40 对各约 2 KiB 的问答：命令回执小于 2 KiB，审计总量小于 400 KiB，checkpoint 后数据库小于 3 MB。没有在每次命令里存整份增长中的任务。新问题预留答案容量，问答内容预算为 256 KiB，旧批次仅计实际内容；答复、取消和结果不受出站队列背压影响。

构建输出约 132 MB，工作区和测试运行数据也在 `/media/rui/codex-thread-studio-agent-teams/`。没有全量桌面构建或其他会话 target 复用。

Astra 已对本批实现完成两轮问题审查和一次定向复核，最终允许提交独立组件里程碑；未宣称真实适配或 UI 验收通过。

## 新版接入探针

[探针](../../scripts/probes/agent-teams/README.md) 继续使用真正的 Codex 0.170.0 App Server、本地合成 Responses SSE 和空配置挂载，无真实模型调用或账户凭据。

仓库保留 [观察摘录与原始记录摘要哈希](probe-observations-v2.json)。`validation/run-04` 的部分 G1b/G2b 结果：

- 对已加载旧 thread 调用 `thread/resume.config` 不会装入新 MCP。服务明确记录 `config overrides were provided and ignored while running`。A/B/C 工具目录均为空；A 原历史仍在，隔离配置文件未变化。
- 目标闲置时，一次 queue/add 会自动执行，不需要 queue/start。
- 目标忙碌时，另一个客户端 queue/add 未向原 Turn 注入输入；精确删除第二个原生待办返回 true。原 Turn 完成后，仅保留的待办执行；末态原生队列为空。

`validation/run-05` 尝试在安全边界 unsubscribe 再按同 ID resume：unsubscribe 成功，但 12 秒观察窗口内 thread 仍在 loaded 列表，故停止，未归档/删除/fork 或重启用户 daemon。不能把 unsubscribe 当作已证实的卸载接口，也不能由此断言它永远不卸载。

这些是部分协议样本，**G1b/G2b 尚未整体通过**。未验证的包括真实模型自主调用、审批/用户输入等待、完整重连和共享 daemon 拓扑。现有测试不放行真实派发。

## 下一批工作与接入边界

1. 验证安全的旧会话工具注册/受管配置路径：隔离测试服务正常退出后，按同一存储/同一原 thread ID 首次载入，检查显式启用的受管配置；不靠 unsubscribe 推断卸载，也不强制重启真实实例；不得静默改全局配置、扩大权限或替换 thread。
2. 实现受信宿主适配、MCP wire schema/工具返回、原生事件关联、单一 worker 所有权及跨数据库 fence 协议。启动/重连先 recover，再使用新证据恢复资格。
3. 将旧普通队列和监督保留迁入共同消费入口，验证多页面、原持久数据、附件/逐轮设置、普通人工发送和 Router 兼容。目前组件没有替换这些入口。
4. 补全控制面：成员编辑/归档、失败执行与结果屏障的人工对账、阶段状态查询、用户预算、协调者根目标路由和对外分页。内部 Rust 类型目前不是完整 MCP 公共接口。
5. 固定版本接入验收通过后，连接独立 Teams UI；默认关闭。保持各阶段可回退，不把组件测试当成端到端通过。

领域 `SetPaused/Cancel` 当前只变更自身数据库，不能对外宣称已撤回原生消息；`effect_ready` 是读取候选，不是跨库原子授权。真实适配必须补足同序 fence 后才使用严格 paused/disabled 状态。这些限制在接入前保持明确，不通过新增 Teams 专用 turn/start 绕过。
