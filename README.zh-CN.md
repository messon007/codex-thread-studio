# Codex Thread Studio

这是一个独立、非官方、使用 Tauri 开发的 Codex/OpenCode 结构化桌面客户端，直接连接 [`codex app-server`](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) 或本机 [`opencode serve`](https://opencode.ai/docs/server/)，不解析终端字符。

它不是 Agent Deck 前端，不嵌入终端，也不依赖 tmux；Thread、Turn、Item 和审批都直接来自 Codex App Server v2。

[English](README.md)

## 主要能力

- 按项目目录分组，直接列出、创建、自动恢复、重新载入、重命名、Fork、归档和删除 Codex Thread，并显示原生会话树、Fork 和父会话信息。
- 结构化显示用户/Agent 消息、推理摘要、执行计划、命令及输出、文件修改、工具调用、Turn 状态、错误和 Token 用量；Agent 正文使用完全本地、经过安全清洗的 GitHub 风格 Markdown，代码块和表格也有专门排版。
- 对 App Server 的高频增量事件进行批处理，只更新当前活动 Item；长响应不再反复解析整段历史记录。
- 支持开始 Turn、向运行中的 Turn 追加意见、停止 Turn，以及处理命令、文件修改和权限审批。
- 支持 `@` 搜索当前项目文件、直接输入 `$技能名` 发现并发送 App Server 结构化技能输入、以 `!命令` 运行本地 Shell，并提供键盘优先的 `/` 命令面板，用于选择模型、推理强度、权限、查看状态、压缩上下文、发起审查、查看 Diff、选择技能、查看 MCP 服务和执行 Thread 操作。
- 提供紧凑的会话内 Turn 导航：高亮当前交互，悬停显示用户提示摘要，点击短线直接滚动到对应 Turn。
- 可以选择结构化输出、输入意见并反复积累批注；每条批注保留来源 Turn/Item 锚点，最终提示词只插入输入框，不自动发送。
- 可以直接收藏一条完整 AI 回复，自动关联同一 Turn 的用户问题并由用户决定是否保存；全局收藏库支持跨后端、跨会话搜索和返回原消息。
- 收藏使用 SQLite 存储，可自动迁移旧版 JSON 收藏，并将完整全局收藏导出为 Markdown。
- 持久化简体中文/English/跟随系统语言、浅色/深色主题、字体、对比度、舒适/宽屏/全宽内容宽度、当前 Thread、批注草稿和分语言批注模板。
- 可在 Codex 与 OpenCode 间切换；两种后端的会话选择和批注草稿彼此隔离。
- 复用本机 CLI 和已有登录状态，不保存模型凭据。Rust 会用仅存在于内存中的随机密码保护 OpenCode 子进程。

## 架构

```text
Tauri WebView
  └─ Rust 本地网关
      ├─ WebSocket ↔ stdin/stdout JSONL ↔ codex app-server
      └─ 同源 HTTP/SSE 代理 ↔ opencode serve
```

Rust Broker 负责 Codex 的 `initialize`/`initialized` 握手，以及 OpenCode 进程、随机端口和认证；WebView 只访问 Studio 的同源网关。

## 启动

```bash
./scripts/install-linux-dev-deps.sh
codex --version
opencode --version
cargo run -p codex-thread-studio
```

如果图形桌面的 `PATH` 找不到 Codex：

```bash
CODEX_THREAD_STUDIO_CODEX_BIN=/Codex/绝对路径 cargo run -p codex-thread-studio
```

OpenCode 不在图形桌面 `PATH` 中时，可设置 `CODEX_THREAD_STUDIO_OPENCODE_BIN=/OpenCode/绝对路径`。

## 验证

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
npm run version:check
```

桌面程序运行时不依赖 Node.js 或网络。Markdown 浏览器资源及许可证已经提交到 `ui/vendor`；维护者可运行 `npm ci && npm run vendor:markdown`，从锁定版本重新生成这些文件。

## 版本与发布

`src-tauri/Cargo.toml` 是唯一需要人工维护的应用版本来源，Tauri 会自动继承；About 面板显示相同的编译期版本。

发布说明记录在 [CHANGELOG.md](CHANGELOG.md)。维护者先在 **Unreleased** 下填写变更，运行 `npm run version:bump -- <semver>`，再以 `npm run version:check` 验证。提交后推送匹配的 `v<semver>` Tag，GitHub 工作流会核对所有版本信息，并创建包含 Linux、macOS 安装包的草稿 Release。

详细内容参见[成熟度路线图](docs/maturity-roadmap.md)、[系统架构](docs/architecture.md)、[产品需求与验收标准](docs/product.md)、[开发指南](docs/development.md)和[故障排查](docs/troubleshooting.md)。

## 当前生命周期边界

当前由 Studio 管理本次运行中使用的后端子进程。Codex/OpenCode 会持久化会话历史，但关闭 Studio 仍可能中断正在执行的任务。

本项目采用 MIT License，是非官方 Codex 客户端。第三方 UI 依赖见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
