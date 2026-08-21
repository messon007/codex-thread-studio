# Document Viewer 目录与章节导航 Specification

状态：已实现，并完成自动化测试与 Linux 实机 UI 验收。

## 1. 目标

为 Document Viewer 增加统一的“目录”入口，让用户查看文档结构、识别当前章节，并跳转到具体章节。目录属于 Document Viewer 的导航能力，不是新的右侧工作区，也不改变当前共享右栏宽度。

本设计重点解决：

- Markdown、HTML、EPUB 和带书签的 PDF 使用同一个目录入口与一致的交互；
- 在预览、源码和编辑视图中跳转到同一个逻辑章节；
- 阅读滚动时自动标记当前章节；
- 目录始终覆盖在正文之上，不改变正文的可见宽度和滚动位置；
- 目录 UI 与现有 Document Header、刷新和关闭按钮保持同一尺寸、边框和图标风格。

术语固定如下：界面使用“目录”和“文档目录”；代码内部使用 `DocumentOutline`。不使用“Map”“资源”“章节树”等可能与现有功能混淆的名称。

## 2. 范围

### 2.1 支持

| 文档类型 | 目录来源 | 跳转目标 |
| --- | --- | --- |
| Markdown | 解析 H1–H6 标题，忽略 fenced code 内的伪标题 | 预览标题、源码行、编辑器行 |
| HTML | 清理后的 DOM 中的 H1–H6；保留合法 `id`，否则生成稳定 ID | 预览标题、源码中对应标签、编辑器位置 |
| EPUB | EPUB navigation document / NCX | CFI 或 spine href |
| PDF | PDF Outline / Bookmarks | explicit destination 或页码 |

无结构的纯文本、图片、CSV/XLSX 不显示目录按钮。PDF 没有书签时也不显示，避免出现一个永远为空的入口。

### 2.2 不在本阶段

- 不用 AI 猜测章节；目录必须来自确定性结构。
- 不自动改写缺少标题的文档。
- 不提供跨文件目录或项目级文档索引。
- 不把搜索结果、评论和收藏混入目录。
- 不提供目录拖拽重排；它是只读导航，不是结构编辑器。

## 3. UI 结构

### 3.1 入口

在 Document Header 的操作区最左侧增加 32 × 32 px 图标按钮，位于搜索、Preview/Source/Edit、保存、刷新、关闭之前。

- 图标：三条不同长度的横线，左侧带小圆点，表达层级目录；沿用现有 1.65 px 圆角线条。
- 默认：透明背景、`muted` 颜色。
- Hover：`panel-soft` 背景。
- 打开：`brand-soft` 背景、`brand` 图标、品牌色边框。
- Tooltip / aria-label：`文档目录`。
- 目录为空或文档类型不支持：按钮完全隐藏，不显示 disabled 状态。

按钮与刷新、关闭均使用现有 `.icon-button.artifact-action-icon` 几何，不增加文字，保证窄宽度下仍然可见。

为避免目录加入后 Header 过度拥挤，现有文档搜索默认收为同尺寸的搜索图标。点击搜索后，在 Header 下展开现有搜索栏（输入、匹配数、上一个、下一个）；再次点击、按 Escape 或清空并失焦后收起。搜索算法和结果高亮不在本需求中改变。这样目录、搜索、刷新和关闭始终是同一套图标尺寸，文件名也不会被操作区完全挤掉。

### 3.2 目录面板

目录面板位于 Document Header 下方、Document Footer 上方，是覆盖在文档画布之上的左侧抽屉。它不创建第二个右栏、不参与正文 flex 布局，也不改变用户拖拽得到的共享右栏宽度。

