# Phase 2A：Wayland 内嵌 WebView 原型

状态：实验实现，可独立运行；不替换 Codex Thread Studio 主程序。

## 1. 要验证的问题

Phase 2A 只回答一个架构问题：Ubuntu Wayland 下，能否在一个 GTK 顶层窗口内稳定承载 Studio WebView 与 Browser WebView，并由同一个原生容器完成布局。

本阶段不考虑 X11，也不承诺完整浏览器产品能力。原型验证通过后，才把这种呈现后端接入 Phase 1 已有的 `BrowserController`。

## 2. 原型边界

已实现：

- 强制使用 Wayland；`WAYLAND_DISPLAY` 缺失时直接拒绝启动；
- 单个 Tao/GTK 顶层窗口；
- 一个共享 `GtkPaned` 原生分栏，承载 Studio Shell 与 Browser Column；
- Browser Column 默认隐藏，通过 Studio 全局“浏览器”入口显示或关闭；
- 原生分隔线同时保留 Studio 520 px 与 Browser 380 px 的最小宽度；
- Browser WebView 只占用工具栏下方的右侧内容矩形；
- 右侧宽度可拖拽调整，并同时更新 GTK 原生视图和 Shell CSS 布局；
- HTTP/HTTPS 地址输入、后退、前进、刷新；
- Browser WebView 使用独立持久化 Profile；
- 页面地址、标题和加载状态通过事件回传 Shell；
- 网页文本选择通过受限 IPC 暂存，用户点击“批注选区”后进入批注抽屉；
- `target=_blank` 请求在当前原型 Browser WebView 中安全打开。

明确未实现：

- 多 Tab 生命周期与崩溃恢复；
- 下载、上传、权限提示、密码管理、开发者工具；
- 将原型批注正式写入现有 Comment Core；当前抽屉只保存一份演示草稿，再次批注会替换其内存状态；
- 与生产版会话、收藏、Web Context 数据模型的集成；
- Windows、macOS 或 X11 支持；
- Browser WebView 自动化与网页正文提取。

## 3. 为什么采用 GTK 原生分栏

Wayland 没有可由应用任意定位的 X11 子窗口模型。WebView 必须从创建开始就是同一个 GTK widget tree 的孩子，不能先创建后重挂载，也不能把远端网页塞进 iframe。

原型的层级固定为：

```text
Tao Window
└── GTK VBox
    └── GTK Paned（原生可拖拽分栏）
        ├── Studio Host
        │   └── Studio Shell WebView
        └── Browser Column
            ├── Toolbar Host（58 px）
            │   └── Browser Toolbar WebView
            └── Browser Host
                └── Browser WebView
```

窗口缩放、分栏拖拽和各 WebView 的 viewport 分配全部由 GTK 完成。三个 WebView 互不重叠，因此远端网页不会遮挡工具栏或抢走分隔线事件，也不会出现 `GtkFixed` 子控件尺寸请求反向撑大顶层窗口的反馈循环。

## 4. 安全边界

远端网页中的脚本不可信。Browser IPC 只接受一个结构：

```json
{
  "text": "用户选择的文本",
  "url": "https://example.com/page",
  "title": "Page title"
}
```

Rust 侧进行以下限制：

- URL 仅允许 `http` 和 `https`；
- 选区最多 16 KiB；
- URL 和标题分别限制长度；
- 不接受文件访问、Shell 命令、Studio 命令或任意方法名；
- Browser WebView 不能直接调用 Comment Core，只能产生无副作用的选区数据。

## 5. 运行方法

在原生 Wayland 会话的终端中执行：

```bash
cd /tmp/codex-thread-browser-workspace
cargo run --manifest-path src-tauri/Cargo.toml --bin wayland-webview-prototype
```

持久化浏览器数据默认保存到：

```text
$XDG_DATA_HOME/codex-thread-studio/wayland-webview-profile
```

如果没有设置 `XDG_DATA_HOME`，则使用：

```text
~/.local/share/codex-thread-studio/wayland-webview-profile
```

## 6. 手工验收 Checklist

