# Browser Workspace 特性规格

状态：Phase 0、Phase 2A 已完成；Phase 2B 已实现，处于合并候选验证；Phase 1 外部浏览器方案已废弃

适用产品：Codex Thread Studio

当前平台：Ubuntu Linux + Wayland

当前渲染引擎：WRY + WebKitGTK

最后更新：2026-08-12

## 1. 当前产品决策

Browser Workspace 是 Studio 级全局能力，不属于某个会话。它默认可用但按需创建：Studio
启动时只创建主界面；用户第一次打开 Browser 后，才创建 Toolbar、共享 WebContext 和首个
页面 WebView。

- 产品默认使用内嵌 Browser Workspace，不再启动或连接外部 Chrome、Brave 或 Chromium。
- 所有会话共享同一组浏览器 Tab、Cookie、登录状态、缓存和持久 Profile。
- `隐藏`只收起右侧区域并保留页面；`退出浏览器`销毁 Toolbar 和全部页面 WebView，释放
  renderer 进程，但保留持久 Profile；再次打开时创建新的空白 Tab。
- 应用重启后不恢复 Tab。`restoreTabs` 当前固定默认为 `false`，属于后续能力。
- Studio 主 WebView、Browser Toolbar WebView 和远端页面 WebView 互相隔离。远端页面不能
  获得 Studio Gateway 凭据，也不能调用 Studio 内部 action。
- 外部浏览器/CDP/Wayland 分屏方案只保留为历史设计，不在运行时代码中提供 fallback。

## 2. 能力边界

### 2.1 已实现

- 单窗口原生 GTK 分栏，右侧 Browser 可显示、隐藏、调整宽度和彻底退出；
- 共享 Profile 的多 Tab、前进、后退、刷新、地址输入、弹窗转新 Tab；
- 响应式页面保持 100%，固定宽度页面可自动适应，并支持 50%–200% 手动缩放；
- 网页选区进入统一 Comment Draft，同一页面可以连续添加多个批注；
- 下载到系统 Downloads 目录，处理重名和不安全文件名，并提供下载记录和打开目录；
- 拒绝摄像头、麦克风、屏幕、位置、通知和 pointer lock 权限；
- 地址栏、页面链接、重定向和 `window.open` 共用同一 URL 策略；
- TLS 证书失败时显示 Studio 自有错误页，不提供绕过证书的入口；
- 页面 renderer 首次异常退出自动重建；短时间内再次退出时停止自动重试，允许手动刷新；
- 开发模式可截取 Studio 自身窗口，并可故障注入终止活动页面 renderer。

### 2.2 尚未实现

- Windows WebView2 和 macOS WKWebView 容器；X11 不在当前范围；
- 重启后的 Tab 恢复、历史记录 UI、收藏夹或浏览器扩展；
- 标签溢出菜单、完整键盘无障碍和深色网页工具栏主题；
- HTML 项目 Preview Server、文件上传策略和下载前确认策略；
- Playwright MCP、AI 页面操作、截图/DOM/Console/Network 加入会话；
- 域名解析后的私网地址复检。当前策略能拒绝字面量私网 IP 和 `.local`，但不能防止
  公网域名解析到私网地址或 DNS rebinding，因此不能把它当作完整 SSRF 边界。

## 3. 与文档审阅器的关系

两者不会相互替代：

| 能力 | 文档审阅器 | Browser Workspace |
| --- | --- | --- |
| Markdown、文本、图片、静态 HTML 阅读 | 是 | 可通过 URL 浏览，但不是主要入口 |
| 执行远端网页 JavaScript | 否 | 是 |
| 文档搜索与稳定行号批注 | 是 | 否 |
| 网页导航、登录、Cookie、下载 | 否 | 是 |
| 安全边界 | DOMPurify 后的受控内容 | 独立远端 WebView 与最小权限 |

项目 HTML 的“安全预览”已经属于文档审阅器；“作为应用运行”仍需未来的独立 Preview
Server，不能直接使用 Studio Gateway Origin，也不能使用不受控 `file:` URL。

## 4. 原生架构

