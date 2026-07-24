export const supportedLanguages = ['system', 'zh-CN', 'en-US']

const english = {
  '全局收藏': 'Global favorites',
  '打开全局收藏': 'Open global favorites',
  '设置': 'Settings',
  '打开设置': 'Open settings',
  '新建会话': 'New session',
  '搜索会话或目录': 'Search sessions or paths',
  '会话': 'Session',
  'Codex 会话': 'Codex session',
  '标题': 'Title',
  '重命名': 'Rename',
  '归档': 'Archive',
  '删除': 'Delete',
  '更多会话操作': 'More session actions',
  '重载': 'Reload',
  '会话信息': 'Session information',
  '结构化 Codex 工作台': 'Structured Codex workspace',
  '消息、命令、文件修改、计划、审批和停止原因都直接来自 Codex App Server，不再解析终端字符。': 'Messages, commands, file changes, plans, approvals, and stop reasons come directly from Codex App Server.',
  '创建第一个会话': 'Create your first session',
  '正在连接…': 'Connecting…',
  '批注': 'Comments',
  '收藏': 'Favorites',
  '添加': 'Add',
  '查看': 'View',
  '重试': 'Retry',
  '停止': 'Stop',
  '发送': 'Send',
  '运行': 'Run',
  '追加意见': 'Steer',
  '向 Codex 发送消息… @ 文件 · $ 技能 · / 命令 · ! Shell': 'Message Codex… @ files · $ skills · / commands · ! shell',
  '将开始一个新的 Turn': 'Starts a new turn',
  '回覆草稿': 'Reply draft',
  '还没有批注': 'No comments yet',
  '在结构化输出中选择文字，再点击“添加批注”。': 'Select text in the structured output, then choose Comments → Add.',
  '整体补充（可选）': 'Overall note (optional)',
  '例如：请先逐项回应，再继续修改。': 'For example: respond to each point before continuing.',
  '清空': 'Clear',
  '插入输入框': 'Insert into composer',
  '只插入输入框，不会自动发送。': 'Only inserts into the composer; it does not send.',
  '跨会话保存的结构化 AI 回复': 'Structured AI responses saved across sessions',
  '搜索标题、正文、问题、标签…': 'Search titles, content, questions, or tags…',
  '还没有收藏': 'No favorites yet',
  '将鼠标移到任意 AI 回复上，点击右上角的收藏按钮。': 'Hover over an AI response and use its favorite button.',
  '收藏保存在本机 Studio 配置目录，与 Codex 会话历史相互独立。': 'Favorites are stored locally and independently from Codex history.',
  '批注选中内容': 'Comment on selection',
  '批注会保留结构化 Turn/Item 锚点。': 'Comments retain their structured turn/item anchors.',
  '你的意见': 'Your comment',
  '说明问题和期望调整。': 'Describe the issue and the expected change.',
  '取消': 'Cancel',
  '加入草稿': 'Add to draft',
  '起始问题独立保存，重命名不会覆盖。': 'The opening question is stored independently and survives renaming.',
  '复制起始问题': 'Copy opening question',
  '完成': 'Done',
  '收藏这条回复': 'Favorite this response',
  '收藏选中内容': 'Favorite selection',
  '编辑收藏': 'Edit favorite',
  '保留结构化消息来源，可随时返回原会话。': 'Keeps the structured source so you can return to the original message.',
  'AI 回复': 'AI response',
  '同时保存本轮问题': 'Also save this turn’s question',
  '以后查看收藏时保留完整问答上下文': 'Keep the complete Q&A context with the favorite',
  '收藏标题': 'Favorite title',
  '标签（可选）': 'Tags (optional)',
  '例如：架构, Rust, 可复用': 'For example: architecture, Rust, reusable',
  '收藏笔记（可选）': 'Favorite note (optional)',
  '记录为什么重要、适用场景或后续行动': 'Record why it matters, use cases, or next actions',
  '保存到收藏': 'Save favorite',
  '保存修改': 'Save changes',
  '收藏详情': 'Favorite details',
  '问题': 'Question',
  '回答': 'Answer',
  '收藏笔记': 'Favorite note',
  '复制内容': 'Copy content',
  '编辑': 'Edit',
  '返回原消息': 'Return to original message',
  '删除收藏': 'Delete favorite',
  '重命名会话': 'Rename session',
  '会话名称': 'Session name',
  '保存': 'Save',
  '创建会话': 'Create session',
  '项目目录': 'Project directory',
  '模型（可选）': 'Model (optional)',
  '使用 Codex 默认模型': 'Use the Codex default model',
  '创建并打开': 'Create and open',
  '界面设置': 'Interface settings',
  '语言': 'Language',
  '跟随系统': 'Use system language',
  '简体中文': 'Simplified Chinese',
  '英文': 'English',
  '主题': 'Theme',
  '浅色': 'Light',
  '深色': 'Dark',
  '内容宽度': 'Content width',
  '舒适': 'Comfortable',
  '宽': 'Wide',
  '全宽': 'Full width',
  '界面字体': 'UI font',
  '代码字体': 'Code font',
  '界面字重': 'UI weight',
  '代码字重': 'Code weight',
  '常规 400': 'Regular 400',
  '中等 500': 'Medium 500',
  '较粗 600': 'Semibold 600',
  '代码字号': 'Code size',
  '增强文字对比度': 'Increase text contrast',
  '加深次要文字和代码输出': 'Darken secondary text and code output',
  '批注提示词模板': 'Comment prompt template',
  '必须包含': 'Must contain',
  '可选': 'optional',
  '恢复默认': 'Restore defaults',
  '后端信息': 'Backend information',
  '关闭': 'Close',
  '运行中': 'Running',
  '执行中': 'In progress',
  '空闲': 'Idle',
  '未加载': 'Not loaded',
  '已停止': 'Stopped',
  '失败': 'Failed',
  '异常': 'Error',
  '已拒绝': 'Declined',
  '未知': 'Unknown',
  '已连接': 'Connected',
  '未连接': 'Disconnected',
  '正在恢复会话…': 'Resuming session…',
  '正在重连事件流…': 'Reconnecting event stream…',
  '起始问题': 'Opening question',
  '从历史提取': 'Extracted from history',
  '已截断': 'Truncated',
  '尚未识别': 'Not identified',
  '当前结构化历史中没有找到用户首条消息。': 'No opening user message was found in the structured history.',
  '状态': 'Status',
  '后端': 'Backend',
  '会话 ID': 'Session ID',
  '项目目录': 'Project directory',
  '未记录': 'Not recorded',
  'Fork 来源': 'Fork source',
  '父会话': 'Parent session',
  '会话树': 'Session tree',
  '来源': 'Source',
  '推理摘要': 'Reasoning summary',
  '执行计划': 'Execution plan',
  '等待差异内容…': 'Waiting for diff…',
  '复制': 'Copy',
  '已复制': 'Copied',
  '拒绝': 'Decline',
  '本会话允许': 'Allow for session',
  '允许本次': 'Allow once',
  '本会话收藏': 'Session favorites',
  '没有匹配结果': 'No matching results',
  '未命名会话': 'Untitled session',
  '未记录项目目录': 'Project directory not recorded',
  '已收藏': 'Favorited',
  '应用': 'Application',
  '版本': 'Version',
  '后端版本': 'Backend version',
  '协议': 'Protocol',
  '传输': 'Transport',
  '正在启动 Codex': 'Starting Codex',
  'Codex 未连接': 'Codex disconnected',
  'WebSocket 连接失败': 'WebSocket connection failed',
  'Codex 已断开': 'Codex disconnected',
  '正在准备重连…': 'Preparing to reconnect…',
  '与本机 Codex App Server 的连接已断开。': 'The connection to the local Codex App Server was lost.',
  'OpenCode Server 未就绪': 'OpenCode Server is not ready',
  'OpenCode 不可用': 'OpenCode unavailable',
  '原生结构化连接': 'Native structured connection',
  'OpenCode 正在重连': 'Reconnecting to OpenCode',
  'SSE 事件流': 'SSE event stream',
  'App Server 已停止': 'App Server stopped',
  'Codex 不可用': 'Codex unavailable',
  '已从 Codex 重新同步会话': 'Session resynchronized from Codex',
  'Codex App Server 尚未就绪': 'Codex App Server is not ready',
  'OpenCode Server 尚未就绪': 'OpenCode Server is not ready',
  'OpenCode 当前没有独立归档操作；可重命名、Fork 或删除会话。': 'OpenCode does not provide a separate archive action. You can rename, fork, or delete the session.',
  'OpenCode 正在运行时不能追加消息；请等待完成或先停止。': 'Messages cannot be added while OpenCode is running. Wait for completion or stop it first.',
  '运行 OpenCode Shell 前请先通过 /model 选择模型。': 'Choose a model with /model before running an OpenCode shell command.',
  '没有匹配的会话': 'No matching sessions',
  '其他会话': 'Other sessions',
  '未记录项目目录': 'Project directory not recorded',
  '会话已刷新': 'Session refreshed',
  '起始问题已复制': 'Opening question copied',
  '非文字输入': 'Non-text input',
  '(非文字输入)': '(non-text input)',
  '批注 {index}': 'Comment {index}',
  '删除批注 {index}': 'Delete comment {index}',
  '正在推理…': 'is reasoning…',
  '网页搜索': 'Web search',
  'Codex 已压缩较早的会话上下文。': 'Codex compacted earlier conversation context.',
  '命令': 'Command',
  '文件修改': 'File changes',
  '个文件': 'files',
  '请求额外权限': 'requests additional permission',
  '正在等待审批': 'is waiting for approval',
  '没有匹配文件': 'No matching files',
  '没有匹配技能': 'No matching skills',
  '没有匹配命令': 'No matching commands',
  '正在由 Codex App Server 搜索文件…': 'Searching files through Codex App Server…',
  '正在由 Codex App Server 发现技能…': 'Discovering skills through Codex App Server…',
  '模型': 'Model',
  '正在从 App Server 读取模型…': 'Loading models from App Server…',
  '没有可用模型。': 'No models are available.',
  '默认': 'Default',
  '使用': 'Use',
  '权限': 'Permissions',
  'OpenCode 权限由项目配置和运行时审批管理；收到权限请求时可允许一次、始终允许或拒绝。': 'OpenCode permissions are managed by project configuration and runtime approvals.',
  '只读': 'Read only',
  '文件只读；需要操作时由 Codex 请求批准': 'Files are read only; Codex requests approval when an action is needed',
  '项目可写': 'Workspace write',
  '允许修改当前项目，网络默认关闭': 'Allows changes in the current project; network is disabled by default',
  '完全访问': 'Full access',
  '关闭沙箱限制；仅用于可信项目': 'Disables sandbox restrictions; use only for trusted projects',
  '选择': 'Select',
  '确认对后续 Turn 使用完全访问权限？': 'Use full access for future turns?',
  '后续 Turn 权限已更新': 'Permissions updated for future turns',
  '目录': 'Directory',
  '推理强度': 'Reasoning effort',
  '审批策略': 'Approval policy',
  '沙箱': 'Sandbox',
  '暂无数据': 'No data',
  '继承会话': 'Inherit session',
  '会话状态': 'Session status',
  '当前 Turn 仍在运行，完成或停止后才能压缩。': 'The current turn is still running. Finish or stop it before compacting.',
  'Codex 已开始压缩会话上下文': 'Codex started compacting the conversation',
  '当前 Turn 仍在运行，完成或停止后才能开始 Review。': 'The current turn is still running. Finish or stop it before starting review.',
  '当前修改': 'Current changes',
  '当前 Turn 还没有可显示的 Diff。': 'The current turn has no diff to display.',
  '正在从 App Server 读取 MCP 状态…': 'Loading MCP status from App Server…',
  '没有配置 MCP Server。': 'No MCP servers are configured.',
  '技能': 'Skills',
  '当前目录没有已启用的技能。': 'No enabled skills are available in this directory.',
  '引用': 'Reference',
  '请先选择一个 Codex 会话。': 'Select a Codex session first.',
  'Shell 命令需等待当前 Turn 完成': 'Shell commands must wait for the current turn',
  '本地 Shell · 不经过模型且不受 Turn sandbox 限制': 'Local shell · bypasses the model and turn sandbox',
  '将通过 turn/steer 加入当前 Turn': 'Will steer the current turn',
  'OpenCode 正在响应；完成或停止后可继续发送': 'OpenCode is responding; send again after it finishes or stops',
  '将通过 turn/start 开始新 Turn': 'Starts a new turn with turn/start',
  '请等待当前 Turn 完成或先停止，再运行本地 Shell 命令。': 'Wait for the current turn to finish or stop it before running a local shell command.',
  '意见已加入当前 Turn': 'Message added to the current turn',
  '已请求停止当前 Turn': 'Requested interruption of the current turn',
  '会话名称已保存': 'Session name saved',
  '会话已归档': 'Session archived',
  '会话已删除': 'Session deleted',
  '归档当前 Codex 会话？': 'Archive the current Codex session?',
  '永久删除当前 Codex 会话及其持久化历史？此操作无法撤销。': 'Permanently delete this Codex session and its stored history? This cannot be undone.',
  '请先在 Codex 输出中选择文字': 'Select text in the Codex output first',
  '请先在 AI 输出中选择文字': 'Select text in the AI output first',
  '无法确定所选文字的消息位置，请在一条回复内选择': 'The selected text could not be anchored. Select within a single response.',
  '选中内容和意见都不能为空。': 'The selected text and comment are required.',
  '每个会话最多保留 32 条批注。': 'A session can keep up to 32 comments.',
  '批注已加入回覆草稿': 'Comment added to the reply draft',
  '清空当前会话的全部批注草稿？': 'Clear all comment drafts for this session?',
  '批注草稿已插入输入框': 'Comment draft inserted into the composer',
  '找到': 'Found',
  '条匹配收藏': 'matching favorites',
  '条当前会话收藏': 'favorites in this session',
  '条跨会话结构化收藏': 'structured favorites across sessions',
  '试试回复中的关键词、会话名称或标签。': 'Try keywords from the response, session name, or tags.',
  '这条回复尚未完成，暂时不能收藏': 'This response is not complete and cannot be favorited yet',
  '字': 'characters',
  '请填写收藏标题。': 'Enter a favorite title.',
  '收藏已更新': 'Favorite updated',
  '已保存到全局收藏': 'Saved to global favorites',
  '收藏内容已复制': 'Favorite copied',
  '收藏已删除': 'Favorite deleted',
  '原会话当前不在会话列表中，可能已归档或删除。收藏内容仍然完整保留。': 'The source session is not in the list and may be archived or deleted. The favorite remains intact.',
  '已返回原会话，但历史中没有找到原消息锚点': 'Returned to the source session, but the original message anchor was not found',
  '等待后端切换超时': 'Timed out waiting for backend switch',
  '批注模板必须包含 {{annotations}}。': 'The comment template must contain {{annotations}}.',
  '已收藏，点击查看': 'Favorited; click to view',
  '未知会话': 'Unknown session',
  'unknown': 'unknown',
  '应用': 'Application',
  '语言': 'Language',
  '跟随系统': 'Use system language',
  '简体中文': 'Simplified Chinese',
  '英文': 'English',
  '文件搜索失败：{message}': 'File search failed: {message}',
  '技能读取失败：{message}': 'Failed to load skills: {message}',
  '尚未支持命令：/{action}': 'Command is not supported yet: /{action}',
  '删除收藏“{title}”？': 'Delete favorite “{title}”?',
  '批注 {index}（{anchor}）\n引用：\n{quote}\n\n我的意见：\n{comment}': 'Comment {index} ({anchor})\nQuote:\n{quote}\n\nMy comment:\n{comment}',
  '批注 {index}\n引用：\n{quote}\n\n我的意见：\n{comment}': 'Comment {index}\nQuote:\n{quote}\n\nMy comment:\n{comment}',
  '整体补充：\n{text}': 'Overall note:\n{text}',
  '还没有 {backend} 会话': 'No {backend} sessions yet',
  '子代理 · {id}': 'Subagent · {id}',
  'Fork · {id}': 'Fork · {id}',
  '会话树 · {id}': 'Session tree · {id}',
  '界面错过了 {count} 条 App Server 事件': 'The UI missed {count} App Server events',
  '界面错过了 {count} 条 App Server 事件，正在从 Codex 重新同步当前会话…': 'The UI missed {count} App Server events and is resynchronizing the current session from Codex…',
  '{backend} 请求额外权限': '{backend} requests additional permission',
  '{backend} 正在等待审批': '{backend} is waiting for approval',
  '{backend} 正在准备此 Turn…': '{backend} is preparing this turn…',
  '{backend} 正在推理…': '{backend} is reasoning…',
  '文件修改 · {count} 个文件': 'File changes · {count} files',
  '已选择模型 {model}{effort}': 'Selected model {model}{effort}',
  '{count} 个技能': '{count} skills',
  '{count} 个文件': '{count} files',
  'Shell 命令已交给 {backend} 执行': 'Shell command sent to {backend}',
  '{backend} 会话已创建': '{backend} session created',
  '已创建 {backend} 会话分支': '{backend} session fork created',
  '找到 {count} 条匹配收藏': 'Found {count} matching favorites',
  '{count} 条当前会话收藏': '{count} favorites in this session',
  '{count} 条跨会话结构化收藏': '{count} structured favorites across sessions',
  '{count} 字': '{count} characters',
  '当前会话还没有可复制的 {backend} 回复。': 'This session has no {backend} response to copy.',
  '已复制最近一条 {backend} 回复': 'Copied the latest {backend} response',
  '新建 {backend} 会话': 'New {backend} session',
  '{backend} Server 无法使用': '{backend} Server unavailable',
  '结构化 {backend} 工作台': 'Structured {backend} workspace',
  '向 {backend} 发送消息… @ 文件 · $ 技能 · / 命令 · ! Shell': 'Message {backend}… @ files · $ skills · / commands · ! shell',
  '创建 {backend} 会话': 'Create {backend} session',
  '由 {backend} 原生服务直接创建并持久化。': 'Created and persisted directly by the native {backend} service.',
  '名称由 {backend} 持久化。': 'The name is persisted by {backend}.',
  'Codex 请求了尚未支持的交互：{method}': 'Codex requested an unsupported interaction: {method}',
  '{method} 请求超时': '{method} request timed out',
  '{method} {path} 请求超时': '{method} {path} request timed out',
  'OpenCode 后端尚未支持 {method}': 'The OpenCode backend does not support {method} yet',
  '无法恢复此 {backend} 会话：{message}': 'Unable to resume this {backend} session: {message}',
  '无法重新同步当前 {backend} 会话：{message}': 'Unable to resynchronize the current {backend} session: {message}',
  '{backend} 默认': '{backend} default',
  '{tools} 个工具 · {resources} 个资源': '{tools} tools · {resources} resources',
  '后端已切换': 'Backend switched',
  '消息、命令、文件修改、计划、审批和停止原因直接来自 Codex App Server。': 'Messages, commands, file changes, plans, approvals, and stop reasons come directly from Codex App Server.',
  '消息、工具、文件修改、权限和停止原因直接来自 OpenCode Server，保留结构化事件。': 'Messages, tools, file changes, permissions, and stop reasons come directly from OpenCode Server as structured events.',
  '可选：provider/model': 'Optional: provider/model',
  '正在启动 OpenCode': 'Starting OpenCode',
  'Fork 自': 'Forked from',
  'OpenCode 后端暂不支持归档': 'The OpenCode backend does not support archiving yet',
  '无法复制代码': 'Unable to copy code',
  ' · 默认': ' · default',
  'OpenCode 权限由项目配置和运行时审批管理；收到权限请求时可允许一次、始终允许或拒绝。': 'OpenCode permissions are managed by project configuration and runtime approvals; requests can be allowed once, always allowed, or denied.',
  '正在由 App Server 发现技能…': 'Discovering skills through App Server…',
  'AI 后端': 'AI backend',
  '新建 Codex 会话': 'New Codex session',
  'Codex App Server 无法使用': 'Codex App Server unavailable',
  '当前会话的 Turn 导航': 'Turn navigation for the current session',
  'Codex 输入建议': 'Codex input suggestions',
  '关闭收藏': 'Close favorites',
  '同时保存这次提问': 'Also save this question',
  '已根据同一 Turn 自动关联，无需手动选择。': 'Automatically linked from the same turn; no manual selection needed.',
  '为这条内容取一个容易搜索的标题': 'Give this content an easy-to-search title',
  '标签': 'Tags',
  '架构, Rust, 值得复用': 'architecture, Rust, reusable',
  '使用逗号分隔，最多 20 个。': 'Separate with commas; up to 20 tags.',
  '笔记（可选）': 'Note (optional)',
  '为什么值得收藏？以后准备怎么使用？': 'Why is this useful, and how will you use it?',
  '你的问题': 'Your question',
  '创建 Codex 会话': 'Create Codex session',
  '由 App Server 直接创建并持久化。': 'Created and persisted directly by App Server.',
  '会话名称（可选）': 'Session name (optional)',
  '例如：实现 Codex 原生界面': 'For example: implement a native Codex UI',
  '必须是本机绝对路径。': 'Must be an absolute local path.',
  '按需询问': 'Ask when needed',
  '从不询问': 'Never ask',
  '不可信命令询问': 'Ask for untrusted commands',
  '项目目录可写': 'Workspace write',
  '名称由 Codex 持久化，并显示在所有原生客户端中。': 'The name is persisted by Codex and shown in all native clients.',
  '保存名称': 'Save name',
  '舒适 · 960px': 'Comfortable · 960px',
  '宽屏 · 1280px': 'Wide · 1280px',
  '全宽 · 自适应': 'Full width · adaptive',
  '；可选': '; optional',
}

