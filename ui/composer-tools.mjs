export const SLASH_COMMANDS = Object.freeze([
  { name: 'model', description: '选择模型和推理强度', action: 'model' },
  { name: 'permissions', description: '设置后续 Turn 的审批和沙箱策略', action: 'permissions' },
  { name: 'status', description: '查看当前会话、模型和 Token 状态', action: 'status' },
  { name: 'compact', description: '压缩当前会话上下文', action: 'compact' },
  { name: 'review', description: '审查当前未提交的代码修改', action: 'review' },
  { name: 'diff', description: '查看当前 Turn 的聚合 Git diff', action: 'diff' },
  { name: 'skills', description: '浏览并引用 app-server 发现的技能', action: 'skills' },
  { name: 'mcp', description: '查看 MCP Server、工具和认证状态', action: 'mcp' },
  { name: 'rename', description: '重命名当前会话', action: 'rename' },
  { name: 'fork', description: 'Fork 当前会话', action: 'fork' },
  { name: 'new', description: '创建新的 Codex 会话', action: 'new' },
  { name: 'copy', description: '复制最近一条 Codex 回复', action: 'copy' },
  { name: 'archive', description: '归档当前会话', action: 'archive' },
  { name: 'delete', description: '永久删除当前会话', action: 'delete' },
])

export function composerTrigger(value, cursor = value.length) {
  const before = String(value || '').slice(0, cursor)
  const slash = before.match(/^\s*\/([\p{L}\p{N}_-]*)$/u)
  if (slash) {
    const slashIndex = before.indexOf('/')
    return { type: 'slash', query: slash[1].toLowerCase(), start: slashIndex, end: cursor }
  }

  const skill = before.match(/(?:^|\s)\$([^\s$]*)$/u)
  if (skill) {
    const start = before.lastIndexOf('$')
    return { type: 'skill', query: skill[1].toLowerCase(), start, end: cursor }
  }

  const mention = before.match(/(?:^|\s)@([^\s@]*)$/u)
  if (!mention) return null
  const start = before.lastIndexOf('@')
  return { type: 'file', query: mention[1], start, end: cursor }
}

export function shellCommandFromComposer(value) {
  const match = String(value || '').match(/^\s*!(.*)$/su)
  return match ? match[1].trim() : null
}

export function matchingSlashCommands(query, commands = SLASH_COMMANDS) {
  const needle = String(query || '').toLowerCase()
  return commands
    .filter((command) => !needle || command.name.startsWith(needle) || command.description.toLowerCase().includes(needle))
    .slice(0, 12)
}

export function replaceComposerTrigger(value, trigger, replacement) {
  if (!trigger) return { value, cursor: String(value || '').length }
  const source = String(value || '')
  const inserted = String(replacement || '')
  const next = `${source.slice(0, trigger.start)}${inserted}${source.slice(trigger.end)}`
  return { value: next, cursor: trigger.start + inserted.length }
}

export function fuzzyFileLabel(file) {
  if (!file) return ''
  return String(file.path || file.file_name || '')
}

const PREVIEWABLE_TEXT_EXTENSIONS = new Set([
  'adoc', 'asciidoc', 'bash', 'bat', 'c', 'cc', 'cfg', 'cjs', 'cmake', 'conf', 'cpp', 'cs', 'css',
  'csv', 'cxx', 'diff', 'dist', 'editorconfig', 'env', 'example', 'fish', 'gitattributes', 'gitignore', 'go', 'gql', 'gradle', 'graphql',
  'groovy', 'h', 'hpp', 'htm', 'html', 'ini', 'java', 'js', 'json', 'json5', 'jsonc', 'jsonl', 'jsx',
  'in', 'kt', 'kts', 'less', 'lock', 'log', 'lua', 'md', 'mdown', 'mjs', 'mk', 'mkd', 'ndjson', 'npmrc', 'nvmrc', 'patch',
  'markdown', 'php', 'properties', 'proto', 'ps1', 'py', 'pyi', 'rb', 'rs', 'rst', 'sass', 'scss', 'sh', 'sql',
  'sample', 'swift', 'template', 'text', 'tmpl', 'toml', 'ts', 'tsv', 'tsx', 'txt', 'xhtml', 'xml', 'yaml', 'yml', 'zsh',
])

const PREVIEWABLE_TEXT_NAMES = new Set([
  'authors', 'changelog', 'cmakelists.txt', 'code_of_conduct', 'containerfile', 'contributing', 'copying',
  'dockerfile', 'gemfile', 'jenkinsfile', 'license', 'makefile', 'notice', 'procfile', 'rakefile', 'readme',
  'security', 'vagrantfile',
])

const PREVIEWABLE_IMAGE_EXTENSIONS = new Set(['gif', 'jpeg', 'jpg', 'png', 'svg', 'webp'])

export function isPreviewableTextFile(file) {
  const path = fuzzyFileLabel(file).replaceAll('\\', '/')
  const name = path.split('/').filter(Boolean).pop()?.toLowerCase() || ''
  if (!name) return false
  const stem = name.replace(/\.[^.]+$/u, '')
  if (PREVIEWABLE_TEXT_NAMES.has(name) || PREVIEWABLE_TEXT_NAMES.has(stem)) return true
  const extension = name.includes('.') ? name.split('.').pop() : ''
  return PREVIEWABLE_TEXT_EXTENSIONS.has(extension)
}

export function isPreviewableImageFile(file) {
  const path = fuzzyFileLabel(file).replaceAll('\\', '/')
  const name = path.split('/').filter(Boolean).pop()?.toLowerCase() || ''
  const extension = name.includes('.') ? name.split('.').pop() : ''
  return PREVIEWABLE_IMAGE_EXTENSIONS.has(extension)
}

export function previewableFileKind(file) {
  if (isPreviewableImageFile(file)) return 'image'
  if (isPreviewableTextFile(file)) return 'text'
  return null
}

export function selectedFileReference(file) {
  const path = fuzzyFileLabel(file)
  if (!path) return ''
  const needsQuotes = /\s/u.test(path) && !path.includes('"')
  return `${needsQuotes ? `"${path}"` : path} `
}

export function matchingSkills(query, skills = []) {
  const needle = String(query || '').toLowerCase()
  return skills
    .filter((skill) => skill?.enabled !== false)
    .filter((skill) => {
      if (!needle) return true
      return [skill.name, skill.description, skill.shortDescription, skill.interface?.displayName]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(needle))
    })
    .slice(0, 30)
}

export function selectedSkillReference(skill) {
  return skill?.name ? `$${skill.name} ` : ''
}

export function transcriptUpdateKind(method) {
  if (method?.toLowerCase().endsWith('delta')) return 'stream'
  if (method === 'item/completed') return 'item'
  if (method === 'thread/tokenUsage/updated' || method === 'turn/diff/updated' || method === 'thread/status/changed') return 'metadata'
  return 'full'
}
