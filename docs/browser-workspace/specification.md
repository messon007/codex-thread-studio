# Browser Workspace 特性规格

状态：Phase 0、Phase 1、Phase 2A 已完成；Phase 2B Wayland 内嵌 Browser Workspace 开发中

> 2026-08 产品决策：Studio 默认且仅使用内嵌 Browser Workspace。外部 Chromium/Brave
> 进程管理、CDP 连接和对应 fallback 已从运行时代码删除。本文中 Phase 1 的外部浏览器
> 内容仅保留为历史设计记录，不再代表当前实现或后续验收目标。
适用产品：Codex Thread Studio
最后更新：2026-08-11

## 1. 决策摘要

Browser Workspace 将浏览器能力作为 Studio 级全局服务加入 Codex Thread Studio，但不替换现有文档审阅器。产品中长期保留三个明确分离的运行边界：

1. **文档审阅器**：安全展示 Markdown、静态 HTML、文本和图片，允许搜索、选择和批注，不执行项目 HTML。
2. **人工浏览器**：由 Studio 启动并管理一个带界面的 Chromium 系浏览器；真实网页、标签和常规浏览器操作留在浏览器窗口，不在 Studio 重复展示。
3. **AI 浏览器**：后续让 Playwright MCP 连接同一个专用 Chromium Profile，向 Codex/OpenCode 提供结构化浏览器工具。

第一阶段不打包或维护 Chromium fork，也不再使用应用层 GtkFixed/child WebView 修补方案。Studio 与浏览器可以是两个物理窗口，但只有一个位于“本机工作区”的全局产品入口。Linux Wayland 下由用户使用桌面分屏快捷键并排窗口。第二阶段保留 WRY 内嵌方案，但必须在 WRY/Tauri Runtime 的正确原生容器层解决，并单独制定跨平台支持矩阵。

## 2. 背景与问题

当前文档审阅器会读取项目目录中的 UTF-8 文件，使用 DOMPurify 删除主动内容，再把结果显示在 Studio 的审阅区。HTML 预览会删除 `script`、`style`、`iframe`、表单、媒体和外部资源属性。这一设计适合阅读和批注不可信文档，但它不是浏览器，不能正确运行应用页面或访问普通网站。

用户需要以下新能力：

- 在聊天旁打开网页、文档站点、GitHub 页面和 localhost 开发服务；
- 将项目中的 HTML 作为真实页面运行，同时保留现有安全预览；
- 后续让 AI 查看 DOM、截图、Console 和 Network，并在批准后点击、输入和测试页面；
- 将当前页面、选择文本、截图或错误证据明确加入当前会话；
- 复用 Codex/OpenCode 已有的 MCP 能力，不再为浏览器另建模型调用链路。

成熟产品的参考方向：

