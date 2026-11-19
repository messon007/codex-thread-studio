# Agent Teams 开发状态

2026-11-19，`feat/agent-teams`。完整功能已接入 Studio：共享右侧区域内的分层界面、已有会话工具安装、协作任务、原生排队、持久恢复和关闭收尾。默认关闭，未合入 main。实现说明见 [host integration](host-integration.md)，协议见 [specification](specification.md)。

## 使用方式

1. 启动功能分支的 Studio，在主菜单打开 **Agent Teams**，共享右侧区域会展示协作组列表。首次使用进入“协作设置”启用。
2. 在面板预览并安装对应本机 Codex 后端的协作工具。预览会指出实际配置文件、共享此配置的客户端影响，以及仅针对十一项 Teams 工具的批准设置；需用户明确确认。
3. 先输入名称创建空 Group，再从列表逐个添加已有 Codex 会话及其分工（最多 5 个）；添加成功后自动进入成员表并定位新成员。添加成员不设置角色。之后在“修改组”中选择可选的 Orchestrator（默认无，每组最多一位）并启用协作；至少需要两名成员。会话 ID、工作目录和原始上下文不替换；全局 Thread Router 控制会话不能加入。
4. 直接在 Orchestrator 原有聊天区提出任务，不再通过 Tasks 界面派发。启用后的下一条普通用户消息会附带组职责提示；停用或改任后的下一条普通消息会撤回旧角色提示。Agent 可先调用 `team_context({})` 发现自己的组和分工，再直接委派、问答、等待、提交带证据的结果、验收或退回修改。用户也可在面板审阅、处理阻塞和恢复不确定执行。
5. 暂停/关闭停止新的交接；已开始的工作不被打断，精确保留其结果回执。卸载工具是单独操作，待回执结清后只删除 Studio 自己的配置项。

## 已实现的边界

- Rust 独立领域 crate 管理组、任务、依赖、问答、结果、审计、幂等及执行/验收屏障。原生 Turn 结束不会自动判定协作任务完成。
- 忙碌时仅 `thread/queue/add`；Teams 不用 `turn/start`、Steer 或 queue/start 自动唤醒。A 等待某任务时仍可处理其他任务。
- SQLite 保存发送意图和原生回执；可能送达的请求不自动重发。重启按正向原生证据恢复，查无不等于未执行。UI 提供有证据的人工核对和保守停派。
- 普通消息保留原附件和逐轮设置，以版本 CAS 防止多页面覆盖队列；发送采用稳定 ID、共享保留及宿主回执账本，监督任务持有完整逻辑任务的保留。
- 组信息/成员编辑、删除与可选协调者在独立面板管理；任务详情、问答、验收、返工预算、失败执行及结果屏障恢复保留在 Activity 中，已完成工作默认折叠。
- Web UI token 与 MCP credential 分离；调用归属取自宿主观察到的原生工具事件，不接受模型填写调用者。采用单用户、受信 App Server 宿主的边界，不能抵御掌握同等宿主控制凭据的恶意客户端冒充。

## 兼容与验证

已验证基线为 Linux 本机 Codex 0.170.0。只开放同主机后端；WSL/远端后端需要独立的桥接验证，目前拒绝安装，不偷偷共享凭据。现有 thread 的工具装载使用配置版本 CAS + MCP reload，恢复旧会话不覆盖 cwd/model/权限。

验证包括完整 Rust workspace 测试、全部 UI 测试、类型和生成产物检查，以及真实 Studio + Codex App Server 的隔离端到端测试。具体数量、场景和证据见 [最终验收记录](implementation-validation.json) 与 [实现审查](implementation-review.md)。端到端使用合成 Responses provider，证明实际协议、调度和界面路径；没有声称测过真实模型的自主规划质量或所有外部 daemon 拓扑。

工作区、依赖缓存、构建及验证数据均位于 `/media/rui/codex-thread-studio-agent-teams/`。开发可执行文件为 `target/debug/codex-thread-studio`；此分支没有发布 release，也没有修改真实 Codex 配置。历史探针的失败结论保留在 [verification results](verification-results.md)，不能把旧候选或仅领域组件的状态当作当前实现。

界面重设计遵循 [交互规范](../design/interaction-guidelines.md)；详见 [Teams 交互方案](interaction-design.md)。旧版单页弹窗的截图与验收记录是历史证据，新的导航/删除验收另行记录。

2026-11-19 界面重设计验收：Rust 224 项通过（1 项原有助手忽略），UI 840 项通过，真实 Studio/Codex/Chromium 的 23 项隔离端到端场景通过。覆盖共享右侧工具互斥、成员会话的原工具恢复、组删除与取消确认、草稿/校验失败保留、双主题/语言与字体矩阵。见 [重设计验收清单](ui-redesign-validation.json)。此前的 `implementation-validation.json` 为原始实现的历史基线。

2026-11-19 成员流程调整验收：先创建空组，列表入口每次添加一名成员，可选原子指定唯一 orchestrator/owner；已有负责人时禁止重复指定，后续可明确改任或清空。Rust 225 项通过（1 项原有助手忽略），UI 840 项通过，真实 Studio/Codex/Chromium 的 24 项隔离场景通过，包括连续添加与窄屏弹窗。见 [成员流程验收清单](member-flow-validation.json)。

