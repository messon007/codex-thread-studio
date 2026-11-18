# Agent Teams 设计审查

2026-11-18，GPT-6.2-Astra。审查对象为 `feat/agent-teams` 基线 `f7a8b7f` 上的 [specification.md](specification.md)，正文 SHA-256 为 `06315d262371b800af996b478847d1f2f5b14b91228883fe2fb144c7ee8c30aa`。取证范围为仓库源码、已有 0.170.0 协议 schema 和官方文档。本次只作静态审查。

**完整方案裁决为 CHANGES_REQUIRED。** 保留最多五个既有 thread、任务级等待、可选组内协调者、成员直接协作、独立 Rust 服务及事务 outbox。把以下 P0/P1 写入规范后复审，再决定是否运行文末两个受限的能力实验。当前未批准产品开发。业务接口可以明确规定的行为，应在实验前规定。

## P0 · 消费权、排队次序与开始边界

**在今天的队列上，另一个页面或外部 CLI 能在 Studio 的检查与发送之间取得会话。** 普通队列由页面保存暂停/运行集合，检查该页监督对象与模型活动状态，在异步准备后执行 `turn/start`，每次重取随机 `clientUserMessageId`。接受成功后才移除并持久化消息，Rust 按会话替换整个 JSON 数组，两个旧快照可互相覆盖。监督执行权只在某个页面的 Map 中；上一轮正常结束和监督作出下一步决定之间确有空档。当前队列归一化最多三条、正文最多 64 KiB。后台订阅只转发四类生命周期事件，其广播可丢失；Rust 实例锁也不能锁住独立 daemon 客户端。[普通发送和消费](../../ui-src/submission-controller.mts) · [执行确认](../../ui-src/queue-execution.mts) · [持久化](../../src-tauri/src/session_state.rs) · [队列格式](../../ui-src/message-queue.mts) · [监督所有权](../../ui-src/turn-supervision.mts) · [daemon 与事件桥](../../src-tauri/src/codex_app_server.rs)

0.170.0 生成的 `ThreadQueueAddParams` 有 `threadId/clientUserMessageId/input`，返回的 `QueuedSubmission` 有服务端 id。重复 add 的语义没有写在 schema 中。`ThreadQueueStartParams` 只有 threadId 和可选 queuedSubmissionId，响应含 Turn。`thread/queue/changed` 仅有 threadId；删除返回布尔。没有声明以预期队首、有效性 epoch、监督保留为条件的原子消费，也没有声明“删除 false 即已开始”。列队、消失和收到相同文本都不能单独证明具体消费结果。

**所需变更**

