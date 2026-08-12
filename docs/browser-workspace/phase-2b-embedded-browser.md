# Phase 2B：内嵌 Browser Workspace

状态：开发中

平台：Ubuntu Wayland
渲染引擎：WRY + WebKitGTK

## 1. 目标

在 Codex Thread Studio 的同一个 GTK 顶层窗口中提供全局 Browser Workspace。Browser 默认隐藏，展开后与 Studio 左右分栏；所有聊天会话共享浏览器标签、Profile、Cookie 和登录状态。

本阶段不把 WebKitGTK 描述成完整的 Chromium。WebKitGTK只提供页面视图；标签、地址栏、缩放、恢复和 Comment Core 接入由 Studio 管理。

## 2. 原生结构

```text
GtkPaned
├─ Studio WebView
└─ Browser Workspace
   ├─ Toolbar WebView
   │  ├─ Tab strip
   │  └─ Navigation / address / zoom / comment
   └─ GtkStack
      ├─ Browser Tab WebView
      ├─ Browser Tab WebView
      └─ Browser Tab WebView
```

- `GtkPaned` 是唯一横向尺寸所有者；网页不能覆盖 Studio、工具栏或分隔线。
- `GtkStack` 同时只显示一个 Tab WebView，隐藏 Tab 保留页面状态。
- Browser 隐藏时不得读取 viewport 或修改 zoom；必须在展开、完成 GTK allocation 后再适配，避免 WebKit backing surface 保留隐藏时的陈旧尺寸。
- Tab WebView 共用一个持久 `WebContext`，不得为每个 Tab 创建独立 Profile。
- Browser 不属于当前 Thread；切换 Thread 不关闭或切换 Browser Tab。
- 远端网页不能调用 Studio 的内部 action。快捷键在 GTK widget 层捕获，Toolbar action 只来自本地 Toolbar WebView。

## 3. 标签模型

每个 Tab 至少保存：

- 进程内唯一 ID；
- 当前 URL 与页面标题；
- WRY WebView 和对应 GTK host；
- 当前原生 zoom；
- 是否启用“适应宽度”。

行为：

- `+`、`Ctrl+T` 创建 Tab；
- 点击标签或 `Ctrl+Tab` 切换；
- 关闭按钮或 `Ctrl+W` 关闭；关闭最后一个 Tab 时复用该 Tab 打开默认页；
- 网页 `window.open` 创建新 Tab，不覆盖当前页面；
- 创建、关闭和切换均为事件驱动，不轮询页面状态。

## 4. 页面宽度策略

窄栏中的桌面网页可能声明约 1000px 的最小内容宽度。出现横向滚动不一定表示 WebView 尺寸错误，因此必须区分：

1. GTK 分配给 Browser 的实际宽度；
2. `window.innerWidth`；
3. 文档 `scrollWidth`。

“适应宽度”在页面加载结束或分栏拖拽停止后读取后两项，并通过 WebKit 原生页面 zoom 计算：

```text
zoom = clamp(innerWidth / scrollWidth, 0.5, 1.0)
```

不向网页注入覆盖布局的 CSS，也不默认伪装移动端 User-Agent。用户手动放大、缩小或恢复 100% 后，该 Tab 退出自动适应；再次点击适应按钮才恢复。

Browser 新安装推荐宽度为 720 逻辑像素，最小宽度为 480；Studio 最小宽度为 520。分栏只保存逻辑宽度，不保存截图中的物理像素。

## 5. Comment Core

Browser provider 只产生统一来源：页面 URL、标题和选中文本。Comment Core 负责 draft ID、意见、追加、编辑、删除和提交；连续批注不得覆盖已有 draft。Browser Tab 不保存批注数据，也不感知聊天或文档批注的实现。

## 6. 验收标准

- [ ] AC-2B-01：Browser 默认隐藏；展开、隐藏和拖拽不会改变顶层窗口尺寸或遮挡 Studio。
- [ ] AC-2B-01a：隐藏状态完成页面加载后再展开，网页内容仍紧贴 Toolbar 下边界，不出现陈旧 viewport 造成的顶部空白。
- [ ] AC-2B-02：至少打开三个 Tab 后可以任意切换，页面状态不丢失。
- [ ] AC-2B-03：关闭当前、后台和最后一个 Tab 均不崩溃。
- [ ] AC-2B-04：`window.open` 进入新 Tab，不覆盖来源 Tab。
- [ ] AC-2B-05：所有 Tab 共享登录状态，但远端网页不能触发 Studio 内部 action。
- [ ] AC-2B-06：拖拽停止后，适应模式重新计算一次；不使用持续轮询。
- [ ] AC-2B-07：约 1000px 固定宽度页面在 580–720px Browser 栏中可完整查看，Toolbar 显示实际缩放值。
- [ ] AC-2B-08：响应式页面保持 100%，用户可按 Tab 手动调整 50%–200%。
- [ ] AC-2B-09：Tab、地址栏、网页和批注输入均支持中文输入法。
- [ ] AC-2B-10：同一页面可连续产生多个独立批注 draft。
- [ ] AC-2B-11：菜单、Tab 和 Toolbar 永远位于网页上方，不被远端内容遮挡。
- [ ] AC-2B-12：Rust 测试、Clippy 和前端测试通过，并完成 Wayland 人工验收。

## 7. 后续项

- Tab URL 与活动 Tab 的安全持久化及崩溃恢复；
- 下载、权限请求、证书错误和弹窗策略；
- 深浅主题、标签溢出菜单和键盘可访问性；
- Windows WebView2、macOS WKWebView 的等价容器实现；
- 与 Phase 3 Agent Browser 的 Tab 引用和审批协议。