- [Cursor Browser](https://cursor.com/docs/agent/tools/browser) 将浏览器显示、截图、Console、Network、审批和域名策略组合成一个受控工作面。
- [Windsurf Browser](https://devin.ai/blog/windsurf-wave-10-browser) 使用 Chromium fork，并把标签页、DOM 和日志纳入人机共享时间线。
- [Playwright MCP](https://github.com/microsoft/playwright-mcp) 提供持久 Profile、隔离上下文、浏览器扩展连接和标准 MCP 工具。
- [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) 允许本地 Codex 客户端通过 STDIO 或 Streamable HTTP 连接第三方工具。

## 3. 目标与非目标

### 3.1 产品目标

- 在不破坏聊天宽度和现有右侧审阅交互的前提下，提供简洁的 Browser 工作区。
- 对 HTML 文件同时提供“安全预览”和“在浏览器运行”，让用户明确选择信任级别。
- Studio 全局只拥有一个 Managed Browser 进程和一个持久 Profile；Thread/Workspace 只记录标签引用和策略上下文，不能触发新的浏览器实例。
- AI 通过结构化工具读取和操作浏览器，浏览器内容不依赖终端文本解析。
- Codex 与 OpenCode 尽量共享同一套浏览器工具契约。
- 采用渐进交付：先完成安全基础和人工浏览，再加入 AI 自动化。

### 3.2 工程目标

- 远程网页只运行在独立 Chromium 进程中，不进入 Main WebView，也不获得 Studio 的 Tauri capability、Gateway 凭据或文件系统能力。
- Studio Gateway 对 HTTP 和 WebSocket 都具备不可猜测的会话凭据及 Origin 校验。
- 项目 HTML 由独立 Origin 提供，不能与 Studio UI 共享 Gateway Origin。
- 浏览器控制器与具体渲染后端解耦，允许后续增加 Browser Backend。
- 高频 Browser Event 不直接重绘完整聊天记录，也不自动消耗模型上下文。

### 3.3 非目标

首批版本不承诺：

- 成为 Chrome、Firefox 或 Safari 的完整替代品；
- 浏览器扩展商店、同步、书签服务、密码管理器或浏览器账号体系；
- 绕过网站的登录、反自动化、CSP、DRM 或机器人限制；
- 自动读取用户日常浏览器的全部标签页和 Cookie；
- 在第一阶段让 Tauri WebView 与 Playwright Chromium 共享同一页面实例；
- 自动把完整 DOM、所有 Console 或 Network 内容加入每一轮提示词；
- 默认允许 AI 在任意互联网或局域网网站上无人值守操作。

## 4. 术语与边界

| 术语 | 定义 |
| --- | --- |
| Review | 当前安全文档审阅器；内容渲染在 Studio UI 中，不执行项目页面 |
| Browser Workspace | 全局浏览器进程、Profile、事件流和未来 Agent Bridge 的总称；不是右侧面板名称 |
| Managed Tab | 专用 Chromium Profile 中由 Studio 通过本机 CDP 枚举和管理的普通页面 |
| Agent Tab | 后续由 Playwright 管理的 Chromium 页面；目标是复用同一 Managed Tab |
| Workspace | 规范化后的项目根目录；用于页面引用、localhost 和 AI 权限策略，不拥有浏览器进程或 Profile |
| Browser Profile | Cookie、LocalStorage、IndexedDB、Cache 等浏览器持久状态 |
| Attach | 用户明确把 URL、选择文本、截图或日志引用加入会话上下文 |
| Handoff | 把 Viewer Tab 的 URL 复制到 Agent Browser；不表示登录状态也被复制 |

## 5. 体验设计

### 5.1 入口与布局

Browser Workspace 不占用会话顶部和右侧工作区。唯一固定入口位于“本机工作区”菜单，与“全局收藏”并列；入口以一个状态点和 `default` Profile 名称表达离线、启动中、已连接或异常。点击离线入口启动浏览器，点击已连接入口显示现有浏览器。

后续入口包括：

- Markdown/HTML 中的普通链接；
- `@` 文件搜索结果中 HTML 文件的“运行”操作；
- 文档审阅器中 HTML 文件的“在浏览器运行”；
- AI 返回的 localhost 或 HTTP(S) URL。

浏览器自己的标签、地址栏、前进后退、下载、权限和 DevTools 继续使用 Chromium 成熟界面。Studio 不复制这些信息。Profile 完整路径和启动参数只在设置中的诊断区域展示。

### 5.2 HTML 文件行为

HTML 文件提供两个语义不同的动作：

- **预览**：继续使用当前 DOMPurify 安全渲染，可搜索、选择、批注；
- **运行**：在隔离的项目 Preview Origin 中加载完整 HTML、CSS、图片和 JavaScript。

首次在一个 Workspace 中运行主动内容时显示一次信任说明。信任记录以 Workspace 路径和配置版本为键；项目目录变化或安全策略升级后可以要求重新确认。

### 5.3 会话与 Workspace

- 全部 Thread 共享 Studio 唯一的 Managed Browser、Profile 和标签页集合。
- Thread 可以保存最近关联的 Tab ID，但不能拥有、启动或停止浏览器进程。
- 删除或切换 Thread 不关闭标签页，也不删除全局 Profile。
- 清除浏览数据是显式操作，并展示影响范围。
- 应用重启后恢复标签元数据；是否恢复页面由配置和上次退出状态决定。

### 5.4 Comment 与加入聊天

Browser 内容只有通过用户显式操作才进入会话，并复用现有 Comment 窗口和 Draft：

- 加入当前页面：标题、URL 和可选的简短选择文本；
- 加入截图：生成本地图片工件，并作为结构化图片输入；
- 加入选择：保存可见文本、URL 和可选 DOM 定位信息；
- 加入 Console/Network：保存完整日志文件，只把摘要和文件路径加入聊天。

普通人工浏览、滚动和地址栏输入不自动产生聊天消息。Comment Core 不感知 Browser；Browser Provider 独立负责 URL/选区锚点和返回网页。删除 Browser Provider 不影响 Chat、Document 或未来 EPUB Comment。

## 6. 总体架构

```text
Codex Thread Studio
├── Main WebView
│   ├── Thread / Turn UI
│   ├── Document Review
│   └── Local workspace / Browser global entry
│
├── Browser Controller (Rust)
│   ├── Chromium discovery and process lifecycle
│   ├── navigation and lifecycle policy
│   ├── persistent Profile manager
│   └── loopback CDP client
│
├── Managed Chromium (separate top-level window)
│   ├── normal browser UI
│   ├── one Studio-wide user-data-dir
│   └── DevToolsActivePort on loopback
│
└── Agent Bridge (Phase 3)
    └── Playwright MCP → same Chromium Workspace
        ├── DOM and accessibility snapshot
        ├── screenshots
        ├── console and network logs
        └── approved interaction tools
```

### 6.1 Main WebView 与外部 Chromium

Main WebView 只渲染 Studio UI。普通外部网站既不放入 iframe，也不作为 Tauri child WebView 添加到主窗口。iframe 会被 `X-Frame-Options` 或 CSP `frame-ancestors` 拒绝；Linux Wayland 上应用层 child WebView 还会产生层级、定位、菜单遮挡、调整宽度和空白页等不稳定行为。

Studio 启动系统已有的 Chromium、Chrome、Brave 或 Edge，传入 Studio 全局专用的 `--user-data-dir` 与动态 `--remote-debugging-port=0`。无论从哪个 Thread 进入 Browser，都复用同一进程、Profile 和标签页。Studio 重启后从 Profile 内的 `DevToolsActivePort` 重新连接仍在运行的浏览器，不要求浏览器随 Studio 退出。

Wayland 普通客户端不能可靠设置其他顶层窗口的全局位置。第一版展示 `Super+Left/Right` 分屏提示；未来自动分屏通过可选桌面环境适配器实现，不写入 Browser Controller 核心。

允许导航的 scheme：`https`、`http` 和专用 Project Preview Origin。默认拒绝 `file`、`javascript`、`data`、`tauri` 和未知自定义 scheme。弹窗、下载、外部协议和权限申请必须由 Browser Controller 决策。

### 6.2 Browser Controller

Browser Controller 是唯一可以发现/启动浏览器、分配 Profile、读取 DevTools endpoint 和调用 CDP HTTP discovery API 的组件。前端只发送声明式命令，不获得调试端口或 WebSocket endpoint。

建议的内部命令：

```text
browser.workspace_status
browser.launch_workspace
browser.list_tabs
browser.create_tab
browser.activate_tab
browser.close_tab
browser.clear_workspace_data (future)
browser.attach_to_agent (Phase 3)
```

Phase 1 从首次状态快照开始即使用 CDP WebSocket 事件驱动，不使用标签轮询。所有请求验证 Tab ID、URL scheme、长度和私网策略。调试 endpoint 始终只在 Rust 后端使用。

### 6.3 Project Preview Server（后续）

项目 HTML 的“运行”入口不属于当前外部 Chromium 最小闭环。实现时不得通过 Studio Gateway Origin 提供，而应使用另一个随机 loopback 端口和不可猜测的 Workspace token：

```text
http://127.0.0.1:<preview-port>/<workspace-token>/<relative-path>
```

服务端必须：

- canonicalize 根目录和目标路径；
- 拒绝符号链接或路径穿越造成的根目录逃逸；
- 设置正确 MIME type 和 `X-Content-Type-Options: nosniff`；
- 禁止目录列表，除非未来明确设计；
- 对不存在、过大、不允许访问和读取失败返回不同错误；
- 不包含 Studio API、偏好设置、App Server WebSocket 或任何管理端点。

### 6.4 Agent Bridge

AI Browser 优先复用标准 Playwright MCP，不自行重新实现完整浏览器协议。推荐采用由 Studio 管理的 Browser Broker：

- Broker 连接 Studio 全局 Managed Browser；Workspace 只限定本次 AI 可见/可操作的标签引用与 Origin；
- Codex/OpenCode 通过 MCP 连接 Broker；
- Studio UI 通过受认证的本地控制通道读取同一 Browser Registry；
- 每个 Tab 使用不可猜测 ID；
- Profile 使用专用目录，不指向用户日常 Chrome Profile；
- Browser Controller 必须串行化启动，任何 Thread/Workspace 都不得创建第二个 Studio Profile 进程。

若当前 Codex App Server 不能动态挂载 Workspace MCP，实验实现可以先使用全局 MCP 配置或项目 `.codex/config.toml`，但必须记录对现有项目文件的影响并展示 MCP 状态。

第二阶段允许 Agent Browser 以独立 headed Chromium 窗口出现。把同一个 Chromium 页面无损嵌入 Tauri 主窗口不是第二阶段验收条件。

## 7. 状态与存储

建议的持久对象：

```text
WorkspaceBrowser
  workspaceId, canonicalRoot, profileKind
  trustedActiveContent, createdAt, updatedAt

BrowserTab
  id, workspaceId, ownerThreadId?
  backend, url, title, status
  lastActivityAt, lastError?

BrowserAttachment
  id, threadId, tabId, kind
  sourceUrl, localArtifactPath?, summary, createdAt
```

结构化状态优先存 SQLite。Profile、截图、日志、HAR 和其他大工件保存在应用数据目录下的 Workspace 子目录。数据库只保存元数据和受控路径。

```text
<app-data>/browser/
├── profiles/<workspace-hash>/
├── artifacts/<workspace-hash>/<tab-id>/
└── logs/<workspace-hash>/<tab-id>/
```

不得把认证 Cookie、LocalStorage 或完整页面内容复制到 SQLite。

## 8. 安全模型

### 8.1 Studio Gateway

加入不可信远程页面前，必须先加固当前 loopback Gateway：

- 启动时生成高熵随机凭据；
- Main WebView 的 HTTP 请求和 WebSocket 握手必须携带凭据；
- 校验 `Origin` 和预期 Host；
- WebSocket 拒绝未知 Origin、无凭据和重复/失效凭据；
- 敏感 POST/DELETE 不接受可被普通跨站表单触发的请求格式；
- 错误信息不泄漏本地绝对路径、令牌或后端环境变量；
- 关闭窗口或后端重启时使旧凭据失效。

### 8.2 Tauri capability

Phase 1 不创建 Browser WebView；远程页面只存在于独立 Chromium 进程，因此天然不匹配任何 Tauri window/webview label。仍不得使用通配符扩大 Main WebView capability。

如果未来重新引入嵌入式 Browser Surface，它必须使用独立 label 和空或最小 capability。Tauri 官方说明远程来源默认不能访问 Tauri API；Linux/Android 还无法可靠区分窗口请求与内嵌 iframe 请求，因此本特性不通过 Main WebView iframe 承载远程页面。参考：[Tauri Capabilities](https://v2.tauri.app/security/capabilities/)。

### 8.3 Profile、凭据与 AI 审批

- 默认使用 Studio 专用 Profile，不直接自动化用户默认 Chrome Profile；
- Persistent 和 Isolated Profile 都必须可选；
- Cookie、密码和 Storage State 不作为模型文本上下文；
- “连接现有 Chrome 标签页”只作为后续显式启用模式；
- 用户必须能查看、断开和清除连接。

Playwright 也建议为自动化创建单独的 User Data Directory。参考：[Playwright persistent context](https://playwright.dev/docs/api/class-browsertype#browser-type-launch-persistent-context)。

| 风险级别 | 默认策略 | 示例 |
| --- | --- | --- |
| Read | 可按会话自动允许 | snapshot、截图、Console、Network 摘要 |
| Navigate | 首次域名确认或允许列表 | 打开 URL、后退、刷新 |
| Interact | 默认逐次确认 | click、type、选择、滚动到目标 |
| Mutate/External | 必须确认 | 提交、上传、下载、剪贴板、支付、发布、删除 |

允许列表不是完整安全边界。重定向、链接和页面 JavaScript 仍可能跨域。自动操作必须记录工具名、目标 Origin、结果和时间。

### 8.4 Prompt injection 与隐私

- 网页内容视为不可信输入；
- 页面文字不能改变 Browser Policy；
- 不自动把完整网页加入 Turn；
- 页面截图、DOM 和日志加入对话前展示来源；
- 默认禁止 Agent Browser 访问回环地址以外的私网管理地址；
- localhost 项目端口可以按 Workspace 显式允许；
- 浏览器日志写文件后由模型按需搜索，避免泄漏无关数据和浪费 token。

## 9. 分阶段交付与验收

### Phase 0：Gateway 与隔离基础

范围：Gateway token、HTTP/WS Origin 校验、Browser capability、URL policy、Browser Controller 最小接口、测试夹具和默认关闭的实验 Feature Flag。

- AC-0.1：知道 Gateway 端口但没有有效凭据的页面不能读取偏好设置、收藏、Map 或连接 App Server WebSocket。
- AC-0.2：非 Main WebView Origin 的 WebSocket 握手被拒绝，并留下不包含凭据的诊断日志。
- AC-0.3：Browser label 不能调用 Main WebView 的 Tauri capability。
- AC-0.4：`file:`、`javascript:`、`data:` 和未知 scheme 导航被拒绝并显示原因。
- AC-0.5：现有 Codex、OpenCode、文档审阅、收藏和 Map 测试全部通过。

Phase 0 实现记录（2026-08-11）：

- Gateway 每次启动生成 122 bit 随机凭据，只通过精确 Main Origin 的 Tauri initialization script 注入；不写入 URL、Cookie、配置或日志。
- HTTP API 使用 `Authorization: Bearer`，App Server WebSocket 使用专用 subprotocol；两者均校验固定 loopback `Host`，WebSocket 额外要求精确 `Origin`。
- `/studio`、`/opencode` 与 `/ws` 受保护；不含本地数据的内嵌静态资源保持公开，以便 Main WebView 首次加载。
- Browser Controller 目前只提供 Feature Flag、状态和导航策略判断，不创建 WebView；默认配置为 `enabled: false`。
- Phase 0 在 URL 语法层拒绝 loopback 之外的私网 IP 和 `.local`；域名解析后的地址复检必须在 Phase 1 实际导航前完成，当前阶段没有可触发导航的 WebView。
- `main` 是唯一匹配现有 Tauri capability 的窗口 label，未来 `browser-*` label 不继承其能力。

### Phase 1：外部 Chromium Browser Workspace

范围：本机工作区全局入口、系统 Chromium 发现/启动、全局 Profile、Studio 重启后重新连接、CDP 事件流、HTTP(S)/localhost 和 Wayland 分屏。

- AC-1.1：用户从 Studio 一个入口启动专用 Chromium，并能在完整浏览器界面正常登录和浏览。
- AC-1.2：Studio 不复制 Chromium 标签栏和地址栏；后续聊天链接和 HTML“运行”统一调用全局 Browser Service。
- AC-1.3：从任意 Thread 打开 Browser 都复用同一进程、Profile 和标签集合；切换 Thread 不启动第二个浏览器，也不重建页面。
- AC-1.4：关闭并重启 Studio 后，如果 Chromium 仍运行，Studio 能通过 `DevToolsActivePort` 重新连接。
- AC-1.5：关闭 Studio 不强制关闭用户仍在使用的 Chromium；浏览器自行退出后 CDP WebSocket 关闭事件使 UI 转为离线。
- AC-1.6：远程页面不能访问 Studio Gateway 或 Tauri API；CDP endpoint 不返回前端。
- AC-1.7：未安装浏览器、启动失败、DevTools 不可用和浏览器退出均有明确 UI。
- AC-1.8：Ubuntu Wayland 完成主验收，并明确提示使用系统分屏；自动窗口定位不是通用验收项。
- AC-1.9：前端不存在 Browser 定时轮询；页面创建、更新、关闭和连接断开均通过 CDP WebSocket → Rust Broadcast → Gateway SSE 推送。

### Phase 2：内嵌 WRY Browser

范围暂定为统一 BrowserController 下的另一种呈现后端。Windows 使用 WebView2，macOS 使用 WKWebView，Linux 使用 WebKitGTK；Wayland 必须在 GTK 原生容器/Runtime 层实现，而不是在应用层重新挂载已有 WebView。详细布局、Profile、弹窗、下载、权限、输入法及跨平台验收标准在 Phase 2 开始前另立规格。

Phase 2A 先只验证 Linux Wayland 下的共享 GTK 原生容器方案，不考虑 X11，也不直接替换生产入口。原型边界、运行方式与验收标准见 [phase-2a-wayland-prototype.md](phase-2a-wayland-prototype.md)。

Phase 2B 在同一原生容器中实现共享 Profile 的多 Tab、窄栏原生缩放、全局入口和 Comment Core 接入。实现约束与验收标准见 [phase-2b-embedded-browser.md](phase-2b-embedded-browser.md)。

### Phase 3：Playwright AI Browser

范围：Playwright MCP 发现/启动/状态、Workspace Profile、Agent Tab、浏览器工具、审批、Origin Policy、工件、Codex 集成和 OpenCode 兼容性验证。

- AC-3.1：Studio 重启后恢复同一个 Persistent Profile；AI 只能访问用户明确关联到当前请求的标签页。
- AC-3.2：AI 能打开本地应用、读取结构化页面快照、点击、输入并截图。
- AC-3.3：Console 错误和失败请求保存为完整工件，聊天只显示摘要和引用。
- AC-3.4：提交、上传、下载和非允许 Origin 的交互执行前要求确认。
- AC-3.5：拒绝审批后 Turn 能继续，并收到结构化拒绝结果。
- AC-3.6：关闭 Studio 或 Browser Broker 后不遗留失控 Chromium/MCP 进程。
- AC-3.7：并行 Thread 共享浏览器进程，但不能在没有明确关联/授权时操作另一个 Thread 正在使用的标签页。
- AC-3.8：不支持的后端能力明确灰化并说明原因。

### Phase 4：共享浏览时间线

范围：Viewer 到 Agent Handoff、可选 Playwright MCP Bridge 扩展、页面/选择/元素/截图/日志加入聊天、localhost 服务发现、视觉比较和 Browser Timeline。

- AC-4.1：Handoff 明确说明只复制 URL，除非用户显式连接已有 Profile。
- AC-4.2：用户可以选择一个可见 Chrome 标签页授权给当前 Workspace，其他标签不可见。
- AC-4.3：选择文本或元素后加入聊天，内容包含来源 URL 且不自动发送。
- AC-4.4：截图、Console 和 Network 工件能在当前 Thread 中打开、收藏和重新引用。
- AC-4.5：AI 操作和人工操作在 Timeline 中可区分、可审计。
- AC-4.6：视觉回归保存修改前后截图及对应 Turn/commit 元数据。

## 10. 配置草案

第一阶段配置保存在 Studio 设置文件，暂不要求全部进入设置 UI：

```json
{
  "browser": {
    "enabled": false,
    "restoreTabs": true,
    "allowHttp": true,
    "allowPrivateNetwork": false,
    "allowLocalhost": true,
    "previewJavaScript": true,
    "externalOpenFallback": true,
    "agent": {
      "enabled": false,
      "provider": "playwright-mcp",
      "profile": "persistent",
      "approval": "interactive",
      "allowedOrigins": []
    }
  }
}
```

`enabled: false` 是 Phase 0 遗留的默认关闭标记；用户在界面点击 Browser/启动 Chromium 被视为本次显式启用，无需手工编辑旧设置。配置加载必须接受缺失字段并应用安全默认值。所有大小、数组长度和 URL 长度都有上限。

## 11. 测试策略

### 11.1 自动测试

- URL/scheme、Origin 和 Host 判定；
- canonical path、符号链接和 Workspace 逃逸；
- Browser state、Tab/Thread/Workspace 关联；
- capability label；
- 审批风险分级；
- 配置默认值、迁移和边界；
- 工件路径与清理。

集成测试使用本地服务器提供普通 HTML/CSS/JS、SPA、弹窗、重定向、下载、Console error、失败请求、CSP 页面、慢响应和尝试访问 Studio Gateway 的恶意页面。测试不依赖公共互联网，外网冒烟测试单独运行。

### 11.2 手工跨平台测试

- Ubuntu Wayland 是当前主开发环境，验证 Brave/Chromium、CDP、分屏和持久 Profile；
- Windows 验证 Chrome/Edge/Brave 发现、Profile 路径和 WSL localhost；
- macOS 验证 Chrome/Chromium 发现、Profile 路径和系统分屏；
- 每个平台验证深浅主题、200% 缩放、宽屏、窄屏和键盘操作。

## 12. 可观测性、发布与回退

Browser 日志记录 Workspace 不可逆短哈希、backend、tab ID、Origin、事件、耗时、PID 和退出原因，不记录 Cookie、请求正文、凭据或输入框敏感值。

UI 区分加载中、空页面、离线、策略拒绝、页面错误、后端未安装、启动失败、需要审批和后端崩溃。

- 实验阶段由 Feature Flag 控制，默认关闭；
- 数据表和配置使用版本号，迁移可重复执行；
- 关闭特性不删除 Profile 和工件；
- 用户可单独清除 Browser 数据；
- Phase 0、1、2、3 分别提交，避免把安全、UI、WRY 和 Playwright 依赖混成大提交；
- 未达到某阶段验收标准时，不在 README 中宣称该阶段可用。

## 13. 实现前技术验证

以下问题通过小型原型解决，结果回写本文档：

1. 已完成：Linux Wayland 上应用层 child WebView/GtkFixed 方案存在层级、位置、菜单、缩放和空白页问题，停止继续修补，实验代码只保留在独立 stash。
2. 实现中：Managed Chromium Profile/Data Directory 在三个平台上的一致性、重连和清除方式。
3. Codex App Server 能否按 Thread/Workspace 动态应用 MCP，或是否需要固定的 Broker Streamable HTTP endpoint。
4. OpenCode 对同一 MCP endpoint、Workspace roots 和审批结果的兼容程度。
5. Playwright MCP 多客户端能否安全共享 Context；否则 Broker 如何串行化或拆分。
6. Windows 客户端连接 WSL 后端时，localhost 和 Preview Server 应运行在哪一侧。
7. 已完成：Gateway token 不使用 URL 或 Cookie；HTTP 使用 Header，WebSocket 使用 subprotocol。

这些验证完成前，不把对应阶段标记为实现中。
