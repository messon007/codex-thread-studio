# Agent Teams：隔离能力验证结果

> 当前实现与验收见 [implementation status](implementation-status.md)、[host integration](host-integration.md) 和 [实现审查](implementation-review.md)。本文保留早期候选/探针的时间顺序及未通过结论；它们不代表最终适配器状态。

本页记录前三轮候选的历史实验，反例仍有效。后续 Astra 已通过 [最小实施方案](implementation-options.md)，允许纯领域开发及 G1b/G2b 验证；下文“不能进入产品开发”是当时旧候选的裁决，不是新方案的当前状态。

2026-11-18，Codex CLI/App Server 0.170.0，设计基线 `f7a8b7f`。范围为 Astra 第二轮批准的 G1/G2；没有产品实现、生产接入或主线合并。

**目前不能进入产品开发。** 已观察到反例，不能把原生队列直接当成幂等的 Teams 队列，也不能把可复制的 MCP metadata 当成模型成员身份。结论限于已测候选适配，不代表所有实现路线不可行。

## 环境和证据

- 专用 worktree：`/media/rui/codex-thread-studio-agent-teams/worktree`，分支 `feat/agent-teams`。
- 原始证据：`/media/rui/codex-thread-studio-agent-teams/validation/run-03/` 中的 `report.json`、`rpc.jsonl`、`model-requests.jsonl`、`mcp-calls.jsonl`、`thread-metadata.jsonl`。仅合成测试内容；临时认证 token 不入库。仓库内 [观察摘录](probe-observations.json) 保留关键事件及原始文件 SHA-256。
- 使用 bubblewrap 隔离网络/PID，将空测试配置挂载到原配置路径。没有重设 HOME/CODEX_HOME、复制用户凭据或连接共享 daemon。所有持久写入位于专用 `/media` 目录。
- 真正的 Codex App Server 处理本地合成 Responses SSE：确定性返回文本、持有响应和 MCP function_call。**没有调用真实 LLM，也没有验证模型自主选择工具**。能够证明已观察到的协议行为，不能代替完整 G1 验收。
- 两个测试 thread 先建立上下文，再安装只读 echo MCP；两个独立 WebSocket 客户端连接同一测试 App Server。CLI 启动报告嵌套 Linux sandbox 探测受限，实验未调用 shell；不能据此声称成员 shell 隔离已验证。
- `run-01` 因旧 ws 客户端附加 Origin 而握手失败；`run-02` 使用错误的扁平工具名，未完成原生 MCP 调用。`run-03` 改用实际模型目录声明的 namespace/name，探针完成。前两次仅作排障记录，不算能力通过。

## G2：原生队列不能直接承担目标契约

测试先保持 A 的原生 Turn 活跃，然后两次发送内容和 `clientUserMessageId` 完全相同的 `thread/queue/add`。

| 检查 | 实际观察 | 能说明什么 |
| --- | --- | --- |
| 重复 add | 返回两个不同的 queuedSubmission.id，list 有两个队列项 | clientUserMessageId 不提供此路径的 add 幂等性 |
| 另一客户端在忙时 start | 返回 -32600，`thread already has an active or pending turn` | 本次竞争样本被拒绝；尚非所有开始路径的原子性证明 |
| 结束原 Turn | 未再显式 start，队列两个副本各自自动开启并完成一轮 | 原生自动消费存在；重复可能真实执行两次 |
| 对照事件 | 两个客户端订阅到相同的两个不同 Turn ID | 不是把同一事件的双客户端副本误算为重复执行 |

G2 当前候选 **未通过**：不能先把未获消费授权的 Teams 正文放入此队列，再靠 Studio 本地暂停/epoch 承诺阻止自动执行。没有实现 Teams 暂停的探针，因此不能宣称“已复现绕过某个已实现的暂停”。取消竞态、监督保留、崩溃恢复、FIFO 全路径仍未验证。后续应先设计受保护的通用队列，另证原生最终 busy 拒绝与回执，不用重试频率补偿缺口。

## G1：metadata 和事件内容匹配不足以认证实际调用

1. MCP 在既有测试 thread 安全轮次边界重载后出现在目录中。
2. 宿主直接调用能传入自定义 callId。
3. 合成 provider 发出 `namespace=mcp__teams_probe, name=echo`，实际 Codex 原生工具路径完成调用并产生 mcpToolCall item 事件。
4. echo 处理中，另一已认证宿主客户端通过 `mcpServer/tool/call` 复制其 arguments 和全部 `_meta`；第二请求同样到达 MCP 并得到结果。
5. 两请求中的 threadId、sessionId、callId、itemId、windowId、turn metadata 和 arguments 相同，progressToken 分别为 2、3。它们不是逐字节相同；现有证据没有提供把 progressToken 绑定到可信原生 item 的认证关系。

这否定了**仅用现有身份元数据加内容匹配**区分重叠 native/direct-call 的充分性。单次领取、先到先得或同业务 request_id 并不能证明结果归属。该反例假定对方可达已认证的宿主直调端点；没有证明普通只读沙箱可获得宿主凭据，也没有证明受保护且禁止该直调路径的另一拓扑必然失败。

G1 当前候选 **未通过**。本探针未实现 Teams 授权服务，不应把 echo 接受请求写成“绕过了已实现的 Teams 鉴权”。子会话、缺事件、跨后端、审批拒绝、旧 generation、真实 LLM 和完整配置保持检查仍未覆盖。可研究可信宿主的逐调用响应通道、给既有 thread 绑定原生工具的支持，或独立设计只消费可信模型事件的异步命令语义；均须修改设计、重新审查后验证。

## 后续边界

完整审查、能力证明和开发仍分开：不会用 Hooks 或新建替代 thread 绕过用户要求。G3 未运行；G4 未开发；不修改现有队列、全局 Router、UI 或生产配置。探针及设计留在特性分支，下一步应针对上述两项缺口选择并审查具体适配路线。