1. 为既有队列定义通用消费契约。每条已接纳输入有稳定消息 ID、每会话的有序位置、来源、当前状态；用户输入与 Teams 共用排序和唯一消费裁决。明确 outbox 等待背压期间尚未取得队列位置，进入后保留既有人工输入相对顺序。历史页面与新消费方互斥，禁止页面按旧整表覆盖新的队列项。保留原有人工直接发送和人工主动 Steer 的权限语义。
2. 定义线性化点。按会话在同一权威消费边界检查当前队首、人工暂停、审批/输入等待、失败停顿、监督保留、无活跃 Turn、来源有效性与派发代数，然后以稳定 ID 取得不可重复的开始权。暂停、取消、返工、关停和领取按同一顺序裁决，撤销先赢则留在 held/withdrawn；领取先赢则返回含 receipt 的“在途”，承诺不强停。写 DB 后再发 RPC 之间的窗口单列为已授权但执行未确认，保留可对账记录，不能向用户报告底层 Turn 已启动。区分明确未入队、已有待消费、已经开始、已知终止与证据不足；未知冻结自动重发。请求投递密钥/epoch 只用于有效性，不代表人工授权。
3. 优先核实原生队列能否为所有入口提供该边界，包括其他 CLI、队列自动推进、手动 start、编辑/重排、断线重连。若不能，则普通 Studio 队列需要一个后端单一消费方、持久化暂停/监督保留和受同一序列约束的撤销。只有仍能由原生端原子拒绝正忙的并发开始，并阻止外部消费者抢走/编辑尚待授权的 Teams 项时，这个实现才成立。把 Teams 工作保存在权威队列受保护的条目中，在确认可接受前不把可直接执行的正文暴露给无门控消费者；若无法与原生开始建立这条边界，该后端能力应为 unavailable。只给 Studio 建锁或“先读 idle 再发起”仍不足。这里可能需要独立的通用队列变更，应单列范围与兼容验收。Teams 侧保持 enqueue/lookup/withdraw/subscribe，无 Teams 专用启动、无 Router 决策变更。[Router 的既有派发路径](../../ui/thread-router-controller.mjs)
4. 后台队列将暂停/保留状态及持有者世代持久化，各 UI 接收快照和游标；失去 owner 时按明确释放条件处理，不能凭短租约推断监督已结束。跨库撤销必须共享一个可证明的序列化协议，例如队列权威层原子撤销可消费凭证，Teams 使变更在收到 durable fence 确认后才对外报告已停；碰撞和进程中断留待对账。仅在两个数据库分别写 epoch 会留下窗口。
5. `UserPromptSubmit` 可作为附加实验。签名封套须绑定 backend/thread、delivery、消息摘要、attempt、epoch；签名验证只证明来源。消费方须原子核对撤销、完整正文与一次性领取。官方把该事件列在 Turn 内，参数是活动 turn_id 和即将提交的 prompt。能够返回 block 仅证明可以拒绝 prompt，未证明此前没有创建/抢占 Turn、所有队列路径必经 hook、hook 缺失或被禁用时能拒绝 Teams 消费。人工复制签名消息、编辑标记、hook 超时、进程停止、外部 CLI 重载和抢先消费都不能破坏边界。若正文标记可被剥离，不能据“看起来是普通文本”向模型释放原 Teams 工作。即便发现有效的 prompt 层拦截，仍须独立证实队首/不抢占与监督保留。[官方 Hooks](https://learn.chatgpt.com/docs/hooks)

## P0 · 成员身份与已有会话工具加载

**裁决：Hooks＋普通 CLI 不作为 v1 的成员命令通道。** 官方说明子 Agent 的 hook 可收到父 session_id；Pre/PostToolUse 的 turn_id、tool_use_id 与命令只在宿主 hook 事件中有意义。另一个普通进程调用 CLI 时，这些字符串、工作目录、命令文件、共享账户的套接字或环境变量均无法证明“被授权的成员在该 Turn 调用了这一条命令”。把执行请求交给受信宿主、另建逐次授权代理并校验参数才可能补齐，此时已引入另一种工具桥。Hooks 用于有限上下文提示即可，v1 可完全省掉。现有 Studio gateway bearer 代表实例用户权限，不能放进成员 shell 或协作 MCP。[官方 Hooks](https://learn.chatgpt.com/docs/hooks) · [Studio 授权](../../src-tauri/src/gateway_security.rs)

**仅将 MCP 作为优先验证的既有会话适配候选。** 0.170.0 只有 thread/start 有 dynamicTools，恢复同一个曾经有该工具的 thread 与给旧 thread 加工具是两件事。Studio 的动态工具示例创建于 thread/start，处理发生在浏览器。官方支持按 thread 检查 MCP 目录、宿主直调工具，工具可用性必须在原 thread、安全轮次边界和真实模型调用中分别取证。[现有创建代码](../../ui-src/session-map.mts) · [现有接收代码](../../ui-src/active-codex-connection.mts) · [App Server](https://learn.chatgpt.com/docs/app-server) · [MCP 配置](https://developers.openai.com/codex/mcp)

**所需变更**

1. 规定受信来源和支持配置。独立的 backend/instance 专属认证通道对接 MCP，认证材料及签发能力只对受信宿主可用，成员的 shell、仓库和任务载荷均不能读取。若现有会话的权限、远端拓扑、沙箱或同用户进程可等价访问宿主认证材料、Teams DB 或用户控制面，则该配置不能声称成员隔离成立；保留原会话权限并给出未就绪原因。组内 UI 的 `user` 定义为实例授权用户，UI 和成员端点授权分开。绑定持久 backend 身份与 thread，进程 generation 只隔离一次连接。
2. 受信 App Server 入站事件与真实 MCP 调用逐次匹配，候选键为 `instance/backend/generation/threadId/turnId/nativeCallId`，另核对 server、tool、规范化业务参数摘要和成员绑定。关联以受信宿主实际批准的调用为准，事务内一次性取得对应调用并记录领域命令，防止重放/串单。先收到任一半时只允许短暂有界等待；缺事件、世代切换、过期、审批拒绝、参数不一致或可疑重复时拒绝。业务 request_id 与原生 call id 独立；下一次真正调用可用同一 request_id 获取已提交结果。宿主生成且可认证的逐调用证明也可满足相同性质。
3. 上述键在 0.170.0 能否从两侧取得、item.id 是否等价于 MCP callId、Studio 能否看到别的客户端发出的真实事件，以及客户端直调能否产生同样的可信证据，均未证实。已有 schema 的 `McpServerToolCallParams` 接受调用方提供的 `_meta/threadId/tool/arguments`；`ItemStartedNotification` 的 MCP item 带 server/tool/arguments/id，外层带 threadId/turnId。Rust 当前单独 lifecycle 流不含 item 事件。不得使用消费前客户端通知、任意 HTTP 元数据、历史文本或“同一时段该会话正忙”作为证明。主动直调即便携带相同参数也必须无法冒充成员，包括合法请求正在 in-flight 时；若直接调用与模型调用不可区分，需要上游不可伪造的逐调用绑定或受保护传输。向实际 caller 返回，只关联到了内容相同的合法 intent 仍可能泄露读结果。对读写操作都执行来源校验。
4. 绑定、卸载、恢复均只改本组件所有且指纹匹配的注册；并发编辑用比较再写，配置作用域和宿主重载均有记录。登记前后核对原 thread 身份、权限、cwd、模型、既有历史。任一目标配置差异需要扩大权限/新开 thread 时保持未就绪。禁用时先断新授权，在途只保留明确列出的回执，排空或移交人工处置后再卸载。

## P1 · 必须先定下的领域契约

| 位置 | 可复现的协议矛盾或缺口 | 应写入规范的决定 |
| --- | --- | --- |
| §4、6.3 | 用户能 resume，但 resume 强制要求有成员的当前 Turn；多个外部 blocker 只能在全部可解时一次处理；当前 get 未承诺返回 blocker ID。 | 分出 `resolve_blocker`，有权用户/外部阻塞所属执行人仅解除指定项，needs_user 仅由匹配的用户决定解除。返回 blocker_id、类型、当前状态及 revision；最后一个解除时产生一次 ready 通知。只有 assignee 在受信 Turn 中 begin/resume 取得 claim。ready 与 working 分开。 |
| §4、6.3、6.5 | working 任务可跨 Turn，但没有持久归属和恢复领取动作；用户发来的普通 Turn 也能提前接手。 | 定义每 task/attempt 的执行 Turn claim 和释放/换绑。一次 Turn 可处理不同任务；一个任务同时最多一个有效 claim。恢复 working 复用同一 attempt，先确认原 Turn 已终止，再以 revision 显式领取；异常/未知由用户核对。submit/checkpoint 等须验证当前适用 claim；清理尚未开始的重复续接。委派时 requester continuation 与 assignee checkpoint 分字段保存。 |
| §6.5–6.6 | 等提交者 Turn 结束只控制验收通知；reviewer 直接读取 submitted 并调用 review 即可提前通过。用户验收没有 Turn，用户在 UI 直接 review 时也会遇到“等验收 Turn 结束”。 | 将提交 Turn 的 settled-success 条件落实到 review 命令本身；失败/中断明确列为人工核对，再按记录的结果放行或要求返工。成员验收绑定真实 review Turn，用户验收用用户事务作为边界。生成工作从属哪个屏障须落盘；依赖有多个完成事件时覆盖所有适用屏障。 |
| §6.2–6.7 | 现有子任务使父 waiting，取消子任务会使父永久不 ready；任意成员又可成为问题接收人。remove_member 仅检查任务/交付，已送达的 open 问题可能无人可答。 | 写清同组依赖方向、取消前置的显式人工处理（v1 可取消/重建下游）、取消父后子保持原 requester/可验收、外部 blocker/父依赖的原子更新、多个父项更新 revision 和事件。移除/归档同时检查 open question、claim、未知/在途以及待审结果；协调者被移除必须原子清空或改指活跃成员。 |
| §4、6.4、6.7 | 合并“答复+ready”与 `Delivery.source_event_id` 单值无法说明多个原因；同一 attempt 可多次等待和解除，按 attempt 去重会吞后续续接。 | 用 task/attempt/wait_generation、recipient、purpose 标识一次 ready 变迁，记录合并来源或同一事务的 causation ID。答复步骤与执行资格独立，已答非阻塞问题不再唤醒终态/旧 attempt。若要求引用被更正问题，则 ask 定义 `related_question_id?`。 |
| §5、7、9、11 | 幂等结构只覆盖已有 group；create_group 与全局开关尚无 group_id，开关/暂停/成员变更等参数不完整，用户无法在关闭后通过已隐藏页面重新开启。 | 分 instance 和 group scope，create/set_enabled 使用实例稳定用户主体；全局开关用 settings_revision，其他组写均用 expected_group_revision，包括 pause/resume/archive。明确方法名、枚举、返回新版本与 errors。create 时用输入 session_ref 指示协调者并返回 member 映射；组目标以期望 roster/revision 在事务内解析指定协调者。保留独立设置及关停回执/对账入口。 |
| §5、6、7、12 | 不可变任务标准 ID 由谁生成含混，唯一键与 read/list/history 校验不全；返工次数限额和用户追加没有可调用操作。 | 明示 criterion_id 是创建方给定且任务内唯一或后端分配并返回，一次完整覆盖且无重复。逐一列 read 组内可见范围、actor filter 对 user 的表达、命令摘要规范（含 operation 和版本）、认证先于返回 replay、并发同键成功一次。确定性拒绝是否缓存及生命周期须统一；超时保留原键，权限撤销后拒绝不泄露历史。分页绑定 scope、排序、水印；补用户有界追加返工命令或 v1 到限额只允许用户手动决策。 |
| §5.3、6.6、12 | 限制“每成员 100 条未完成 delivery”可能使 answer、review、cancel、关停等解除阻塞的事务因需通知而失败；输入各 32 KiB，组合验收正文可破普通队列 64 KiB，静默截断。 | 对产生新工作的命令限流；释放/回执必须可提交，用有界持久事件与按需投递游标、合并/保留容量及明确背压策略。消息只放固定上限的摘要与 ID，旧会话能读取完整权威对象；验收条件保留完整正文读取途径。列出每个 text/array/envelope 的上限与编码单位。 |
| §8、9、11 | 已关闭但尚可回执，MCP 却准备卸载；相同旧 attempt 不能证明结果来自关闭前已开始的工作；未知 delivery 无用户处置协议，会挡住归档。 | 关闭事务冻结派发世代并快照已开始的 `member/task/attempt/nativeTurn/claim` 回执资格，只允许原提交结果或幂等回读；已取消、已失效 Turn 与新委派始终拒绝。保持限权被动入口至相关 Turn 终结/可人工恢复，保留失败原因；明确暂停时允许 ask/answer/review 等但 effect held，恢复只投有效项、重新绑定已停用的权限。提供用户对账命令、证据、revision 与审计；无法证明安全只允许放弃/标记不确定，不能自动再启。 |

建议在修订版列出一张可执行的命令矩阵，逐项覆盖 actor、caller/claim、状态、attempt/revision、活跃/暂停/关闭与允许的 effects，并列出每类消费回执的唯一性与丢失恢复。执行失败、批准失败、限额拒绝、持久化失败和未知投递各有独立状态。组内其他成员能否看到全部任务也应作为明确的入组可见性约定。后端通过唯一约束和同一事务落实 2～5 人、同一实际 backend/thread 的唯一未归档绑定、排除当前和历史 Router 控制会话、依赖环与不可变结果；控制器不能经普通选择器作为一般成员间接加入。

## 复审后可单独放行的隔离验证设计

此处给出可限定范围的实验计划，**本轮没有运行实验，也没有批准实现**。先修订规范和验证协议，由设计复审独立裁决是否放行。使用相互隔离的临时配置、凭据、测试服务和两个预先存在的可恢复测试 thread；每个 thread 写有可检查的原上下文标记和 cwd。不得指向用户真实配置/会话或共享生产 daemon。实验只用无业务副作用的小工具和标记消息，无 Teams 产品端点、数据库迁移或 UI 开发。

1. **身份与装载**。在测试 thread 已存在后安装并重载只读 echo/whoami 适配，先记录目录、再记录宿主直调、最后记录模型调用三种证据。做两条并发调用（允许相同工具和参数），从两个独立受信来源提取并核对上述字段。覆盖子会话、审批拒绝、host direct-call、重试、缺事件、错代数、关闭及重启，检查不能取得另一调用的结果，且不改变原模型/cwd/权限/历史。重载只有安全轮次边界生效。若无法认证两侧或区分来源即停止；一次 host 直调成功、读到了 threadId 或模型声称“完成”均不足。
2. **消费边界**。固定同一原生 thread、两个独立客户端与一个确定性协调栅栏，区分 Studio 普通队列、原生队列与 Teams 暂存位置。记录入队、消费、人工/CLI 竞争、turn ID、完整 transcript、相邻人工项 FIFO、撤销与回执。交替让 revoke/claim 先提交，在 RPC 前后断开、重复 add、撤回与 start 并发，以及监督两轮间、审批中、用户输入中、失败暂停、页面退出时逐项观察。禁止用 Steer/interrupt 模拟“忙时成功”。若兼测 hook，只对签名无害标记，逐项验证缺失/禁用/超时、外部客户端消费、签名重复/修改及拒绝时是否已改变原 Turn 或丢掉队列项。即使模型没有看到标记，若出现多余抢占 Turn、吞掉人工输入、越过监督或无法确定记录，判不通过。单次实验成功只说明覆盖到的运行路径；须另有固定版本实现或受支持的协议保证说明线性化位置。

已核阅同期新增的 [verification-plan.md](verification-plan.md)。G1、G2 可分别对应上述两个实验，复审前应补充同参数并发、合法调用与直调重叠时的读结果归属、旧 thread 配置保持、拒绝时的多余 Turn 与人工消息丢失检查。G3 留到 G2 确有需要时另批。G4 是基础设施范围决策，需要独立设计和审查。

两项结果独立归档。任何一项失败均保留其余设计，明确版本/后端不支持；改路线须先改契约再复审。两项均通过也只提供能力证据，完整开发另按修订后的总体审查决定。用户已指定磁盘约束，后续开发 worktree、构建产物、验证配置和缓存放在 `/media/rui` 专用目录；本次仅在轻量审查 worktree 写本报告。功能继续隔离在 `feat/agent-teams`，合入 main 需用户另行指示。

## 第二轮复审 · G1、G2 隔离验证设计通过

**裁决为 PASS，授权范围仅限 G1 和 G2 的隔离能力验证。** 2026-11-18，GPT-6.2-Astra。复审基线为 `feat/agent-teams` 的 `f7a8b7f`，审查文件为 [修订规范](specification.md)（SHA-256 `361ea52c8972d988a397a1f8e13702fda53b98d25035104c81a976e275ff38a6`）和 [验证计划](verification-plan.md)（SHA-256 `2aebe345c804713f386380cea48aedf2dbf584cc1e561b7153df769b83202638`）。本轮核阅了迁移后的文档，无运行证据。总体产品开发仍须复审 G1/G2 的证据和下表中的产品契约。G3 需要单独批准；G4 为通用队列基础的范围决策，需独立设计审查。

| 初审事项 | 第二轮判定 |
| --- | --- |
| P0 消费权、暂停和外部竞争 | §8.1.1–8.1.2 明确定义权威队首、持久领取与实际开始两种状态、busy 的原子拒绝、人工与监督保留、跨库 fence receipt 和崩溃恢复。源正文在受保护队列内等待。G2 有可证伪的边界，实际 0.170.0 支持性待取证。 |
| P0 逐调用身份与装载 | §10.3.1 对认证通道、可信事件、相同参数并发、宿主与模型调用重叠、结果归属和不支持拓扑给出明确规则。旧 thread 在装载前已存在，验证保持原身份、cwd、模型、权限与上下文。普通 CLI 已退出 v1，G1 可验证候选 MCP。 |
| P1 任务执行与回执 | §4、6.3、6.6、6.8 与 §11 已拆分解除 blocker、resume 和 reclaim，并要求当前 claim、独立检查提交屏障、关闭白名单和被动回执。成员及协调者移除检查覆盖 open question、待验结果与在途引用。尚待收口的 review-Turn 核对见下文。 |
| P1 父子关系、等待与去重 | 已规定取消依赖的人工处理、取消父后子保留 requester、父修订、wait_generation、答复与 ready 的因果合并。需覆盖“waiting 且已 ready、尚未 resume 时又添加 blocker”的边沿：原 ready 凭证应失效；下一次从 blocked 到 ready 有独立 generation。 |
| P1 控制面与容量 | 已定义实例与组幂等 scope、组/任务/设置的不同版本、成员可见性、分页水印、协调者解析、用户预算和未知交付处置。限额已允许回答、提交、验收和取消持久化；派生投递和阶段响应还需统一返回契约。 |

**本次可执行的工作**仅包含专用 `/media/rui/codex-thread-studio-agent-teams/` 目录内的隔离测试配置、无业务副作用的探针和合成日志。G1 可在自建隔离 App Server 与测试 threads 中加载只读 inspect/echo，分别检查工具目录、真实模型调用及宿主直调的来源。G2 可由两个独立测试客户端对同一隔离原生队列投送无副作用标记并观察队首、忙碌、撤回、断线、审批、监督及人工消息。保留配置与进程的真实隔离，不能靠线程不同来隔离同一个生产 daemon。G1 中重放只对同一个已认证的合法请求返回结果；缺失来源证明时拒绝并记录。G2 实验结论区分样本与固定版本实现/协议保证。发现反例可立即判定对应必要条件失败；无法建立所需绑定、消费边界或隔离环境时记录未通过或未知，停止依赖该条件的后续步骤。

**总体开发前的契约修订**

1. §6.6 规定验收者 Turn 失败时冻结后续交付，§7 的 reconcile_result 处理提交者屏障，reconcile_execution 处理 assignee claim。应让用户用明确的 review/barrier ID 核对验收者的 terminal-failed/interrupted 事件、打开指定屏障或保留冻结，审计证据并保留已提交 verdict。所有直接 begin/resume 也应检查依赖的同一组屏障；对当前验收 Turn 的本轮续接仅限已明示的同会话例外。
2. §8.1.2 的 mutation_intent 允许先返回进行中、再完成，而 §5.3 规定同键返回原响应。定义同一 request_id 的稳定 operation/intent ID，区分阶段响应与唯一终态响应；明确处理中的查询、并发重试、重启和 fence 失败。把 `fence_scope` 与 authorized/in-flight 的撤回结果写进 QueuePort 签名，保证调用方能表达“已领取，原生尚未确认”。
3. §5.3 已允许先落盘事件，稍后才物化 Delivery，而部分命令示例和 §6.6 立即返回 delivery_id。规定稳定逻辑 effect ID 与迟后产生的队列 receipt 的关系，保证事务重放的响应一致。§5.3 的 8 KiB 摘要与 §6.5、§10 的投送描述应统一为“有界摘要 + 权威读取”。明确每字段的大小约束或通用默认上限，并将调用者生成的 request_id/criterion_id 明列为 §4.1 服务生成 ID 规则的例外。
4. 区分 §10.4、§14 中后续业务端到端测试与本次两个能力实验。当前授权不覆盖委派、任务写 API、队列改造、工作流 UI 或生产接入。探针结果与审查通过的计划须分别标明版本、入口和证据，再决定后续开发范围。

## 第三轮复审 · 0.170.0 隔离实验结果

**本轮总体裁决为 CHANGES_REQUIRED。G1 当前身份判据失败，G2 直接使用原生队列的候选失败；产品开发继续等待替代设计与证据。** 第二轮的 PASS 是运行两项限定实验的许可。这里复核的是 `validation/run-03` 的原始记录，没有重复执行实验。两项必要条件各有一个反例，足以停止相应候选；继续付费模型测试或穷举竞态不能消除这些反例。

| 对象 | 已确认事实 | 结论边界 |
| --- | --- | --- |
| G1 旧会话装载 | 两个测试 thread 均先完成包含上下文标记的初始 Turn。重载后受测的 A 已能列出 echo，合成 Responses 流通过真实 Codex 模型响应处理路径产生 MCP 工具项。末态读取保留两个原 ID、各自 cwd 和模型标识，thread 元数据为 CLI 0.170.0。 | 证明此配置中至少一个既有 thread 的工具装载和执行路径。回读未包含完整 turns，未验证所有旧 thread 的权限及历史恢复、真实 LLM 自主选工具、远端部署或完整生命周期。 |
| G1 请求来源 | 合法模型路径和有权接入同一测试 App Server 的第二个宿主客户端，在重叠窗口内分别抵达同一只读 MCP 服务。接收端记录的工具、业务参数、关联原生调用的身份字段一致；两条请求的进度标识不同。 | “可信事件存在 + 相同身份字段/参数 + 后端级连接”不足以验证当前请求或其返回通道。进度标识与原生批准调用之间没有已证明的可信绑定。此次使用 stdio MCP，未测试规划中的 HTTP 端点或独立逐调用签发机制。实验以第二个已认证宿主客户端为前提，未证明普通成员 shell 持有该权限，也未观察到真实业务数据外泄。 |
| G2 原生队列 | 同一客户端消息 ID 的两次 add 获得不同原生条目 ID，列表中同时存在。忙碌时单次显式 start 收到“已有活跃或待开始 Turn”的拒绝。原 Turn 结束后，两条相同标记各自启动并完成一个不同 Turn，队列随后为空；中间没有新的显式 start。 | 在本路径中，clientUserMessageId 不提供 add 去重；入原生队列后可自动执行，Studio 无法在每次消费前单独授予该候选所需的开始权。没有观察到向忙碌 Turn 注入标记。一次忙时拒绝不足以证明所有与外部 CLI 的竞态均安全；没有实测生产审批、监督、跨库 fence 和重启对账，保留未知。 |

**阻塞的必要性。** 一个会话可以同时参与不同任务，也可以带着自己的审批与 cwd 从其他客户端继续工作。允许任意有效宿主调用占用另一个成员的读取或写入授权，会破坏组内责任归属；把尚可撤销的工作交给会自动推进的原生队列，会使暂停、监督续接及取消失去确定的边界。这两项都来自产品已接受的约束。无需将“所有 MCP 传输均不安全”“所有忙时操作都抢占”或“原生队列必须自己去重”列为额外前置要求；应用可以在正确的信任和消费边界上实现来源证明及持久去重。

**最小后续路线。** 身份侧优先寻找能将一次已批准模型调用及其响应通道绑定在受信宿主内的固定版本接口，并要求可在安全轮次边界绑定原有 thread。仅当能够证明成员与其他非受信客户端均无法使用宿主级直调、且目标拓扑仍支持用户从原客户端正常使用会话时，才可另审更窄的权限模型。队列侧保留 Teams 的事务 outbox，单列通用 QueuePort：持久稳定 ID 与排序、受保护待办、撤销与领取共用 fence、人工与监督保留，由同一个通用入口消费。已有原生队列在自动消费前不存放未获授权的 Teams 正文。原生最终开始还须有可核对的原子忙时拒绝及恢复关联；若这些条件无法证明，能力维持 unavailable。上述均为下一次设计审查的候选，继续使用原有会话，无须新增 Router 或为抽样反例运行真实 LLM。G3、G4 及产品变更的授权边界沿用第二轮裁决。

证据索引按 NDJSON 文件行号定位。队列见 rpc 行 65–76、83–144，装载与原生工具项见行 145–173，以及末态读取行 193–196。两个客户端记录同一 Turn 时，以原生 Turn ID 去重。MCP 接收记录见第 2、3 行，合成模型记录见第 6、7 行。原始文件保留在任务专用磁盘目录，摘要仅引用合成数据。[App Server 收发记录](../../../validation/run-03/rpc.jsonl) · [MCP 接收记录](../../../validation/run-03/mcp-calls.jsonl) · [合成模型请求](../../../validation/run-03/model-requests.jsonl) · [测试汇总](../../../validation/run-03/report.json)

复核同期修订的 [验证结果摘要](verification-results.md) 与 [产品规范](specification.md)。结果摘要准确标出 stdio 探针所观察的身份字段、进度标识差异、宿主客户端前提和 G2 的两次实际执行；以原始 run-03 为准，历史 report.json 中“indistinguishable”的表述仅适用于所用身份判据。规范已补充第二轮要求的 reconcile_review 接口和同一屏障检查、ready 再次失效与分代、稳定 operation_id 和阶段重放、effect_id 与延迟物化的区别、QueuePort 的 fence/已领取回执，以及 8 KiB 消息摘要。第二轮列出的这批文档修订已完成。总体裁决仍由本轮两项运行时缺口决定。可移植证据摘要见仓库内 verification-results.md；上段原始记录链接对应本机专用磁盘布局。
