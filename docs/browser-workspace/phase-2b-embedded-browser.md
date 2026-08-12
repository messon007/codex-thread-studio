# Phase 2B：内嵌 Browser Workspace

状态：功能实现完成，处于合并候选验证

平台：Ubuntu Wayland

渲染引擎：WRY + WebKitGTK

最后更新：2026-08-12

## 1. 目标

在 Codex Thread Studio 的同一个 GTK 顶层窗口中提供全局 Browser Workspace。Browser
默认隐藏且懒加载，展开后与 Studio 左右分栏；所有聊天会话共享浏览器标签、Profile、Cookie
和登录状态。

本阶段不把 WebKitGTK 描述成完整 Chromium。标签、地址栏、缩放、下载、恢复和 Comment
Core 接入由 Studio 管理。当前交付只承诺 Ubuntu Wayland，不考虑 X11、Windows 或 macOS。

## 2. 原生结构与生命周期

```text
GtkPaned
├─ Studio WebView
└─ Browser Workspace（首次打开时创建）
   ├─ trusted Toolbar WebView
   └─ GtkStack
      ├─ remote Tab WebView
      ├─ remote Tab WebView
      └─ ...
```

- `GtkPaned` 是唯一横向尺寸所有者；网页不能覆盖 Studio、工具栏或分隔线。
- `GtkStack` 同时只显示一个 Tab，隐藏 Tab 保留页面状态。
- 所有页面 Tab 共用一个持久 `WebContext`；Toolbar 使用独立 trusted context。
- Browser 隐藏时保留 Tab；`退出浏览器`销毁 Toolbar 和全部 Tab WebView。
- 退出后保留 Profile 和 WebContext，重开时创建空白 Tab，不恢复旧 Tab。
- Browser 不属于当前 Thread；切换 Thread 不关闭或切换 Browser Tab。
- 启动、布局、加载、导航、下载和异常恢复均由事件驱动，不轮询。

## 3. 标签与导航

每个 Tab 在进程内保存 ID、URL、标题、WebView/GTK host、zoom、适应宽度状态和崩溃状态。

- `+`、`Ctrl+T` 创建 Tab；
- 点击标签或 `Ctrl+Tab` 切换；
- 关闭按钮或 `Ctrl+W` 关闭；关闭最后一个 Tab 时复用该 Tab 打开默认页；
- `window.open` 创建新 Tab，不覆盖来源 Tab；
- 地址栏、普通链接、脚本导航、重定向和新窗口统一执行 URL policy；
- URL policy 只允许 HTTP(S)，按配置处理 localhost/私网，并拒绝账号密码和危险 scheme；
- TLS 失败显示本地安全错误页，不允许绕过证书检查。

当前私网策略只分析 URL 中的主机字面量，不解析 DNS。域名解析复检和 DNS rebinding 防护是
明确的后续安全项。

## 4. 页面宽度策略

窄栏中的桌面网页可能声明约 1000px 最小内容宽度。适应宽度在页面加载结束或分栏拖拽停止
后读取 `innerWidth` 与 `scrollWidth`，通过 WebKit 原生页面 zoom 计算：

```text
zoom = clamp(innerWidth / scrollWidth, 0.5, 1.0)
```

不向网页注入覆盖布局的 CSS，也不伪装移动 User-Agent。用户手动调整 50%–200% 后，该 Tab
退出自动适应；再次点击适应按钮才恢复。推荐宽度 720，Browser 最小宽度 480，Studio 最小
宽度 520。

## 5. 下载、权限和异常恢复

- 下载保存到系统 Downloads 目录；文件名会解码、清理路径字符并为重名添加序号；
- Browser 信息对话框显示 Profile、Cache、Downloads 路径及清除操作；
- 摄像头、麦克风、屏幕、设备信息、位置、通知和 pointer lock 请求一律拒绝；
- 页面 renderer 首次异常退出时重建同一 Tab 并恢复 URL/zoom；
- 同一 Tab 在 30 秒内再次异常退出时停止自动恢复，标题显示异常并等待手动刷新；
- Toolbar renderer 异常时只自动重载一次；再次异常则退出 Browser，避免无限循环；
- 开发命令 `--dev-crash-browser-tab` 用于验证恢复路径，不暴露在 release UI。