2026-11-19 成员管理重做：组列表直接查看成员；成员页逐人编辑显示名称和职责，通过 Studio 共用菜单打开或移除成员。移除使用真实后端阻断检查，保留原会话；支持并发编辑草稿恢复及移除响应丢失后的状态恢复。GPT-6.2-Astra 对源代码、交互和实际截图复审通过。详见 [审查记录](member-management-review.md) 与 [验收清单](member-management-validation.json)。

2026-11-19 表格界面实现：按用户批准的原型，组列表和成员列表改为严格对齐的表格，固定操作列；标题、返回和菜单复用共享右栏。成员编辑使用铅笔图标，更多菜单保留打开会话和移除；负责人、重命名、暂停/恢复、归档和删除直接位于组菜单。保持原有并发编辑、移除阻断及响应丢失恢复。Rust 227 项通过（1 项原有助手忽略），UI 841 项通过，实际 Studio/Codex/Chromium 隔离端到端 30 项通过。见 [表格界面验收](table-ui-validation.json) 与 [审查记录](table-ui-review.md)。

当前界面微调（用户五项反馈）：Member count 增宽且不换行；总组数移到表尾；添加成员及组设置统一使用简短 Orchestrator 下拉框；组名箭头垂直居中；取消组归档操作和归档筛选入口。旧归档数据仅保留兼容读取和删除，不自动恢复或删除数据。之前的归档截图/验收仅作为历史记录。

五项界面微调验收：GPT-6.2-Astra 复审通过；前端 841 项、类型/生成产物和构建通过，31 项隔离浏览器/协议场景通过，含大字号单行表头、表尾统计、箭头居中、搜索时总数保持不变及协调者下拉选择。详见 [微调验收](table-polish-validation.json)。

最新调整：按用户反馈，将组数量恢复为顶部 `Groups N`，与 `Members N/5` 共用字号、字重、颜色和行高，移除表尾统计。统一校正各级返回箭头和标题对齐，覆盖成员、任务列表/详情、活动及协作设置。前端 841 项及 32 项隔离端到端检查通过；构建已更新，见 [返回入口与统计验收](header-alignment-validation.json)。此前表尾布局记录为历史方案。

返回箭头视觉对齐修正：此前行框居中检查不能证明可见字形居中，实际截图仍差约 2.5px。现按真实基线和可见字形度量对齐，并处理省略标题和原地换字。20 张真实截图按独立像素边界检测，最大中心差 0.5px；两个长标题用例均为 0px。Astra 复核、前端 841 项及隔离端到端 32 项通过；见 [字形对齐验收](glyph-alignment-validation.json)。

2026-11-19 聊天协作入口调整：移除 Tasks 入口和手动派发表单；组名、可选 Orchestrator（默认无）与启停在“修改组”一次保存，添加成员不设置角色。下一次普通 Composer/队列发送附带角色变化提示，关闭/改任/移除后旧角色按同样方式撤回；不自动启动或打断 Turn，Steer 不消耗提示。Activity 保留任务结果和恢复，已离组会话的待确认发送在协作设置恢复。Astra 最终复审通过；Rust 232 项通过（1 项原有忽略），UI 842 项通过，真实 Studio/Codex/Chromium 36 项隔离场景通过。见 [聊天协调说明](chat-coordination.md) 和 [验收记录](chat-coordination-validation.json)。

组列表与编辑提示调整：协作状态移至独立 Status 列；组名后的箭头移除，Actions 中用眼睛图标查看成员，与添加成员图标对齐。修改组提示紧邻相应字段，按当前人数/启用/协调者状态说明条件与行为。窄栏大字号保留组名最小可读宽度，仅表格可横向滚动。Astra 复审、842 项 UI、类型检查和 36 项隔离端到端通过，见 [验收记录](group-columns-validation.json)。

列表快捷操作：Actions 增加铅笔图标直接修改组；Status 改为启停开关，沿用成员人数、全局开关、组 revision 和后台队列边界。回执延迟时可继续编辑，独立的行内请求不会关闭弹窗或清除草稿；失败核对实际状态，不自动重发。842 项 UI、类型检查和 37 项隔离集成检查通过，见 [列表快捷操作验收](group-controls-validation.json)。


已有会话选择器去重：同一 Codex thread ID 可能有多份 rollout 记录，Add member
原先逐条追加 `thread/list` 的返回记录，导致同一会话重复显示。现在在当前后端的
一次列表加载生命周期中按 ID 去重，覆盖单页与追加分页；切换后端或重新打开弹窗
时重置。保留名称/目录相同但 ID 不同的会话、已有成员过滤以及跨后端过期响应防护，
追加页不改变当前选中项。不删除历史记录、不修改会话身份或后端协议。
独立 Chromium 回归脚本见 `scripts/probes/agent-teams/session-picker.mjs`；旧代码
在重复项断言处失败，修复后通过。该检查使用合成响应，不声称完成真实 App Server
或 WebKit 弹出菜单端到端验证。
本次验证：842 项 UI 回归、TypeScript 类型/生成产物检查及隔离选择器检查通过；使用现有 `/media` SDK 完成 feature 分支 debug 构建。未合入 main。
