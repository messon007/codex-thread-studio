# Comment 架构

## 目标

Comment 是长期稳定的“摘录 → 意见 → 草稿 → 输入框”能力。核心不解释摘录来自聊天、文件、网页、电子书或未来的新阅读器，也不依赖 Browser Workspace、文档预览器或具体后端。

## 模块边界

```text
Comment Core
├── CommentDraft / SourceReference
├── 草稿校验、迁移和持久化模型
└── Source Provider Registry
       ├── Chat Provider
       ├── Document Provider
       ├── Browser Provider（可选）
       └── EPUB Provider（未来、可选）
```

`ui/comment-core.mjs` 只能处理以下通用字段：

```json
{
  "id": "uuid",
  "excerpt": "保存下来的内容快照",
  "note": "用户意见，可以为空",
  "createdAt": "ISO-8601",
  "source": {
    "provider": "opaque-provider-id",
    "version": 1,
    "anchor": {}
  }
}
```

核心把 `provider` 和 `anchor` 当成不透明数据。来源模块独立负责：

- 创建和规范化 anchor；
- 在界面显示简短来源；
- 生成提供给模型的定位文字及可选指令；
- 返回原位置；
- 来源升级时迁移自己的 anchor schema。

来源模块不能修改 Comment Core 的数据生命周期；核心也不能对 provider ID 使用 `if/switch`。

## 可删除性要求

Browser Comment 必须位于独立模块，并且只通过一条 provider 注册语句接入。删除浏览器场景时：

1. 删除 Browser Provider 模块；
2. 删除注册语句和浏览器侧的 capture 入口；
3. Comment Core、Chat、Document、EPUB、草稿存储和提示词组装均不修改。

已经保存的 Browser Comment 仍保留 `excerpt` 和 `note`。当对应 provider 不存在时，界面使用通用来源标签，允许阅读、删除和加入输入，只禁用“返回来源”。

## EPUB 扩展

EPUB Provider 建议在 anchor 中保存书籍稳定 ID、spine href、EPUB CFI、章节名和文本 quote selector。Comment Core 只持久化该对象，不理解章节或 CFI。返回原文时优先使用 CFI，失败后由 EPUB Provider 使用文本快照和前后文重新定位。

## 验收约束

- Comment Core 的源码和测试不能导入 Browser、Document、Chat 或 EPUB 实现。
- 未注册来源的批注仍可读取并加入输入框。
- `note` 为空时仍可把摘录加入草稿。
- 每个来源适配器可以独立测试和删除。
- 旧版 `quote/comment/target` 数据启动时迁移为稳定模型，不丢失内容。
