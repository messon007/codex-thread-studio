# MarkNoria 主维护与 Studio 同步计划

共享文档/阅读能力以 MarkNoria 为主维护源，Studio 按经过验证的固定版本同步。
Studio 负责 AI 工作台、宿主授权和集成；MarkNoria 负责可复用文档能力及独立阅读器。
现有实现逐模块择优收敛，不以项目归属推断性能或功能等价，也不整体覆盖另一个产品。

本轮交付是差异分析、维护约定和上下游任务建立。**运行时尚未迁移到新版 MarkNoria**。
下游总跟踪：[Studio #48](https://github.com/messon007/codex-thread-studio/issues/48)；
上游总任务：[MarkNoria #2](https://github.com/messon007/marknoria/issues/2)。

## 分析基线与证据

2026-11-04 的分析使用已提交内容，通过 `git show <commit>:<path>` 读取，未包含两个活动工作目录的未提交修改。

| 快照 | 提交 | 用途 |
| --- | --- | --- |
| Studio main | `a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a` | 当前正式下游 |
| MarkNoria main | `c607b78689f0339351aa6a1a4c8b893bfbf09670` | 拟作为主维护源的当前上游 |
| Studio 原生开发分支 | `1e4e4a459e7959192e607e1ac66693ffb383517c` | 仅作差异参考，尚未合入 main，本轮不修改 |
| Studio 内嵌 MarkNoria 基线 | `42a9b8deb7c0068f52d4e22f11b444d1a2a0c586` | 另叠加本地移植与嵌入适配 |

逐文件 SHA-256、行数和抽样相似度见 [分析清单](marknoria-diff-inventory.json)。
它是冻结的证据快照，不是实时同步配置，也不代表功能、安全或性能等价。
清单同时列出只在一边存在的顶层 Web 模块；上游 standalone 的文件浏览/笔记等模块不能因此被自动导入宿主。

- 16 个同名 Web `.mjs` 模块中 8 个字节完全一致，包括表格控制、SQL/jq 对话框和图表模块。
- 原生分支与上游的 structured、SVG resources、image animation 样本按行相似度约为 98.2%、99.7%、97.0%；
  上游 third-party 记录也明确了 Studio 来源。相似度只用于发现共用候选。
- Studio 当前 `marknoria-queries.patch` 为 2,356 行，另有 89 行 embedding adapter。
  补丁含新增功能、上下文和格式适配，不能当作 2,356 行净业务差异；但其中一些功能上游已拥有，长期保留会增加同步成本。

维护者可以重新生成源码对比（不读取未提交文件，也不改变任何 Git 引用）：

```bash
python3 scripts/compare-marknoria-sources.py \
  --marknoria-checkout /path/to/marknoria \
  --marknoria-ref <upstream-commit> --studio-ref <studio-commit> \
  --output /tmp/marknoria-source-review.json
```

原生样本需另外明确传入 `--native-checkout`、`--native-ref`，可用 `--native-label` 标注来源；
不提供时报告明确标记“未扫描”，不推断已经兼容。退出成功只代表读取/对比完成，不能代替后续语义、性能和平台验证。

## 职责与差异矩阵

| 能力 | 当前差异 | 维护与接入方向 |
| --- | --- | --- |
| 整文档解析、索引、source/search、编辑、表格投影 | Studio 使用旧平铺模块；上游已拆 core/runtime/web/native | MarkNoria 提供稳定公共入口；Studio 用适配层，不随内部路径逐处修补 |
| structured / SVG / animation | 存在高度同源副本及宿主存储、错误和传输适配 | 先收敛工具包无关的计算/数据模块，保留来源和许可 |
| SQL/jq | 普通表格内存查询与整文档磁盘分页不同，限额/输入也不同 | 共享引擎、授权和预算机制，保留明确输入与宿主策略；不静默统一默认限额 |
| 小 Markdown / 聊天流 / 原生投影 | 主线小文件 Comrak/WASM、大文件 MarkNoria；原生分支另有片段模型 | 先用 corpus 比较 GFM、原文映射和端到端成本，再逐项迁移；流式增量不强制创建整文档会话 |
| PDF / 图表 / EPUB 等格式 | 上游 PDF 使用 Poppler 子进程，下游存在其他后端；图表和原生/浏览器表现不同 | 先明确能力和后端边界，验证现有操作及依赖预算，不能只按扩展名或功能名替换 |
| Web 表格、目录、查询、分页与选区 | 多个文件一致，但宿主桥接依赖私有 DOM/调试全局 | 上游维护组件和正式宿主协议，Studio 保留主题与 AI 动作接线 |
| AI 会话、模型、Router、工具、评论发送 | Studio 产品行为 | 留在 Studio；共享选区/源位置，不迁移 AI 业务 |
| standalone 窗口、文件打开、独立笔记 | MarkNoria 产品行为 | 留在 MarkNoria；不把 standalone 会话偏好强加给 Studio |
| GTK / WebView / AppKit / WinUI 控件 | 平台与产品容器不同 | 先共享 Rust 数据、计算和契约，UI 仅在边界稳定后复用；保留两边现有受维护目标 |

原生参考快照尚未进入远端 main，不能假定它已可公开下载。MarkNoria 可以先公共化自己已有的同源实现；
Studio 原生替换/逐字兼容验证，等该分支合入或有经授权的可访问快照后再执行，不阻塞当前接口设计或要求另一工作目录。

宿主继续拥有 canonical-path grants、网关 token、路由白名单、缓存命名空间、生命周期和资源限额。
共享内核不能扩大宿主文件权限或取得 AI 凭据。字节偏移、Unicode 字符偏移、行列、选区精度和 revision 的含义必须显式约定。

## 已确认的接入阻碍

1. Studio 适配仍针对 `src/server.rs` 等旧布局做源码替换；上游已移动到 `src/web/mod.rs` 等目录。
2. 上游嵌入 Service 仅有只读启动，router 没有 standalone 已拥有的完整编辑和 SQL/jq 嵌入入口；
   embedded reader 仍跳过编辑初始化。Studio 的 `start_editable` 和 `marknoriaHost` 是本地补丁。
3. Studio host 从 `readerExperiment`、DOM 选择器和 MutationObserver 获取状态/选区；这些不应继续作为公共接口。
4. 现有 vendor 工具对源码使用固定提交，却从活动工作目录读取部分构建资源；源码提交与资产来源可能不一致。
5. Studio 已验证的 Windows 保存句柄处理、表格测量预算/卸载清理和部分依赖修复尚需核对回流。
   上游 rusqlite 0.41 接口已经适配，不重复照搬旧实现。

## 任务顺序与阶段验收

| 阶段 | 上游任务 | Studio 接入前需要看到的结果 |
| --- | --- | --- |
| 1 | [MarkNoria #3：通用修复与回归基线](https://github.com/messon007/marknoria/issues/3) | 逐项已接收/已存在/不适用的证据；保存、表格和依赖修复对应提交与用例 |
| 2A | [MarkNoria #4：共享内核接口与模块收敛](https://github.com/messon007/marknoria/issues/4) | 无 GUI 的最小宿主、明确能力/单位/预算、同源模块单一维护位置和迁移说明 |
| 2B | [MarkNoria #5：嵌入协议与 Web 组件](https://github.com/messon007/marknoria/issues/5) | 显式编辑/查询能力、版本化状态/命令/选区协议、无私有 DOM/调试全局依赖的宿主示例 |
| 3 | [MarkNoria #6：可复现交付与兼容验证](https://github.com/messon007/marknoria/issues/6) | 固定源码/资源包、manifest/哈希/许可、干净导出、共享 fixtures 和下游组合验证 |
| 独立选型 | [MarkNoria #1：高亮性能与 bincode](https://github.com/messon007/marknoria/issues/1) | 公平实测后采用性能最好的合格实现；只有 syntect 实测最优才等待其上游方案 |

实现基线按阶段 1 → 2A/2B → 3 推进，接口讨论可以并行。高亮由既有任务负责，不额外建立竞争实现。
两产品都继续独立构建；本计划不要求新建第三个仓库或立即合仓。

## Studio 的同步流程

1. 上游任务提供可采用的提交/版本与交付说明后，复查其契约、依赖、实际测试范围和仍存在的差异。
   issue 关闭是复查触发点，不是已经兼容或可以直接上线的证明。
2. 在独立工作区从明确提交/干净资源包生成同步 PR，核对源码与资产同源、hash 和许可。
   不导入 MarkNoria 活动工作树或未提交生成物，不写入其他会话的原生分支。
3. PR 列出采用的接口/模块、仍保留的差异、移除的临时补丁及宿主适配。
   已被上游吸收的修复应删除对应补丁，而不是再保留一套长期实现。
4. 运行共享语义 fixtures 与受影响的 Studio 验证：Markdown/GFM、Unicode/CRLF/source anchors、
   SQL/jq 授权和预算、编辑冲突、选区/目录/表格重挂、渲染与传输成本及 Linux/macOS/Windows CI。
   原生分支的 GTK/布局/后端回归由该分支接入时执行，不能用当前主线 CI 冒充已经验证。
5. 验证通过后合入；在 Studio #48 更新实际采用版本、已完成模块和剩余任务。
   只有目标能力实际进入 Studio 并完成对应回归，才勾选迁移完成。

Studio 发现通用缺陷时可先做有测试的紧急修复，但应关联上游 issue、记录窄范围补丁和移除条件，随后回流 MarkNoria。
AI 业务、授权和宿主策略仍在 Studio 维护，不强行上移。

## 发布与跟踪边界

MarkNoria 当前是私有仓库，Studio 是公开仓库。短期继续使用审核过、固定来源的 vendoring，
转为消费上游正式导出的库/组件；不能使普通构建或外部 PR CI 依赖私有令牌、兄弟 checkout、其 build/assets 或运行时服务。
发布包只包含允许公开分发的源码/资源与许可，不包含用户文件、私有 fixtures、截图、缓存或凭据。

本轮建立的是 GitHub 上下游任务/依赖和维护者复查流程。现有 weekly ecosystem watch 仍监控 Codex/OpenCode 与项目健康，
**没有自动读取私有 MarkNoria 的定时同步**。待上游交付 manifest 后，再按可访问的版本元数据评估只读监控；
无法访问/未运行验证必须记为未知，不能当作已经同步。

bincode 的原有维护告警继续由 MarkNoria #1 / Studio #25 跟踪；本计划不添加审计忽略项，
也不承诺未完成高亮评估前的引擎选择。
