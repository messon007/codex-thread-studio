# MarkNoria 主维护与 Studio 同步计划

Current fixed-import implementation and executed local checks: [adoption status](marknoria-adoption.md). The analysis below preserves its frozen source baseline. Current bounded-table adoption, host presentation correction and the remaining shared typography-layout issue are recorded in [the November 12 adoption](marknoria-large-tables-adoption.md).

用户明确排除：Studio 不需要 MarkNoria 的双栏文档比较（Compare）功能。它不属于功能并集、阅读器一致性缺口或后续同步任务；不再要求上游为 Studio 提供比较嵌入接口。此范围约定不影响现有 Git Review/diff 功能。

共享文档/阅读能力以 MarkNoria 为主维护源，Studio 按经过验证的固定版本同步。
**同步范围仅为 Web desktop 阅读器：`web/`、`src/web/` 及该路径实际使用的 Rust core/runtime、资源和构建依赖。**
MarkNoria 的 browser/WebView 使用同一阅读器；Studio 复用该阅读组件，保留自己的 Tauri/网关和 AI 工作台。
platform-native desktop 的 GTK/AppKit/WinUI、原生布局/FFI 和 Studio 正在开发的原生分支不在跟踪、对比或验收范围。
MarkNoria 的 WebView 外壳由其自己维护；不要求 Studio 改用 Tao/Wry 外壳或采用另一种原生控件。

目标是两边通用阅读能力的并集，而非只保留当前上游已有能力。Studio 独有的通用功能先补到 MarkNoria；
共同功能在相同文档、版本、设置和授权下，使用同一组件、契约、默认行为和回归用例，逐项消除细微差异。
宿主专有的 AI、文件授权、项目/工作区和外壳通过正式扩展点接线；不能用“产品差异”豁免通用阅读体验。