const reverseEnglish = Object.fromEntries(Object.entries(english).map(([zh, en]) => [en, zh]))
let preference = 'system'
let locale = 'zh-CN'
let observer = null
const listeners = new Set()
const skippedSelector = '#terminal, #thread-title, #thread-path, .thread-copy, #transcript .markdown-body, .message.user, pre, blockquote, code, .favorite-card > strong, .favorite-card-answer, .favorite-card-question, .favorite-card footer span, #favorite-source-label, #favorite-detail-source, .favorite-answer-preview, .favorite-question-preview, .favorite-question-full, #favorite-detail-title, .favorite-detail-section.answer, [data-no-i18n]'

export function resolveLanguage(value = preference, systemLanguage = globalThis.navigator?.language || 'en-US') {
  if (value === 'zh-CN' || value === 'en-US') return value
  return String(systemLanguage).toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

export function setLanguage(value) {
  preference = supportedLanguages.includes(value) ? value : 'system'
  locale = resolveLanguage(preference)
  if (globalThis.document) {
    document.documentElement.lang = locale
    translateDocument()
  }
  listeners.forEach((listener) => listener({ preference, locale }))
}

export function getLanguagePreference() { return preference }
export function getLocale() { return locale }
export function onLanguageChange(listener) { listeners.add(listener); return () => listeners.delete(listener) }

export function t(source, variables = {}) {
  const translated = locale === 'en-US' ? english[source] || source : reverseEnglish[source] || source
  return Object.entries(variables).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    translated,
  )
}

