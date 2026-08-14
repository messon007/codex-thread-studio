# Session Resources 特性规格

状态：MVP 已实现

适用产品：Codex Thread Studio

最后更新：2026-08-14

UI 草图：[SVG](resources-panel.mockup.svg) · [PNG](resources-panel.mockup.png)

## 1. 目的

Session Resources（会话资源）从当前 Thread 最后一个 Turn 的用户/Agent 正文中识别可访问资源，在不打断阅读的前提下提供统一索引和打开入口。

资源面板是统一入口，不是新的内容渲染器：网页交给 Embedded Browser，本地文件交给 Document Viewer，目录交给 Files，代码位置在打开文件后定位到行列。功能不得使用 `file:` URL 绕过现有文件访问边界。

产品目标：

- 用户可以集中查看当前一轮刚刚产生的 URL、文件和代码位置；
- 一个资源可从消息正文直接打开，也可从右侧资源面板统一查找；
- 同一 Turn 内的相同资源合并展示，同时保留每一次出现的来源；
- Resources 是易失的当前上下文；需要长期保存的资源由用户使用 Favorites 明确收藏；
- 后续可增加 GitHub、Jira、飞书文档等增强能力，而不修改核心提取和打开流程；
- 资源识别不能拖慢流式回复、自动触发远端请求或降低文本选择体验。

## 2. 术语

| 术语 | 含义 |
| --- | --- |
| Resource | 经过规范化后可定位的网页、文件、目录、代码位置或外部实体 |
| Occurrence | Resource 在某个 Thread、Turn、Item 和文本范围内的一次出现 |
| Extractor | 从一种结构化字段或文本格式中发现 Resource 候选的 Provider |
| Resolver | 结合会话目录、Backend 和安全策略确认候选目标的组件 |
| Opener | 将已解析资源分发给 Browser、Document Viewer 或 Files 的组件 |
| Enricher | 按需获取标题、图标、PR 状态等非必要预览信息的 Provider |
| Canonical target | 用于保守去重的稳定目标；不等同于展示文本或原始输入 |
| Source anchor | 可返回资源原始 Turn、Item 和文本位置的稳定来源锚点 |

## 3. 产品决策

1. 功能名称为“会话资源”，界面短标签为“资源”，不命名为“链接”。
2. 资源图标位于会话标题栏的批注、收藏与文件工具组内，默认顺序为：批注、收藏、资源、文件、终端、Git Review。
3. Resources 与 Browser、Document Viewer、Files、Terminal、Git Review、Comments、Favorites 和 Map 共享同一右侧区域及宽度。任意时刻只显示一个右侧工作区。
4. 点击 Web URL 时在 Embedded Browser 打开；点击本地文件时在 Document Viewer 打开；点击目录时在 Files 中定位。
5. 不为所有 URL 自动生成卡片或联网抓取标题。增强预览只在悬浮、进入可视范围或用户明确打开时按需执行。
6. 基础识别必须是确定性的。AI 仅可作为未来的可选模糊引用解析器，不参与 MVP 的正确性路径。
7. Resources 只展示当前 Thread 最后一个 Turn 的资源；新 Turn 开始后不回退展示历史资源。
8. Resources 不持久化。需要跨 Turn 或跨会话长期保留的内容由用户明确加入 Favorites。

## 4. 范围

### 4.1 MVP 支持

- Markdown 正文中的显式链接和自动链接；
- 用户消息和 Agent 消息中的纯文本 HTTP/HTTPS URL；
- 用户消息和 Agent 消息正文中的明确文件路径和 `path:line:column`；
- 最后一个 Turn 的资源列表、筛选、搜索、去重、出现次数和来源跳转；
- Browser、Document Viewer 与 Files 的统一动作分发；
- Linux 与 Windows 路径规范化；
- resolved、missing、blocked 和 unresolved 状态。

### 4.2 后续支持

- xterm OSC 8 和 Terminal LinkProvider；
- 经工作区存在性验证的命令诊断路径和结构化 Tool URL 字段；
- GitHub PR、Issue、Commit，Jira、Linear 和飞书文档 Enricher；
- PDF 页码/区域、Markdown 标题、HTML 锚点等深层定位；
- 用户自定义 Extractor 和 Provider 插件；
- 经用户授权的认证预览。

### 4.3 非目标