本轮交付是差异分析、维护约定和上下游任务建立。**运行时尚未迁移到新版 MarkNoria**。
下游：[Studio #48](https://github.com/messon007/codex-thread-studio/issues/48)；
上游：[MarkNoria #2](https://github.com/messon007/marknoria/issues/2)；
功能补齐/一致性：[MarkNoria #7](https://github.com/messon007/marknoria/issues/7)。

## 分析基线与证据

2026-11-04 通过 `git show <commit>:<path>` 读取已提交文件，不包含活动工作目录的未提交修改。

| 快照 | 提交 | 用途 |
| --- | --- | --- |
| Studio main | `a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a` | 当前正式下游 Web 阅读路径 |
| MarkNoria main | `c607b78689f0339351aa6a1a4c8b893bfbf09670` | 当前上游 Web desktop 路径 |
| Studio 内嵌 MarkNoria 基线 | `42a9b8deb7c0068f52d4e22f11b444d1a2a0c586` | 另叠加本地移植与嵌入适配 |

逐文件 SHA-256、行数见 [源码差异清单](marknoria-diff-inventory.json)；
逐项能力、证据和收敛要求见 [Web 阅读器功能与行为清单](marknoria-web-parity.md)。
清单是冻结的源码证据，不是实时同步配置或双端运行通过的证明。

- 16 个同名 Web `.mjs` 模块中 8 个字节完全一致，包括表格控制、SQL/jq 对话框和图表模块。
- 同时列出单边存在的模块及共同 HTML/CSS/theme 源码，避免只比较同名 JS 而漏掉功能/外观差异。
- Studio 的 `marknoria-queries.patch` 为 2,356 行，另有 89 行 embedding adapter。
  补丁含上下文和格式适配，不能当作净业务差异；部分功能上游已拥有，应择优收敛和删除相应补丁。
- 已确认 Studio 的嵌入编辑/控制、普通表格懒挂载与布局修复；还需补齐表格绘图/单元格操作、PDF 文本/区域选择、
  EPUB 阅读位置与显示设置等通用能力。上游较完整的语言高亮、多格式/资源、笔记也应反馈到 Studio。
  “未见等价入口”的范围和需验证之处在功能清单中明确标注，不能当作全项目缺失的证明。

维护者可用只读命令重新生成对比；只检查 Web desktop 及其实际依赖，不扫描 native desktop：

```bash
python3 scripts/compare-marknoria-sources.py \
  --marknoria-checkout /path/to/marknoria \
  --marknoria-ref <upstream-commit> --studio-ref <studio-commit> \
  --output /tmp/marknoria-source-review.json
```

读取失败报告未知且不覆盖已有报告；退出成功只代表读取/对比完成，不代表行为、性能、安全或视觉一致。

## 职责与收敛方式

| 能力 | 目标 |
| --- | --- |
| 解析、索引、source/search、分页编辑、SQL/jq | MarkNoria 维护供 Web 阅读器/嵌入宿主共同消费的公共 Rust 入口；Studio 不随内部目录做字符串替换 |
| 大/小 Markdown、CSV/TSV/XLSX、JSON/text/code、PDF、EPUB、HTML、图片/图表 | 用双方能力并集逐格式验收；上游补齐 Studio 通用功能，下游跟进上游优势，不能只按扩展名整体替换 |
| 表格、目录、搜索、选区、笔记、主题/字体、文案/locale、快捷键和状态 | 同一组件/契约和默认交互；普通 DOM 与虚拟分页使用同一机制，共同 fixtures 验证 |
| 不同输入规模、查询预算、后端/格式能力 | 明确能力/profile；统一共同场景默认值，差异可见且分别验证，禁止静默截断数据或功能 |
| AI 会话、模型、Router、工具、引用发送 | Studio 维护；上游提供通用选区/源位置及动作扩展点，不接收 AI 凭据 |
| 文件打开、项目授权、网关、应用外壳 | 两产品通过适配接入；宿主权限策略不能因组件复用而扩大 |

笔记/评论不能笼统归为 standalone 功能：MarkNoria 已有本地笔记，Studio 有选区引用及宿主工作流。
共享锚点、状态和阅读交互，存储/附加 AI 动作经正式接口注入，避免两套重叠工具栏。

宿主继续拥有 canonical-path grants、网关 token、路由白名单、缓存命名空间、生命周期和资源限额。
字节/Unicode/行列、选区精度、revision 和文档位置的含义必须显式约定；底层后端不同不能伪装为定位等价。
计算优先评估 Rust，DOM/布局留前端；端到端延迟、响应、内存及传输/序列化成本按项目性能原则实测。

## 接入阻碍与任务顺序

当前 Studio 适配针对旧平铺源码布局做替换；上游已拆 core/runtime/web。上游 standalone 的编辑、SQL/jq 等
尚未完整暴露给单文档嵌入 Service；Studio 仍用 `marknoriaHost`、`readerExperiment` 和 DOM 观察适配。
现有 vendor 工具又混合固定提交源码与活动工作目录资源，尚不能直接升级 SHA。

| 阶段 | 上游任务 | Studio 接入前的验收 |
| --- | --- | --- |
| 1 | [#3：通用修复与回归基线](https://github.com/messon007/marknoria/issues/3) | Windows 保存、表格测量/重挂和依赖修复，不重复已有 rusqlite 适配 |
| 1～2 | [#7：Web 功能补齐与行为一致](https://github.com/messon007/marknoria/issues/7) | 双向能力表、Studio 通用功能无损补齐、共同设置下行为/视觉/位置一致的共享用例 |
| 2A | [#4：Web 阅读器公共内核接口](https://github.com/messon007/marknoria/issues/4) | 无 GUI 的公共 Rust 入口、能力/单位/预算与 source 映射，无 platform-native UI 交付要求 |
| 2B | [#5：嵌入协议与 Web 组件](https://github.com/messon007/marknoria/issues/5) | 编辑/查询/资源/笔记的显式能力，版本化状态/命令/选区，普通与虚拟容器共享组件 |
| 3 | [#6：可复现交付与兼容验证](https://github.com/messon007/marknoria/issues/6) | 固定源码/资源包、manifest/哈希/许可、共享 Web fixtures 和下游组合验证 |
| 独立选型 | [#1：高亮性能与 bincode](https://github.com/messon007/marknoria/issues/1) | 实测采用性能最好的合格实现；仅 syntect 最优才等待其上游方案 |

阶段 1 建立修复/功能基线，2A/2B 实现公共接口和组件，#7 的功能/行为验收随之完成，再进入阶段 3。
两产品独立构建，本计划不要求新建第三个仓库或合仓，不改动 MarkNoria 自身 native desktop 的维护安排。

## Studio 同步与验收

1. 上游提供固定提交/版本、契约/能力、功能差异与实际测试范围后复查；issue 关闭只触发复查，不等于兼容。
2. 从明确提交/干净资源包生成独立同步 PR，核对源码/资源/锁文件同源、hash 和许可；不读活动工作树或其生成物。
3. PR 逐项列出采用功能、行为差异清除、仍待验证项和宿主适配；上游吸收的功能删除对应本地补丁/重复实现。
4. 同一 Web fixtures 同时覆盖 MarkNoria browser/WebView、Studio 嵌入及小文档普通 DOM：格式/锚点、编辑冲突、
   SQL/jq、PDF/EPUB/表格操作、主题/快捷键、选区/笔记、卸载重挂、权限、视觉和性能；保留 Studio Linux/macOS/Windows CI。
   不要求跟踪或验证上游 platform-native desktop，也不引用未合并的 Studio 原生分支作为验收证据。
5. 下游验证并合入后，在 Studio #48 更新实际采用版本、功能一致性结果、删除的补丁和剩余工作，才勾选同步完成。

Studio 通用紧急修复可先落地，需有测试、上游 issue、窄范围补丁和移除条件，随后回流；长期行为以共享上游为准。

## 发布与跟踪边界

MarkNoria 当前私有、Studio 公开。短期采用审核过、固定来源的 vendoring，逐步消费正式库/组件；
普通构建和外部 PR CI 不依赖私有令牌、兄弟 checkout、其 build/assets 或运行时服务。
交付只包含允许公开分发的源码/资源和许可，不包含用户文档、私有 fixtures、截图、缓存或凭据。

本轮建立 GitHub 任务/依赖和维护者复查流程。weekly ecosystem watch 仍监控 Codex/OpenCode 与项目健康，
**尚未自动读取私有 MarkNoria 定时同步**；交付 manifest 可用后再评估可访问的只读版本监控。
不可访问/未验证记为未知，不当作同步成功。bincode 告警仍由 #1 / Studio #25 跟踪，不添加审计忽略项。