export function formatDate(value, options) {
  return new Intl.DateTimeFormat(locale, options).format(value)
}

export function inferTextLanguage(value, fallback = 'en-US') {
  const hanCharacters = String(value || '').match(/\p{Script=Han}/gu)?.length || 0
  return hanCharacters >= 4 ? 'zh-CN' : resolveLanguage(fallback)
}

export function migrateLocalizedTemplates(value, legacyValue, fallbackLocale = 'en-US') {
  const templates = Object.fromEntries(['zh-CN', 'en-US'].flatMap((language) => {
    const template = value?.[language]
    return typeof template === 'string' && template.includes('{{annotations}}')
      ? [[language, template.slice(0, 32000)]]
      : []
  }))
  const legacy = typeof legacyValue === 'string' && legacyValue.includes('{{annotations}}')
    ? legacyValue.slice(0, 32000)
    : ''
  if (!legacy) return { templates, legacyLocale: null, migratedLegacy: false }

  const legacyLocale = inferTextLanguage(legacy, fallbackLocale)
  let migratedLegacy = false
  if (!templates[legacyLocale]) {
    const misplacedLocale = legacyLocale === 'zh-CN' ? 'en-US' : 'zh-CN'
    if (templates[misplacedLocale] === legacy) {
      delete templates[misplacedLocale]
    }
    templates[legacyLocale] = legacy
    migratedLegacy = true
  }
  return { templates, legacyLocale, migratedLegacy }
}