- 宽度：`min(420px, 100% - 20px)`；与正文区域四周保留 10 px 间距。
- 面板标题行：`目录`、章节数量、与现有关闭操作一致的 32 × 32 px 线框图标。
- 筛选框：`筛选章节…`，使用 12 px 正文字号，只过滤显示项，不改变正文。
- 列表：扁平渲染的层级树；使用 12 px 字号和至少 37 px 行高，每级缩进 16 px，视觉缩进最多四级，语义深度完整保留。
- 长标题最多显示两行，按 Unicode 安全换行；仍通过原生 tooltip 提供完整标题。
- 当前章节：浅品牌色背景、左侧 2 px 品牌色指示条、较高字重。
- 有子项的章节支持展开/折叠；默认展开当前章节的祖先和最上层节点。
- 列表底部不增加说明卡片，保持导航工具简洁。

### 3.3 Overlay 行为

所有窗口宽度均采用同一个 overlay 模式，不再根据 680 px 阈值切换 docked/overlay。目录带完整边框、圆角、轻量阴影和透明遮罩；正文保持原有尺寸和滚动位置。点击章节只导航，不自动关闭目录，方便连续浏览多个章节；用户点击遮罩、关闭按钮、Header 目录按钮或按 Escape 后关闭。

目录开合不能改变 `--right-rail-width`，不能触发共享右栏宽度持久化，也不需要 ResizeObserver 或响应式布局状态。

## 4. 交互

### 4.1 打开与关闭

- 点击 Header 目录图标切换面板。
- `Escape` 先关闭目录，再处理 Document Viewer 的其他退出行为。
- 打开、刷新或关闭文档时目录恢复关闭状态；目录开合是当前阅读上下文中的临时状态，不写入 Studio preferences。
- EPUB 现有内部目录按钮和抽屉迁移到统一入口，避免两个目录按钮同时出现。

### 4.2 章节跳转

点击目录项后：

1. 立即更新选中状态；
2. 导航 Provider 定位目标；
3. 预览模式采用短距离平滑滚动，源码/编辑/PDF/EPUB 直接定位，避免长距离动画；
4. 目标章节短暂显示品牌色定位提示；
5. 目录保持打开；用户确认查看正文时再手动关闭。

Markdown 和 HTML 在三种视图中共用逻辑章节 ID：

- Preview：滚动到渲染后的标题，顶部预留 18 px；
- Source：滚动到标题源行，并短暂高亮该行；
- Edit：调用 CodeMirror `dispatch({ selection, scrollIntoView: true })`，光标放到标题行首，不修改内容。

### 4.3 阅读位置同步

- Markdown/HTML：使用 `IntersectionObserver` 观察标题；若没有标题进入观察区，选择滚动位置上方最近的标题。
- PDF：页码变化时匹配包含该页或最近前置 destination 的书签。
- EPUB：使用现有 `relocated` 事件和 href/CFI 匹配。
- 当前位置变化时目录只更新 active 状态；仅当 active 项离开目录可视区时自动滚到最近位置，不持续抢夺用户正在操作的目录滚动。

### 4.4 筛选

筛选按 Unicode 不区分大小写匹配章节标题：

- 命中子项时保留其祖先作为路径；
- 祖先仅作为上下文显示，不计入命中数量；
- 清空筛选恢复用户此前的折叠状态；
- `Enter` 跳转到第一个命中项。

## 5. 数据模型与 Provider 边界

Document Shell 只管理覆盖抽屉、筛选和 active 状态。文档格式通过统一接口提供结构和导航：

```ts
type DocumentOutlineItem = {
  id: string
  label: string
  depth: number
  target: unknown
  children?: DocumentOutlineItem[]
}

type DocumentNavigationProvider = {
  getOutline(): Promise<DocumentOutlineItem[]> | DocumentOutlineItem[]
  navigate(target: unknown, context: { view: string }): Promise<void> | void
  subscribeLocation?(listener: (location: unknown) => void): () => void
  matchLocation?(location: unknown): string | null
}
```

Provider 约束：