- [ ] AC-2A-01：确认日志和窗口均在 Wayland 下启动；移除 `WAYLAND_DISPLAY` 后原型拒绝启动。
- [ ] AC-2A-02：右侧网页只覆盖浏览器内容区，不遮挡地址栏、按钮和 Studio 菜单。
- [ ] AC-2A-03：连续拖动分隔线 20 次，网页跟随调整宽度，不出现底部横向滚动条、空白页或突然上移。
- [ ] AC-2A-04：最大化、恢复和改变窗口尺寸后，两块 WebView 仍无错位和间隙。
- [ ] AC-2A-05：地址输入可完整输入 URL，不会自动删除字符；只允许 HTTP/HTTPS。
- [ ] AC-2A-06：前进、后退、刷新可用；链接弹窗不会创建额外顶层窗口。
- [ ] AC-2A-07：关闭再启动原型后，Browser WebView 复用相同 Profile。
- [ ] AC-2A-08：网页选中文本后“批注选区”变为可用；点击后显示选区、URL、标题，并可填写意见。
- [ ] AC-2A-09：中文输入在地址栏和批注输入框中均正常，连续输入不会重复提交组合文本。
- [ ] AC-2A-10：Shell 菜单/批注浮层不与 Browser WebView 交叠；需要跨越 Browser 区域的浮层必须由原生层或专用覆盖层实现。

## 7. Phase 2B 强制设计约束

### 7.1 批注必须是集合，不是单一 UI 状态

原型中的 `currentSelection` 只用于验证网页选区能够跨 WebView 到达 Studio。生产实现不得把 textarea 或当前选区当成持久数据源，而应复用 Comment Core：

1. 每次“添加批注”创建独立 draft ID；
2. draft 保存来源类型、URL、标题、选区和用户意见；
3. 连续添加只追加，不覆盖已有 draft；
4. 批注面板显示当前会话的 draft 列表，可编辑、删除和提交；
5. Browser provider 只负责生成标准化 source context，不感知 Comment Core 的存储和 UI。

### 7.2 顶部基线必须统一

当前 Toolbar WebView 的地址框在 58 px 工具栏内垂直居中，并带有 9 px 内边距；整个应用内容还位于 Wayland compositor 管理的系统标题栏之下。因此地址框本身不会贴住物理窗口最顶端。

生产布局必须区分：

- 系统标题栏：由 Wayland/compositor 管理；
- 应用顶部栏：Studio Header 与 Browser Toolbar 共用同一 GTK row、高度和上边界；
- 控件内边距：只负责视觉居中，不得造成左右区域顶部错位。

验收要求：Browser Toolbar 的背景顶边、底部分隔线必须与 Studio Header 严格对齐；地址框可在工具栏内部留白，但 Toolbar WebView 自身不能出现额外空白行或纵向偏移。若未来采用无系统装饰窗口，则必须同时实现窗口拖动、最大化和系统窗口按钮，不能只为了让地址栏贴顶而关闭 Wayland 装饰。

### 7.3 分栏不能吞没任一工作区

原生分隔线必须同时设置两侧最小宽度。当前基线为 Studio 520 px、Browser 380 px；窗口不足以容纳两侧最小宽度时，不允许继续拖动，而不是覆盖或隐藏 Studio。生产版可以根据显示缩放和窗口宽度调整阈值，但必须满足：

- Studio 的会话导航、内容区和输入框始终可操作；
- Browser 地址栏和网页 viewport 始终保留可用宽度；
- 展开 Browser 时恢复上次合法宽度，越界的旧值要重新 clamp；
- 双击分隔线或执行“重置布局”可恢复推荐比例；
- 分栏宽度持久化的是逻辑像素或比例，不直接保存 Wayland 物理像素。

## 8. 进入 Phase 2B 的条件

AC-2A-01 至 AC-2A-09 全部通过，且 AC-2A-03、AC-2A-04 在一次完整人工测试中没有偶发错位，才进入 Phase 2B。Phase 2B 将处理多 Tab、生命周期、生产 `BrowserController` 适配和 Comment Core 正式接入。