export function translateDocument(root = document.body) {
  if (root) translateNode(root)
}

export function startTranslationObserver() {
  if (!globalThis.MutationObserver || observer) return
  observer = new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === 'characterData') translateTextNode(record.target)
      else record.addedNodes.forEach(translateNode)
      if (record.type === 'attributes') translateAttribute(record.target, record.attributeName)
    }
  })
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
    attributeFilter: ['placeholder', 'title', 'aria-label'],
  })
}

function translateNode(node) {
  if (node.nodeType === Node.TEXT_NODE) return translateTextNode(node)
  if (node.nodeType !== Node.ELEMENT_NODE || node.matches(skippedSelector)) return
  for (const attribute of ['placeholder', 'title', 'aria-label']) translateAttribute(node, attribute)
  node.childNodes.forEach(translateNode)
}

function translateTextNode(node) {
  if (!node.parentElement || node.parentElement.closest(skippedSelector)) return
  const raw = node.nodeValue
  const core = raw.trim()
  if (!core) return
  const translated = t(core)
  if (translated !== core) node.nodeValue = raw.replace(core, translated)
}

function translateAttribute(element, name) {
  if (!element.hasAttribute?.(name) || element.closest(skippedSelector)) return
  const value = element.getAttribute(name)
  const translated = t(value)
  if (translated !== value) element.setAttribute(name, translated)
}

export const translationEntries = Object.freeze({ 'en-US': english })