```text
Tauri / GTK top-level window
└─ GtkPaned
   ├─ Studio host
   │  └─ Main WebView
   └─ Browser column（按需创建）
      ├─ Toolbar host
      │  └─ trusted local Toolbar WebView
      └─ GtkStack
         ├─ remote page WebView / Tab 1
         ├─ remote page WebView / Tab 2
         └─ ...

Persistent WebContext
├─ browser profile: cookies / local storage / IndexedDB
└─ browser cache
```

`GtkPaned` 是横向尺寸的唯一所有者。Toolbar 和页面位于不同 GTK host 中，页面无法覆盖
菜单、Tab 或分隔线。所有页面 WebView 共用一个持久 WebContext；Toolbar 使用独立的
trusted WebContext。布局、加载、标题、下载、崩溃和 Tab 更新均由事件驱动，不轮询页面。

### 4.1 生命周期与资源

| 状态 | 主 WebView | Toolbar | 页面 WebView | Profile |
| --- | --- | --- | --- | --- |
| Studio 刚启动 | 已创建 | 未创建 | 未创建 | 未打开 |
| Browser 显示/隐藏 | 已创建 | 保留 | 保留 | 保留 |
| 退出 Browser | 已创建 | 销毁 | 全部销毁 | 文件保留 |
| Studio 退出 | 销毁 | 销毁 | 销毁 | 文件保留 |

退出 Browser 后保留 WebContext 对象到 Studio 生命周期结束，是为了避免 WebKitGTK 反复
建立 NetworkProcess；页面 renderer 仍会被释放。持久 Profile 与缓存位于：

```text
$XDG_DATA_HOME/codex-thread-studio/<profile-name>
# fallback: ~/.local/share/codex-thread-studio/<profile-name>

$XDG_CACHE_HOME/codex-thread-studio/WebKitCache
# fallback: ~/.cache/codex-thread-studio/WebKitCache
```

下载目标为系统识别的 Downloads 目录。浏览器信息对话框展示实际路径；清除历史/缓存属于
显式用户操作，不随隐藏或退出自动删除。

## 5. 导航、权限与错误策略

### 5.1 统一导航策略

下列入口必须调用同一个 `validate_browser_url`：

- 地址栏；
- Studio 打开 URL 的全局 action；
- 页面中的普通链接和脚本导航；
- HTTP 重定向；
- `window.open` / 新窗口请求。

当前只允许 `http`、`https`，拒绝 URL 中的账号密码、`file:`、`data:`、`javascript:`、
`tauri:` 和未知 scheme。默认允许 localhost，拒绝其他字面量私网 IP 与 `.local`。策略拒绝
必须保留原页面并在 Studio 显示原因。

### 5.2 TLS 与网络错误

证书校验由 WebKitGTK 完成。TLS 失败后：

1. 终止失败导航；
2. 在该 Tab 中显示经过 HTML 转义的本地错误页；
3. Toolbar 保留失败 URL，Studio 显示简短通知；
4. 不提供“仍然继续”或关闭证书校验的设置。

普通 DNS、连接或 HTTP 错误继续使用 WebKitGTK 的标准错误行为；后续可统一视觉，但不能
吞掉真实错误。

### 5.3 权限与弹窗

- 摄像头、麦克风、屏幕采集、设备枚举、地理位置、通知和 pointer lock 一律拒绝；
- `target=_blank` 和 `window.open` 创建 Studio Browser Tab，不创建额外顶层窗口；
- 未识别的外部协议不交给 Shell 自动执行；
- 远端页面不能调用 Toolbar 的 `studio-action://`，Toolbar action 只来自本地 WebView。

## 6. Comment Core

Browser Provider 只负责产生来源数据：URL、标题、选中文本。Comment Core 负责 draft ID、
意见、追加、编辑、删除与提交。Browser 不保存批注，也不感知 Chat、Document 或未来 EPUB
的批注实现；删除 Browser Provider 不应修改 Comment Core。

普通浏览、滚动和地址输入不会自动加入会话。Phase 3 之前，网页批注只进入现有 Draft，
不会触发额外模型请求。

## 7. 配置

配置保存在 Studio 设置文件，当前不要求提供 UI：

