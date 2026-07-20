# Codex Thread Studio

这是一个独立、非官方、使用 Tauri 开发的 Codex 结构化桌面客户端，直接连接 [`codex app-server`](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)。

它不是 Agent Deck 前端，不嵌入终端，也不依赖 tmux；Thread、Turn、Item 和审批都直接来自 Codex App Server v2。

[English](README.md)

## 主要能力

- 按项目目录分组，直接列出、创建、自动恢复、重新载入、重命名、Fork、归档和删除 Codex Thread，并显示原生会话树、Fork 和父会话信息。
- 结构化显示用户/Agent 消息、推理摘要、执行计划、命令及输出、文件修改、工具调用、Turn 状态、错误和 Token 用量；Agent 正文使用完全本地、经过安全清洗的 GitHub 风格 Markdown，代码块和表格也有专门排版。
- 对 App Server 的高频增量事件进行批处理，只更新当前活动 Item；长响应不再反复解析整段历史记录。
- 支持开始 Turn、向运行中的 Turn 追加意见、停止 Turn，以及处理命令、文件修改和权限审批。
- 支持 `@` 搜索当前项目文件，并提供键盘优先的 `/` 命令面板，用于选择模型、推理强度、权限、查看状态、压缩上下文、发起审查、查看 Diff、选择技能、查看 MCP 服务和执行 Thread 操作。
- 提供紧凑的会话内 Turn 导航：高亮当前交互，悬停显示用户提示摘要，点击短线直接滚动到对应 Turn。
- 可以选择结构化输出、输入意见并反复积累批注；每条批注保留来源 Turn/Item 锚点，最终提示词只插入输入框，不自动发送。
- 持久化浅色/深色主题、字体、对比度、舒适/宽屏/全宽内容宽度、当前 Thread、批注草稿和可配置批注模板。
- 复用本机 Codex CLI 和已有登录状态，不保存模型凭据。

## 架构

```text
Tauri WebView
  └─ 同源 WebSocket
      └─ Rust 本地 Broker
          └─ stdin/stdout JSONL
              └─ codex app-server
                  └─ Thread / Turn / Item / Approval
```

Rust Broker 负责唯一一次 `initialize`/`initialized` 握手；之后 WebView 通过 WebSocket 转发 App Server JSON-RPC 请求、响应和事件。

## 启动

```bash
./scripts/install-linux-dev-deps.sh
codex --version
cargo run -p codex-thread-studio
```

如果图形桌面的 `PATH` 找不到 Codex：

```bash
CODEX_THREAD_STUDIO_CODEX_BIN=/Codex/绝对路径 cargo run -p codex-thread-studio
```

## 验证

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
```

桌面程序运行时不依赖 Node.js 或网络。Markdown 浏览器资源及许可证已经提交到 `ui/vendor`；维护者可运行 `npm ci && npm run vendor:markdown`，从锁定版本重新生成这些文件。

详细内容参见[成熟度路线图](docs/maturity-roadmap.md)、[系统架构](docs/architecture.md)、[产品需求与验收标准](docs/product.md)、[开发指南](docs/development.md)和[故障排查](docs/troubleshooting.md)。

## 当前生命周期边界

第一版由 Studio 进程启动一个 App Server 子进程。Codex 会持久化 Thread 历史，但关闭 Studio 可能中断正在执行的 Turn。后续里程碑是改用 App Server daemon/control socket，让执行生命周期彻底独立于窗口。

本项目采用 MIT License，是非官方 Codex 客户端。第三方 UI 依赖见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