- `id` 在同一份内容和同一解析版本内稳定；重复标题通过出现序号消歧。
- `label` 去除 Markdown 装饰和 HTML 标签，只保留可读文字；空标题显示 `未命名章节`。
- 单项 label 最多 512 个 Unicode 字符，目录最多 2,000 项，超过后显示前 2,000 项并在面板尾部提示已截断。
- Shell 不解析 EPUB CFI、PDF destination 或 CodeMirror 文档位置。

## 6. 各格式实现策略

### Markdown

从 `marked` token 树提取 heading，保留源文本 offset/line。不得用正则扫描整份 Markdown，避免把代码块内的 `#` 当标题。预览渲染时为标题写入 `data-outline-id`，不依赖标题文字反查 DOM。

### HTML

在 DOMPurify 清理后读取 H1–H6。保留唯一且安全的原始 `id`；重复或缺失时用文档内稳定序号生成 `studio-heading-N`。源码定位使用解析时保留的标签顺序和近似 offset；无法可靠匹配时只在 Preview 提供跳转，不能跳到错误源码位置。

### EPUB

复用现有 `flattenEpubToc()`、href/CFI 导航和 `relocated` 状态，将目录数据与导航 API 上提到 Document Shell。字体、主题、翻页工具仍保留在 EPUB 内部工具栏。

### PDF

调用 PDF.js `getOutline()`，递归解析 bookmarks，并用 `getDestination()` / `getPageIndex()` 解析目标。无 outline 时隐藏入口。点击书签后调用现有 reader 的 `goToPage()` 或新增 `goToDestination()`。

## 7. 状态与生命周期

- `state.artifactOutlineOpen`：仅表示当前 Document Viewer 的临时开合状态，不持久化；打开、刷新或关闭文档时重置。
- `state.artifactOutlineFilter`：仅当前文档内存状态，切换文档后清空。
- `state.artifactOutlineActiveId`：由 Provider 位置事件更新，不持久化。
- 文档刷新、切换视图、关闭或销毁 reader 时，必须注销 observers/listeners。
- 编辑模式内容变化后 250 ms debounce 重建 Markdown/HTML 目录；尽量按稳定 ID 保留 active 和折叠状态。

## 8. 性能与安全

- 目录解析复用已经载入内存的文档，不发起网络请求。
- 解析结果按 `root + path + content hash + kind` 缓存；刷新或保存后失效。
- 大文档的 DOM 观察只针对 heading，不观察全部段落。
- HTML 目录只读取已经清理的 DOM；目录 label 始终使用 `textContent` 渲染，不插入 HTML。
- EPUB/PDF target 只交给对应 Provider，不能形成任意 URL 或文件路径。

## 9. 验收标准

1. Markdown 预览中打开目录，点击 H1/H2/H3 能准确跳转，当前章节随滚动更新。
2. 同一 Markdown 切换 Source 或 Edit 后，点击相同目录项定位到对应标题行。
3. fenced code 中的伪标题不进入目录；重复标题可以分别跳转。
4. HTML 标题可跳转，脚本和被清理内容不会进入目录。
5. EPUB 只有一个目录入口，原有章节跳转、当前位置和阅读状态不退化。
6. 带书签 PDF 显示目录并能跳页；无书签 PDF 不显示目录按钮。
7. 图片、纯文本、CSV/XLSX 不显示目录按钮。
8. 所有宽度下目录均覆盖正文；正文宽度、滚动位置和共享右栏宽度不发生变化。
9. 目录、搜索、刷新、关闭图标均为 32 × 32 px，Header 分隔线仍与中间会话 Header 精确对齐。
10. 编辑标题后目录在 250 ms 左右刷新，保存、刷新、关闭无 listener 泄漏。

## 10. UI 决策摘要

采用一个统一 Header 图标和一块始终覆盖正文的宽目录抽屉。目录是临时导航工具，不参与正文布局；这种单一模式消除了窗口阈值造成的布局变化，同时避免把目录做成新的共享右栏工具。