```json
{
  "browser": {
    "enabled": true,
    "restoreTabs": false,
    "allowHttp": true,
    "allowPrivateNetwork": false,
    "allowLocalhost": true,
    "previewJavaScript": true,
    "embeddedWidth": 720,
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

- `embeddedWidth`：480–2400 逻辑像素；
- `restoreTabs`：字段为前向兼容保留，当前实现不恢复 Tab；
- `previewJavaScript`：为未来 Project Preview Server 保留，当前远端 Browser 不读取该值；
- `agent.*`：为 Phase 3 保留，当前不会启动 Playwright；
- 缺失字段使用安全默认值；无效值拒绝加载，不能静默放宽权限。

## 8. 分阶段状态

### Phase 0：安全与隔离基础——已完成

- Gateway 使用启动期随机凭据，并校验 HTTP/WS Host、Origin 与认证；
- 只有 Main WebView 获得现有 Tauri capability；
- URL policy 及配置边界有自动测试；
- 现有 Codex、OpenCode、文档、收藏与 Map 能力保持回归测试。

### Phase 1：外部 Chromium Workspace——已废弃

曾实现外部 Chromium/Brave 进程管理、CDP 事件和 Wayland 分屏。产品最终选择单窗口内嵌
体验，因此该代码与 fallback 已删除。Phase 1 的旧验收项不再是发布条件，也不会在主线中
保留第二套浏览器生命周期。

### Phase 2A：Wayland 原生容器原型——已完成

验证了同一 GTK 顶层窗口、原生 Paned、独立 Toolbar/Page WebView 和稳定尺寸分配。历史
原型说明见 [phase-2a-wayland-prototype.md](phase-2a-wayland-prototype.md)。

### Phase 2B：内嵌 Browser Workspace——合并候选

产品实现和逐项验收状态见
[phase-2b-embedded-browser.md](phase-2b-embedded-browser.md)。当前合并目标仅为 Linux Wayland。

### Phase 3：AI Browser——未开始

计划通过 Playwright MCP/Broker 提供 DOM、无障碍快照、截图、Console、Network、受控点击和
输入。AI 只能访问用户明确关联的 Tab；提交、上传、下载和跨 Origin 操作需要审批。不能把
Cookie、Storage 或完整网页默认加入模型上下文。

### Phase 4：共享浏览时间线——未开始

计划支持页面/选区/元素/截图/日志加入会话，记录人工和 AI 操作，并将浏览工件与 Thread、
Turn 和代码提交关联。

## 9. 测试与合并门槛

自动测试：

- URL/scheme/私网字面量、配置默认值与边界；
- Gateway HTTP/WS 认证和 Origin；
- Toolbar action 解析、Tab、缩放、下载命名、TLS 错误页转义；
- Browser 代码不重新引入外部 Chrome/CDP 依赖；
- Rust tests、Clippy、前端测试和 release check。

Wayland 人工测试：

- 首次打开懒加载、隐藏、再次显示、退出和重新创建；
- 多 Tab、弹窗、关闭最后 Tab、地址栏、导航按钮和中文输入；
- 拖动分栏、固定宽页面自适应、响应式页面、手动缩放；
- 下载成功/重名/打开目录；敏感权限被拒绝；
- renderer 首次崩溃自动恢复，短时间重复崩溃停止重试；
- Browser 从未打开时不产生额外页面 renderer，退出后页面 renderer 被释放。

已知不能仅靠自动测试证明的项目必须在 Phase 2B 文档中保持未勾选，不能因代码存在就写成
人工验收通过。

## 10. 后续设计约束

- 跨平台时复用 Browser domain model、Toolbar 协议、导航策略和 Comment Provider；只替换
  原生容器与 WebView backend。
- 不为 macOS/Windows 回退到外部浏览器，也不在 Linux 回退到 iframe 或应用层 GtkFixed。
- Project Preview Server 必须使用独立随机 loopback Origin，canonicalize 路径并阻止根目录
  逃逸；不得复用 Studio Gateway Origin。
- Phase 3 的 Agent Browser 必须复用现有 Tab registry，不能为每个 Thread 启动独立浏览器。
- 所有页面事件保持事件驱动；禁止用定时轮询补偿生命周期设计问题。