- 不替代 Browser、Document Viewer 或 Files；
- 不自动下载、执行或上传资源；
- 不自动把网页内容加入模型上下文；
- 不在 Transcript 内嵌远端 iframe；
- 不将模糊自然语言猜测展示为已确认资源；
- 不把项目根目录外的路径当作可打开文件；
- 不对 URL query 做激进清理或跟踪参数删除。

## 5. 用户体验

### 5.1 标题栏入口

资源按钮使用链环/引用形态的 17×17 线性图标，外框尺寸与现有工具按钮一致。存在资源时显示不超过两位的计数角标；超过 99 显示 `99+`。按钮的 active 状态与其他共享右侧工具一致。

窄窗口中禁止只隐藏 Files、Terminal、Git Review 或 Resources 中的部分按钮。工具组整体保持可访问，可在极窄宽度下降级为单个“工作区工具”溢出按钮。

### 5.2 Resources 面板

面板 Header 必须与中间会话标题栏和 Browser/Document Header 的底部分隔线精确对齐。

Header 内容：

- 资源图标；
- 标题“资源”和当前会话路径；
- 刷新按钮，仅重建本地索引，不联网；
- 关闭按钮。

Header 下方依次为：

1. 搜索框：匹配展示名、原始文本、URL host、文件路径和 Provider 标签；
2. 类型筛选：全部、网页、文件、代码；仅显示计数大于零的可选分类仍保持固定顺序；
3. 资源列表：按最近出现时间降序；用户可切换为首次出现顺序，MVP 可只实现默认顺序；
4. Footer：显示资源数、引用次数以及本地索引状态。

每个资源卡片包含：

- 类型图标、主要标题和次要目标；
- 状态、Provider 标签和出现次数；
- 主动作“打开”；
- 次动作“返回消息”“复制地址”；
- 展开后的 occurrence 列表，包含用户/Agent 来源、Turn 序号和短摘录。

### 5.3 Transcript 行内行为

- Markdown 显式链接维持原有视觉样式，普通点击改为进入 Embedded Browser；
- 文件路径使用轻量下划线或悬浮强调，不增加永久胶囊背景；
- 文本选择优先。只有点击命中已识别文字范围且未发生拖动时才打开；
- 悬浮显示真实目标。当展示文字与 URL 不一致时必须明确显示实际目标；
- 右键或更多菜单提供“在右侧打开”“外部打开”“复制”“在资源中显示”；
- 命令输出和终端默认使用 Ctrl/Cmd 点击，避免影响复制和选择；
- 无效或被策略阻止的目标不打开，悬浮说明原因。

### 5.4 打开与返回

从 Resources 打开文件或网页时，右侧从 Resources 切换到目标查看器，并保持用户最近一次调整的共享宽度。目标查看器的返回按钮回到 Resources，并恢复：

- 搜索词和筛选；
- 列表滚动位置；
- 展开的资源卡片；
- 当前选中资源。

从 Transcript 直接打开资源时，关闭目标查看器恢复此前右侧工作区；若此前没有右侧工作区则关闭右侧区域。

## 6. 资源类型与打开策略

| Kind | 示例 | Resolver | 默认 Opener |
| --- | --- | --- | --- |
| `web` | `https://example.com/spec` | Browser URL policy | Embedded Browser |
| `file` | `docs/design.md` | Workspace file resolver | Document Viewer |
| `code` | `src/main.rs:42:7` | Workspace file resolver + range | Document Viewer/Editor 定位 |
| `directory` | `src/components/` | Workspace directory resolver | Files reveal |
| `issue` | GitHub/Jira URL | Generic URL resolver；可选 Enricher | Embedded Browser |
| `commit` | Commit URL | Generic URL resolver；可选 Enricher | Embedded Browser |
| `artifact` | Tool 声明的输出文件 | Declared artifact resolver | 与 MIME 匹配的 Viewer |

未知资源不得通过 Shell 自动执行。MVP 不提供“使用系统默认应用打开本地文件”。

## 7. 数据模型

