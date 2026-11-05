# Web desktop 阅读器功能与行为收敛清单

Current fixed-import implementation and executed local checks: [adoption status](marknoria-adoption.md). The analysis below preserves its frozen source baseline.

用户确定：MarkNoria 主维护，Studio 只跟踪 **Web desktop（browser/WebView 阅读器及其 Rust 服务/内核依赖）**。不跟踪 platform-native desktop 的 GTK/AppKit/WinUI、原生布局或 FFI，也不依赖 Studio 正在开发的原生分支。

## 目标

以两边当前能力的并集建立 Web 阅读器功能/行为基线，将 Studio 独有的通用能力补入 MarkNoria，Studio 采用同一上游组件；共同能力在相同文档、版本、设置和授权下保持行为一致。不能因为“上游为主”就丢失 Studio 功能，也不能以两个产品不同为由长期保留可消除的阅读交互差异。

基线：Studio main `a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a`；MarkNoria `c607b78689f0339351aa6a1a4c8b893bfbf09670`。以下为已提交源码核对结果，**还不是双端运行通过的证明**。职责与交付流程见 [维护计划](marknoria-maintenance.md)，源码校验和见 [差异清单](marknoria-diff-inventory.json)。

## 已发现的双向差异及要求

| 能力 | 当前证据 | 收敛要求 |
| --- | --- | --- |
| 嵌入编辑/保存 | Studio `scripts/marknoria-embedding.mjs` 提供 start_editable、编辑路由和嵌入编辑初始化；上游 standalone 有编辑，web::Service 仍只读 | 不能称为上游整体缺少编辑；补齐正式嵌入能力，同一分页、dirty、保存/冲突、撤销和关闭行为，关联 #5 |
| 宿主控制/选区 | Studio `ui/large-markdown-host.mjs` 接入预览/源码/编辑、搜索、目录、busy/dirty、选区；仍读取内部 DOM/readerExperiment | 在 MarkNoria 维护正式状态/命令/选区事件，统一 revision/位置精度，AI 动作仅留宿主 |
| 小 Markdown 表格 | Studio `ui/document-workspace-controller.mjs::mountMarkdownTableLayouts` 对普通 DOM 表格懒挂载同源 table-layout，fillWidth，离屏卸载重挂，保留列宽；上游主要在虚拟分页表使用 | 提供普通/虚拟容器同一组件；自然宽度预算、fit/fill、拖动、双击 fit、隐藏列、分页、滚动/持久化一致，关联 #3/#5 |
| 表格独立阅读器 | Studio `ui/table-reader.mjs` 有选中单元格复制、完整单元格对话框、键盘调整列宽、数据与 SQL 结果绘图；当前上游 `web/tables.mjs`/相关 Web 模块未看到等价的绘图入口 | 逐项补齐通用能力，包括安全、有界的表格/SQL 结果图表；验证源表/查询视图、sheet 和原始单元格锚点；不要只迁移 Markdown 分页表 |
| SQL/jq | 两边都有引擎/UI，但普通内存表与整文档磁盘查询输入和限额不同，上游嵌入路由不完整 | 同一组件/语义/错误/快捷键/默认交互；明确数据集及预算 profile，禁止截取数据后冒充全表查询；差异可见且有验收 |
| PDF | Studio `ui/pdf-reader.mjs` 的 PDF.js 提供文本层选择、跨页搜索、目录、页码/归一化矩形、Shift 框选；上游 `web/assets.mjs` 当前展示 PDF 栅格，尚未看到等价文本层/区域交互 | 补齐文本与区域引用、目录/搜索/定位/缩放；Poppler/PDF.js 的后端选择依据端到端性能与功能验证，不要求复制旧引擎；嵌入不应被强制新增后端 Chromium |
| EPUB | Studio `ui/epub-reader.mjs` 有 CFI/进度恢复、目录、分页/滚动、字体、light/sepia/dark、选区回调；上游走文档索引/虚拟化路径，未见同等 CFI/flow 状态接口 | 对照逐项补齐/统一交互与可持久化位置契约；转换为通用锚点时明确映射和精度，不能静默丢失已有阅读位置 |
| 多格式源码/编辑高亮 | 上游 code-view/source/editor 传递 language，支持 TOML 等；Studio 旧快照固定 Markdown | Studio 跟进更完整的上游实现，保持语言识别和主题一致；#1 继续实测选高亮库 |
| JSON 默认视图/状态提示/搜索跳转 | Studio JSON 类打开时显示 Source，dirty 提示隐藏规则不同；上游对表格命中行定位、多格式 sourceAvailable 等已更完整 | 在共享基线确定一种默认行为并两端采用，补齐定位与提示用例，不长期保留这些细微分叉 |
| 笔记/收藏/评论 | 上游 `web/notes.mjs` 已有本地评论/收藏；Studio 有格式选区与引用/评论宿主工作流 | 共享选区、锚点、笔记交互和版本契约，避免重复工具条；宿主可注入存储及附加 AI 动作。不得将“笔记”误判为纯 standalone 而一概排除 |
| 图表/本地资源/主题 | 上游 image/PDF asset 模块较新，两边图表渲染后端/依赖和主色、字体设置不同 | 以 corpus 统一实际阅读能力、配置接口和状态；共用设置下行为/视觉一致；Studio 现有 translate 接口所需的文案/locale 也须由上游组件支持，宿主配色/工具栏通过正式主题与 slots 接入 |

“未见等价入口”限定为上述已核对的提交和路径，实施时必须继续核实其它入口；若已存在，应给出可运行证据并复用，不再造一份。

## 验收

- [ ] 发布逐格式/逐动作的双端能力表（Markdown 大/小文档、text/code、JSON、CSV/TSV/XLSX、PDF、EPUB、HTML、图片/图表），每项标明入口、默认值、快捷键、状态/持久化、源码锚点、两端测试和结论。
- [ ] Studio 独有的通用功能逐项在上游提供可采用实现，或给出用户认可的明确替代；不能仅标为宿主特性而无依据地搁置。
- [ ] 相同能力/输入/设置使用同一组件、契约与默认行为，细微差异逐项消除；有必要的授权/输入规模差异通过可见能力/profile 描述并单独验证，不是永久豁免。
- [ ] 共同 fixtures 同时跑 MarkNoria browser/WebView 与 Studio 嵌入/普通 DOM 容器：结果、选区/锚点、状态转换、快捷键、可访问性、错误/取消、重挂/关闭、渲染快照与性能预算。
- [ ] 无 AI 账号的 MarkNoria 可独立阅读；Studio 保留 AI、项目/grants、网关认证与外壳，通过扩展点接线，不维护第二套通用阅读实现。
- [ ] 只验收 Web desktop 路径及其实际 Rust 依赖；platform-native desktop 不作为该同步的交付项、门槛或依赖。

上游落实任务：[MarkNoria #7](https://github.com/messon007/marknoria/issues/7)，与 #4/#5 共同完成组件与契约，再由 #6 交付；[Studio #48](https://github.com/messon007/codex-thread-studio/issues/48) 只有实际同步并验证后才勾选完成。
