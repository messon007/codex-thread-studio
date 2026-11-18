# Agent Teams 开发状态

2026-11-19，`feat/agent-teams`。完整功能已接入 Studio：独立面板、已有会话工具安装、协作任务、原生排队、持久恢复和关闭收尾。默认关闭，未合入 main。实现说明见 [host integration](host-integration.md)，协议见 [specification](specification.md)。

## 使用方式

1. 启动功能分支的 Studio，在主菜单打开 **Agent Teams** 并启用。
2. 在面板预览并安装对应本机 Codex 后端的协作工具。预览会指出实际配置文件、共享此配置的客户端影响，以及仅针对十一项 Teams 工具的批准设置；需用户明确确认。
3. 选入 2～5 个已有 Codex 会话，填写分工，可从成员中选一位 orchestrator。会话 ID、工作目录和原始上下文不替换；全局 Thread Router 控制会话不能加入。
4. 创建组目标或向成员委派任务。Agent 可先调用 `team_context({})` 发现自己的组和分工，再直接委派、问答、等待、提交带证据的结果、验收或退回修改。用户也可在面板审阅、处理阻塞和恢复不确定执行。
5. 暂停/关闭停止新的交接；已开始的工作不被打断，精确保留其结果回执。卸载工具是单独操作，待回执结清后只删除 Studio 自己的配置项。

## 已实现的边界

- Rust 独立领域 crate 管理组、任务、依赖、问答、结果、审计、幂等及执行/验收屏障。原生 Turn 结束不会自动判定协作任务完成。
- 忙碌时仅 `thread/queue/add`；Teams 不用 `turn/start`、Steer 或 queue/start 自动唤醒。A 等待某任务时仍可处理其他任务。
- SQLite 保存发送意图和原生回执；可能送达的请求不自动重发。重启按正向原生证据恢复，查无不等于未执行。UI 提供有证据的人工核对和保守停派。
- 普通消息保留原附件和逐轮设置，以版本 CAS 防止多页面覆盖队列；发送采用稳定 ID、共享保留及宿主回执账本，监督任务持有完整逻辑任务的保留。
- 组信息/成员编辑、归档、可选协调者、根目标、任务详情、问答、验收、返工预算、失败执行及结果屏障恢复都在独立面板中。
- Web UI token 与 MCP credential 分离；调用归属取自宿主观察到的原生工具事件，不接受模型填写调用者。采用单用户、受信 App Server 宿主的边界，不能抵御掌握同等宿主控制凭据的恶意客户端冒充。

## 兼容与验证

已验证基线为 Linux 本机 Codex 0.170.0。只开放同主机后端；WSL/远端后端需要独立的桥接验证，目前拒绝安装，不偷偷共享凭据。现有 thread 的工具装载使用配置版本 CAS + MCP reload，恢复旧会话不覆盖 cwd/model/权限。

验证包括完整 Rust workspace 测试、全部 UI 测试、类型和生成产物检查，以及真实 Studio + Codex App Server 的隔离端到端测试。具体数量、场景和证据见 [最终验收记录](implementation-validation.json) 与 [实现审查](implementation-review.md)。端到端使用合成 Responses provider，证明实际协议、调度和界面路径；没有声称测过真实模型的自主规划质量或所有外部 daemon 拓扑。

工作区、依赖缓存、构建及验证数据均位于 `/media/rui/codex-thread-studio-agent-teams/`。开发可执行文件为 `target/debug/codex-thread-studio`；此分支没有发布 release，也没有修改真实 Codex 配置。历史探针的失败结论保留在 [verification results](verification-results.md)，不能把旧候选或仅领域组件的状态当作当前实现。