## 6. Comment Core

Browser Provider 只产生 URL、标题和选中文本。Comment Core 负责 draft ID、意见、追加、编辑、
删除和提交；连续批注不能覆盖已有 draft。Browser Tab 不保存批注数据，也不感知 Chat、
Document 或未来 EPUB 批注实现。

## 7. 验收状态

下列勾选表示已经实现并完成对应验证；未勾选项即使已有代码，也仍需合并前人工验收。

- [x] AC-2B-01：Browser 默认隐藏且懒加载；展开、隐藏和拖拽不改变顶层窗口尺寸或遮挡 Studio。
- [x] AC-2B-01a：页面内容紧贴 Toolbar 下边界，不出现陈旧 viewport 导致的顶部空白。
- [ ] AC-2B-02：同一轮人工验收中打开至少三个 Tab，任意切换且页面状态不丢失。
- [x] AC-2B-03：关闭当前、后台和最后一个 Tab 均不崩溃。
- [x] AC-2B-04：`window.open` 进入新 Tab，不覆盖来源 Tab。
- [ ] AC-2B-05：使用真实登录站点验证 Tab 间共享登录状态；自动隔离测试已确认远端页面不能触发 Studio action。
- [x] AC-2B-06：拖拽停止后适应模式重新计算一次，代码和前端均无 Browser 定时轮询。
- [x] AC-2B-07：固定宽度页面在窄 Browser 栏中可完整查看，Toolbar 显示实际缩放值。
- [x] AC-2B-08：响应式页面保持 100%，用户可按 Tab 手动调整 50%–200%。
- [ ] AC-2B-09：在同一轮人工验收中覆盖 Tab、地址栏、网页和批注输入的中文输入法。
- [x] AC-2B-10：同一页面可连续产生多个独立批注 draft。
- [x] AC-2B-11：菜单、Tab 和 Toolbar 永远位于网页上方，不被远端内容遮挡。
- [ ] AC-2B-12：最终提交上的 Rust test、Clippy、前端 test、release check 和 Wayland 人工验收全部通过。
- [x] AC-2B-13：Browser 从未打开时不创建 Toolbar/Page WebView；退出后页面 renderer 被释放。
- [x] AC-2B-14：下载成功，下载记录与打开目录可用，重名不覆盖现有文件。
- [x] AC-2B-15：页面 renderer 故障注入可触发一次自动恢复和重复崩溃熔断。
- [x] AC-2B-16：链接、重定向、弹窗和地址栏共用导航策略；TLS 失败无绕过入口。

## 8. 合并前手工检查

1. 冷启动 Studio，确认会话列表默认可见，且未出现额外页面 renderer。
2. 打开 Browser，创建三个 Tab；分别浏览响应式页面、固定宽页面和会产生弹窗的本地夹具。
3. 切换、关闭后台 Tab、关闭当前 Tab、关闭最后 Tab；验证 `+`、`Ctrl+T/W/Tab`。
4. 在地址栏、网页输入框和批注框分别使用中文输入法。
5. 下载同名文件两次，检查文件名、记录和打开目录。
6. 隐藏后重新显示，确认 Tab 保留；退出后重新打开，确认创建空白 Tab。
7. 运行故障注入两次，确认第一次恢复、第二次熔断；手动刷新后恢复。
8. 关闭 Browser 后检查 renderer 释放；关闭 Studio 后确认无遗留应用进程。

## 9. 后续项

- Tab/活动 Tab 的安全持久化与重启恢复；
- 标签溢出菜单、深色主题和完整键盘无障碍；
- 文件上传与下载前确认策略；
- DNS 解析后的私网复检和更完整的网络错误页面；
- Windows WebView2、macOS WKWebView 等价容器；
- Phase 3 Playwright AI Browser 的 Tab 引用、审批和工件协议。
