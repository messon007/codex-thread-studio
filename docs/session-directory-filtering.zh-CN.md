# 会话目录过滤

Studio 可以隐藏指定项目目录中的会话，但不会删除、归档或修改 Codex/OpenCode 保存的真实历史。该过滤同时作用于两个后端、左侧所有视图及其数量。

关闭 Studio 后，编辑其普通设置文件：

- Linux：`~/.config/codex-thread-studio/settings.json`
- macOS：`~/Library/Application Support/codex-thread-studio/settings.json`
- Windows：`%APPDATA%\codex-thread-studio\settings.json`

在 JSON 顶层增加 Gitignore 风格的 `sessionDirectoryIgnore`：

```json
{
  "sessionDirectoryIgnore": [
    "# 隐藏整棵目录树",
    "/home/rui/desktop/lisource/aswcodex/",
    "**/node_modules/",
    "scratch-*",
    "!/home/rui/desktop/lisource/aswcodex/keep-this/"
  ]
}
```

规则按顺序执行，后面的规则覆盖前面。Studio 支持目录过滤所需的 Gitignore 语法：`*`、`**`、`?`、`[0-9]` 这类字符范围、末尾 `/`、`!` 反选和 `#` 注释。不含 `/` 的规则匹配任意层级的目录名；绝对路径规则匹配完整会话目录。Linux 和 WSL 路径区分大小写；Windows 盘符及 UNC 路径不区分大小写。规则中建议使用 `/` 分隔符。

旧的 `hiddenSessionDirectories` 数组仍然兼容，其每一项匹配指定目录及全部子目录；新配置建议使用 `sessionDirectoryIgnore`。

被目录规则过滤的会话也会从 Thread Router 的普通目标目录和 fallback 目标选择器中排除。搜索文字以及临时的全部/运行/待处理视图不会改变路由目标。

Studio 在启动时读取此配置。请先关闭 Studio 再编辑，因为运行期间保存其他界面设置会重写 `settings.json`。删除或反选规则并重启后，相应会话会重新出现。
