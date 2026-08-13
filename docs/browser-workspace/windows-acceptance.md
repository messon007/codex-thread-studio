# Windows Embedded Browser 验收记录

状态：传入 bundle 的 Windows x64 Debug/Release 构建、自动测试和自动化 WebView2 运行时
验收已通过，MSI 与 NSIS 安装包均已生成。代码审查后已删除 bundle 携带的整份 WRY fork，
改回 crates.io 正式版 WRY；因此这些结果是移植依据，不代表当前合并候选已重新完成 Windows
原生验收。其余真实输入设备、真实站点帐户和正式 WRY 路径仍待 Windows 原生复验。

最后更新：2026-08-13

## 架构范围

- Windows 11 使用系统 WebView2，经 WRY 创建同一 Tauri 顶层窗口内的 child WebView；
- Studio、可信 Toolbar、分隔条和远端 Tab 是不同 WebView/HWND；远端 Tab 不接收 Studio
  Gateway 初始化脚本或 Tauri capability；
- Browser 是全局工作区，独立于 Session；隐藏保留运行时，退出浏览器释放 child WebView，
  但不删除 WebView2 Profile；
- 所有页面 Tab 复用同一个 Browser `WebContext` 和 persistent Profile 目录；可信 Toolbar
  使用独立 Context。项目不修改 WRY 内部 Environment 创建逻辑；
- 运行时按需创建，冷启动只创建 Studio 主 WebView；布局由 resize 与 pointer 事件驱动。

## 传入 bundle 的自动验收记录

下表来自 Windows 分支作者在原生 Windows x64 环境的记录。当前集成保留产品实现，排除了
未获上游接受的 WRY 私有补丁；合入前后的 Linux 验证不能替代 Windows 原生复验。

| 项目 | 状态 | 备注 |
| --- | --- | --- |
| Node 语法、UI 单元测试 | 通过 | `node --check ui/app.js`、`npm test`：136/136。 |
| Rust fmt/test/clippy/build | 通过 | `cargo fmt --all -- --check`、`cargo test --workspace --locked`：45/45、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build --workspace --locked`。 |
| Windows 依赖图无 GTK/WebKitGTK | 通过 | `cargo tree --target x86_64-pc-windows-msvc`，未出现 GTK/WebKitGTK。 |
| URL、Toolbar、Tab、Hide/Exit、下载纯逻辑 | 通过 | Rust/Node 测试覆盖 URL policy、Toolbar action、Tab lifecycle、Hide/Exit、下载文件名与防覆盖。 |
| Studio 内真实 Browser action | 通过 | 通过 Studio WebView2 IPC 自动点击 Browser；首次懒加载完成 Toolbar、Splitter 与首个页面 WebView。 |
| 多 Tab 与关闭最后 Tab | 通过 | 自动创建 `example.org` 与 IANA Tab；关闭后台、当前、最后 Tab 后创建新的空白 Tab，无卡死。 |
| Hide 与 Exit renderer 生命周期 | 通过 | Hide 后状态为“已隐藏”且页面运行时保留；Exit 后为“未启动”，等待 WebView2 清理后 Profile 对应 renderer 已释放。 |
| 单窗口 child HWND 几何 | 通过 | Studio 680×900、Splitter 8×900、Toolbar 720×75、Page 720×825；页面紧贴 Toolbar 下方。 |
| `window.open` / popup 新 Tab | 通过 | 受控页面执行一次真实 `window.open('https://example.org/')`；WebView2 NewWindow 回调创建 Studio 内新 Tab，无额外顶层窗口或卡死。 |
| `npm run tauri build` / 安装包 | 通过 | Release EXE、MSI 与 NSIS 均成功生成；NSIS 下载使用 Tauri 的镜像环境变量，仍由 Tauri 校验 SHA-1/SHA-256。 |

## 人工验收

以下项目仍待真实用户操作或真实外部服务验证，尚未标记通过：中文输入法、20 次分隔条拖动、
最大化/恢复、响应式/固定宽页面缩放、跨 Tab 登录态与重启 Cookie、批注连续 draft、下载、
权限请求、Session/Chat/Document 回归和关闭 Studio 后无遗留后端进程。

冷启动懒加载、单窗口几何、多 Tab、关闭最后 Tab、Hide/Exit 与 Browser renderer 释放已经由
自动化 WebView2 检查验证通过。

## 平台差异

- Linux 用 GTK 原生 `GtkPaned`/`GtkStack`；Windows 用 WebView2 child HWND bounds，绝不把
  GTK/WebKitGTK 编译进 Windows 依赖图。
- Linux 能建立自定义 TLS 错误页；Windows 保留系统 TLS 阻断，当前不提供任何绕过入口。
- Windows ARM64 暂不在支持声明内。WRY 上游的第二个 WebView2 controller 死锁仍未解决；
  不以 vendored fork 或未审查 Git revision 绕过该限制。
