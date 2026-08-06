# Markdown 与 Mermaid 渲染配置

Studio 将渲染配置保存在普通的 `settings.json` 中，目前不在设置界面提供入口。

- Linux：`~/.config/codex-thread-studio/settings.json`
- macOS：`~/Library/Application Support/codex-thread-studio/settings.json`
- Windows：`%APPDATA%\\codex-thread-studio\\settings.json`

请先关闭 Studio，再编辑配置文件，然后重新启动。Studio 启动时会把默认配置补写到文件中；以后保存其他设置时也会保留这些字段。

```json
{
  "markdown": {
    "mode": "technical"
  },
  "mermaid": {
    "style": "auto",
    "density": "standard",
    "curve": "rounded",
    "layout": "auto",
    "fontSize": 15
  }
}
```

配置文件还包含 Studio 的其他设置，请保留那些字段。

## Markdown

`markdown.mode` 允许使用：

- `reading`：字号较大、行距较宽，段落、标题、列表、引用、表格和代码之间留白更多，适合阅读书籍和长篇解释。
- `technical`：正文密度适中，代码与表格较醒目，是默认模式。
- `compact`：字号、行距、段落间距和代码块留白更小，适合日志、Checklist 和快速浏览。

模式只改变排版，不改变 Markdown 语法和原文内容。标记为 `text`、`plaintext` 或 `txt` 的围栏块始终隐藏语言标题和复制按钮；编程语言代码块仍然保留二者。

## Mermaid

| 字段 | 允许值 | 含义 |
| --- | --- | --- |
| `style` | `auto`、`classic`、`neo`、`handDrawn`、`document` | 整体主题与绘图风格；`auto` 根据 Studio 明暗主题使用 Mermaid Neo。 |
| `density` | `compact`、`standard`、`loose` | 流程图节点间距、层级间距与留白。 |
| `curve` | `rounded`、`linear`、`step`、`basis` | 流程图连线曲线。 |
| `layout` | `auto`、`dagre`、`elk` | 布局引擎；`auto` 使用 Mermaid 默认布局。 |
| `fontSize` | `12`–`20` 的整数 | 图中文字大小，单位为像素。 |

图表字体族自动跟随 Studio 的 UI 字体。为保证安全和中文 SVG 文本可靠显示，以下参数固定且不能覆盖：`securityLevel: strict`、`startOnLoad: false`、`suppressErrorRendering: true`、`htmlLabels: false`。单个图表源码最多 100,000 个字符，生成的 SVG 在显示前还会再次清洗。