```ts
type ResourceKind =
  | "web"
  | "file"
  | "code"
  | "directory"
  | "issue"
  | "commit"
  | "artifact";

type ResourceState = "unresolved" | "resolved" | "missing" | "blocked";

interface ResourceRef {
  schemaVersion: 1;
  id: string;
  kind: ResourceKind;
  raw: string;
  canonical: string;
  display: string;
  providerId: string;
  confidence: number;
  state: ResourceState;
  reason?: string;
  target:
    | { url: string }
    | {
        backend: string;
        workspaceRoot: string;
        path: string;
        line?: number;
        column?: number;
        fragment?: string;
      };
  mime?: string;
  firstSeenAt: string;
  lastSeenAt: string;
}

interface ResourceOccurrence {
  id: string;
  resourceId: string;
  threadKey: string;
  turnId: string;
  itemId: string;
  itemType: string;
  field: string;
  start?: number;
  end?: number;
  excerpt?: string;
  observedAt: string;
}
```

`ResourceRef` 与 `ResourceOccurrence` 必须分离。去重只能合并 Resource，不能丢失来源锚点。

资源 ID 建议由版本化 canonical identity 生成：

```text
sha256("resource-v1" + kind + canonicalTarget)
```

文件 canonical identity 必须包含 Backend 和规范化后的 workspace root，避免不同项目中的同名相对路径碰撞。

## 8. Provider 架构

```ts
interface ResourceExtractor {
  id: string;
  priority: number;
  supports(source: ResourceSource): boolean;
  extract(source: ResourceSource, context: ExtractContext): ResourceCandidate[];
}

interface ResourceResolver {
  id: string;
  supports(candidate: ResourceCandidate): boolean;
  resolve(candidate: ResourceCandidate, context: ResolveContext, signal: AbortSignal): Promise<ResourceRef>;
}

interface ResourceOpener {
  id: string;
  supports(resource: ResourceRef): boolean;
  open(resource: ResourceRef, context: OpenContext): Promise<void>;
}

interface ResourceEnricher {
  id: string;
  supports(resource: ResourceRef): boolean;
  enrich(resource: ResourceRef, context: EnrichContext, signal: AbortSignal): Promise<ResourcePreview>;
}
```

Provider 独立运行并隔离异常。一个 Provider 超时或抛错不得取消其他 Provider 的结果。Registry 按 priority 仲裁重叠范围：

```text
结构化字段 > Markdown/HTML AST > 诊断和堆栈 > 文件路径 > 通用 URL > AI 模糊引用
```

同一文本范围只保留优先级最高的候选；不同范围但 canonical target 相同的候选保留为多个 occurrences。

核心层不得包含 `if (github)`、`if (jira)` 等服务分支。服务特定行为只能存在于 Enricher 或专用 Extractor 中。

## 9. 提取管线

```text
Backend event / thread hydrate
  → 选择模型中的最后一个 Turn
  → narrative Item 完成或替换
  → Extractor Registry
  → overlap arbitration
  → normalize / canonicalize
  → resolve and validate
  → per-session ResourceIndex
  → Transcript affordance + Resources pane
  → optional lazy enrichment
```

### 9.1 触发时机

- 流式文本 delta：不提取；
- `item/completed`：提取该 Item；
- 已完成 Item 被替换：先按 source key 删除旧 occurrences，再重新提取；
- Thread hydrate：只从最后一个 Turn 重建；
- 用户手动刷新：只重建本地索引，不触发 Enricher；
- 会话切换：取消上一会话尚未完成的 Resolver/Enricher。

### 9.2 MVP Extractors

| Extractor | 输入 | 输出 |
| --- | --- | --- |
| `markdown-link` | Markdown AST 的 link/image/autolink | web/file candidate |
| `plain-http-url` | 用户/Agent 正文 | web candidate |
| `inline-code-path` | 用户/Agent 正文中的行内代码 | 高置信度 file/code candidate |
| `workspace-path` | 用户/Agent 正文 | 高置信度 file/code candidate |

不要把清洗后的 DOM 当作资源索引的唯一数据源。ResourceSource 应来自原始结构化 Item 或 Markdown AST，使来源范围稳定且可测试。

### 9.3 保守识别策略

Resources 只收录可合理解释为用户可打开目标的内容，不能把“包含斜杠”当成路径证据：

- fenced code、Mermaid、命令输出、推理和工具结果不进入 MVP 文本扫描范围；
- 普通相对路径必须带受支持的文件扩展名，或使用 `./`、`../`、Windows drive 等明确路径语法；
- HTML 结束标签、API route、`MC/DC`、`FIR/IIR`、`A/B/C` 等术语不得生成 Resource；
- `fileChange`、命令执行和工具调用属于活动记录，不进入 Resources；
- 后续若索引命令或工具结果，必须先由结构化字段或 workspace existence resolver 提供高置信度证明。

## 10. 规范化与解析

### 10.1 URL

- scheme 和 host 转小写；
- 移除默认端口；
- 保留 path、query 和原始导航 URL；
- fragment 可从去重 identity 中分离，但必须保留为 occurrence 的深层定位信息；
- 不自动删除 query 参数；
- Unicode host 使用 URL 标准格式解析并在 UI 中显示安全形式；
- URL 中的用户名或密码直接标记 blocked；
- 最终打开时再次调用 `validate_browser_url`，不能依赖提取期结果。

### 10.2 文件

- 相对路径基于产生 Item 时的会话 working directory，而不是 Studio 进程 cwd；
- 同时支持 `/` 与 Windows `\`，展示使用平台习惯，canonical identity 使用统一分隔符；
- Windows 驱动器盘符比较不区分大小写；
- `.`、`..` 和符号链接解析后必须仍位于会话 root；
- quoted path、空格、括号、尾随标点应由 source-aware parser 处理，不能依赖单个巨型正则；
- 行列必须为正整数，超出文件范围时打开文件但定位到最后有效位置，并提示已调整。

### 10.3 懒解析

结构化文件和明确 URL 可立即解析。昂贵或可能访问磁盘的路径验证允许延迟到面板打开、资源进入可视区或用户悬浮时执行。列表先显示 `unresolved`，解析完成后原位更新，顺序不跳动。

## 11. ResourceIndex

建议使用进程内、按 Session 隔离的索引：

```ts
class SessionResourceIndex {
  resourcesById: Map<string, ResourceRef>;
  occurrencesById: Map<string, ResourceOccurrence>;
  occurrenceIdsByResource: Map<string, string[]>;
  occurrenceIdsBySource: Map<string, string[]>;
  orderedResourceIds: string[];
}
```

MVP 不增加数据库 migration。索引只从 native Thread 模型的最后一个 Turn 确定性重建，并在进程内按 Session 隔离。新 Turn 替换旧索引。长期保存是 Favorites 的职责，Resources 不建立第二套历史数据库。Enricher 若需要 TTL 缓存，只缓存展示元数据，不改变这一产品边界。

## 12. 与现有模块的集成

### 12.1 Transcript

在 `renderMarkdown` 前后传递 ResourceOccurrence 映射，使用来源范围生成安全的 `data-resource-id`，而不是渲染完成后再次扫描整棵 DOM。`handleTranscriptClick` 只负责把资源 ID 交给 ResourceActionRouter。

### 12.2 Browser

Web Opener 复用 `openBrowserUrl()` 和 Rust `validate_browser_url`。Resources 不管理 Tab、Cookie、Profile 或下载。

### 12.3 Document Viewer

File/Code Opener 复用 `openArtifact()`，并扩展可选的 line、column、fragment。Resources 不读取或渲染文件正文。

### 12.4 Files

Directory Opener 切换到 Files，逐级展开并选中目标。文件无法预览时可在 Files 中定位，但不得自动调用系统 Shell。

### 12.5 Terminal

Terminal LinkProvider 属于第二阶段。它可复用同一个 Resolver 和 Opener，但终端滚动缓冲区与结构化 command Item 默认都不进入 SessionResourceIndex；用户在终端中明确点击链接即可打开，不扩大 Resources 的索引范围。

### 12.6 Shared right workspace

Resources 必须注册为现有共享右侧 workspace 类型，不能新增独立 overlay 或第二套 resize 逻辑。打开子资源前记录 `returnWorkspace = resources`，关闭目标后恢复；用户从其他入口打开目标时沿用现有返回规则。

## 13. Enricher 与预览

Enricher 只能增强显示，不能决定资源是否存在或能否打开。

允许的懒加载字段：

- 网页标题、favicon、MIME；
- GitHub PR/Issue 编号、状态和仓库；
- 文档服务的类型和最近更新时间。

约束：

- 默认不带 Browser Cookie；
- 需要认证的 Provider 必须声明独立账户和 scope；
- 缓存按 provider + account + canonical target 隔离；
- 支持 TTL、ETag、超时、大小和并发限制；
- 离线或失败时保留基础资源，不显示破损占位卡；
- 用户可在设置中整体关闭联网预览。

## 14. 安全与隐私

1. 识别 URL 本身不得产生网络请求。
2. 所有导航入口和重定向继续共用 Browser URL policy。
3. Preview Fetcher 必须有独立的 SSRF 防护：DNS 解析复检、重定向逐跳校验、私网策略、响应大小、MIME 和超时限制。
4. 本地文件必须经过现有 workspace root confinement；Browser 不接受 `file:`。
5. 不将 URL query、文件绝对路径、预览内容或账户信息写入分析遥测。
6. 复制操作复制原始目标；UI 必须在执行前让用户看见真实目标。
7. 外部协议、可执行文件和 shell 片段永不自动执行。
8. Enricher 返回内容必须经过转义或 DOMPurify，不能向 Transcript 注入 HTML。
9. Provider 不能获得完整会话；只接收完成任务所需的 ResourceRef 和最小上下文。

## 15. 性能预算

- Item 完成后的同步提取目标：普通消息 P95 小于 8 ms；
- 单 Item 文本扫描上限：1 MiB，超过部分不扫描并记录本地诊断；
- 单 Item occurrence 上限：100；单会话默认展示上限：2,000 个 Resource，可继续搜索已索引结果；
- Resolver 并发不超过 8；联网 Enricher 默认并发不超过 3；
- 切换会话或关闭面板后 100 ms 内发出 AbortSignal；
- Resources 首次打开不等待联网预览；
- 索引更新不得触发 Transcript 全量重绘或改变当前滚动位置。

性能数字是初始工程预算，可在基准测试后调整，但任何调整必须记录原因。

## 16. 错误与状态

| 状态 | UI | 行为 |
| --- | --- | --- |
| `unresolved` | 中性旋转/省略状态 | 可触发解析，不允许猜测打开 |
| `resolved` | 正常类型图标 | 启用打开 |
| `missing` | “文件不存在” | 保留来源跳转和复制 |
| `blocked` | 锁形标记和原因 | 禁止打开，允许复制经过安全处理的显示值 |
| Provider error | 不污染 Resource 状态 | 记录本地诊断，可重试对应 Provider |

面板重建失败时保留上一版索引并显示“结果可能已过期”，而不是清空列表。首次构建失败则显示可重试错误页。

## 17. 可扩展性要求

- Registry 可注册多个 Provider，Provider 通过 capability 判断支持范围；
- API 数据结构带 `schemaVersion`，新增字段保持向后兼容；
- Provider 有稳定 ID、优先级、超时和可取消能力；
- Extractor、Resolver、Opener、Enricher 之间不共享可变内部状态；
- 第三方 Provider 不能覆盖核心 Browser/File 安全校验；
- 服务特定 Provider 删除后，资源仍可退化为 generic web/file；
- 所有 Provider 必须通过相同的 contract tests。

## 18. 测试策略

### 18.1 单元测试

- URL 尾随标点、括号、Unicode、query、fragment、账号密码；
- Linux、Windows、UNC、空格、quoted path、line/column；
- Markdown AST、HTML、代码块、diff、Rust/Python/JS 堆栈；
- Provider 重叠和优先级；
- 去重后 occurrence 完整性；
- Item 替换、删除和重复 hydrate 的幂等性；
- root confinement、符号链接逃逸和 blocked 状态。

### 18.2 Contract tests

每个 Provider 验证：

- 输入不可变；
- 异常隔离；
- AbortSignal 生效；
- 不产生未声明的网络或文件访问；
- 输出符合 schema、范围合法且结果稳定。

### 18.3 集成测试

- Markdown URL → Browser；
- file change → Document Viewer；
- code location → 文件与行列；
- directory → Files reveal；
- Resources → 文件 → 返回 Resources 状态恢复；
- 在 Browser、Comments、Favorites、Files 间切换时宽度不变；
- 切换 Session 后索引和来源不串线；
- Codex 与 OpenCode 产生等价 Resource 模型。

### 18.4 视觉验收

- Header 分隔线与中间会话、Browser、Document Viewer 精确对齐；
- 工具按钮尺寸、间距、字体和 active 状态与现有 UI 一致；
- 100%、125%、150% 缩放以及半屏宽度下无工具按钮静默消失；
- 长 URL、长 Windows 路径、中文文件名不会撑宽面板；
- 拖动右侧分隔线不会选择左右文本；
- 浅色和深色主题均满足现有对比度基线。

## 19. MVP 验收标准

1. 最后一个 Turn 正文中的显式 Markdown URL 和纯文本 HTTP/HTTPS URL 均能出现在 Resources，并可在 Embedded Browser 打开。
2. 最后一个 Turn 正文中的明确文件路径能出现在 Resources，并可在 Document Viewer 打开。
3. 相同 canonical target 只显示一个资源，出现次数正确，所有来源都能返回最后一个 Turn。
4. 打开资源后关闭或返回，恢复 Resources 的搜索、筛选、滚动和选中状态。
5. Resources 与所有右侧工作区共享宽度和分隔线，不同时显示。
6. 流式消息期间不重复扫描；Item 完成后资源在 100 ms 内出现在本地索引。
7. URL 识别不产生网络请求；blocked URL 和 root 外文件不能打开。
8. Provider 失败不阻止其他资源类型工作，刷新不会清空仍有效的旧结果。
9. Linux 和 Windows 路径测试均通过。
10. fenced code、Mermaid、HTML 标签、API route 和斜杠术语不会产生资源误报。
11. 历史 Turn 和 `fileChange` 不进入 Resources；需要长期保留时使用 Favorites。
12. Rust tests、前端 tests、Clippy 和 release check 保持通过。

## 20. 分阶段实施建议

### Phase 1：确定性核心

- Resource model、Registry 和 SessionResourceIndex；
- Markdown URL、plain URL、workspace path Extractors；
- Resources 面板和共享右侧工作区集成；
- Browser/File/Directory Openers；
- 来源跳转、复制、去重和基础状态。

### Phase 2：代码工作流

- 诊断/堆栈 Extractors；
- line/column 和文档 fragment；
- xterm LinkProvider；
- 大输出限制、增量更新和完整 Windows 路径语料。

### Phase 3：服务增强

- Generic Enricher 基础设施；
- GitHub Provider；
- 授权、TTL 缓存和安全 Preview Fetcher；
- Provider 管理设置。

### Phase 4：收藏协作

- 从 Resource 快速创建 Favorite（MVP 已完成）；
- Favorite 保留来源会话和原始响应锚点；
- Resources 继续保持易失，不演化为第二套历史索引。

## 21. 待评审决策

以下问题不阻塞 specification，但应在开发 Phase 1 前确认：

1. 资源按钮计数显示唯一 Resource 数还是 occurrence 数；本规格建议显示唯一 Resource 数。
2. Markdown 图片是否默认归入“文件/网页”还是增加“图片”筛选；本规格建议 MVP 归入目标类型，不增加筛选。
3. 当前会话 root 外但确实存在的绝对路径是否只允许复制；本规格建议标记 blocked，不提供系统打开。
4. 外部浏览器动作是否需要设置项；本规格建议保留右键显式动作，不作为默认行为。

## 22. 对标依据

- [VS Code DocumentLinkProvider](https://code.visualstudio.com/api/references/vscode-api)：多 Provider 合并与懒解析目标；
- [VS Code TerminalLinkManager](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/terminalContrib/links/browser/terminalLinkManager.ts)：终端 Detector、Provider 与 Opener 分层；
- [xterm.js Link Handling](https://xtermjs.org/docs/guides/link-handling/)：OSC 8、WebLinksAddon、修饰键点击与真实目标展示；
- [JetBrains Console Filter](https://plugins.jetbrains.com/docs/intellij/execution.html)：把控制台文本范围转换为可扩展链接；
- [Slack link unfurl](https://docs.slack.dev/messaging/unfurling-links-in-messages/) / [Work Objects](https://docs.slack.dev/messaging/work-objects-overview/)：轻量识别、异步增强和稳定外部实体；
- [Notion embeds and connected apps](https://www.notion.com/en-gb/help/embed-and-connect-other-apps)：资源与 link/bookmark/mention/embed 展示方式分离；
- [Obsidian Outgoing Links](https://obsidian.md/help/plugins/outgoing-links) / [unresolved links API](https://docs.obsidian.md/Reference/TypeScript%20API/MetadataCache/unresolvedLinks)：文档级索引、来源与未解析状态。

本项目的实现应优先复用现有 `openBrowserUrl()`、`openArtifact()`、Files reveal、共享右侧宽度和 Provider registry 思想，不引入第二套浏览器、文件读取器或 resize 技术。
