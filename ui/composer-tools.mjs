export const SLASH_COMMANDS = Object.freeze([
  { name: 'model', description: 'Select the model and reasoning effort', action: 'model' },
  { name: 'permissions', description: 'Set approval and sandbox policies for subsequent turns', action: 'permissions' },
  { name: 'status', description: 'View the current session, model, and token status', action: 'status' },
  { name: 'compact', description: 'Compact the current session context', action: 'compact' },
  { name: 'review', description: 'Review current uncommitted code changes', action: 'review' },
  { name: 'diff', description: 'View the aggregate Git diff for the current turn', action: 'diff' },
  { name: 'skills', description: 'Browse and reference skills discovered by app-server', action: 'skills' },
  { name: 'mcp', description: 'View MCP servers, tools, and authentication status', action: 'mcp' },
  { name: 'rename', description: 'Rename the current session', action: 'rename' },
  { name: 'fork', description: 'Fork the current session', action: 'fork' },
  { name: 'new', description: 'Create a new Codex session', action: 'new' },
  { name: 'copy', description: 'Copy the latest Codex response', action: 'copy' },
  { name: 'archive', description: 'Archive the current session', action: 'archive' },
  { name: 'delete', description: 'Permanently delete the current session', action: 'delete' },
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

export function createComposerDraftStore() {
  const drafts = new Map()
  let visibleKey = null

  function write(key, value) {
    const normalizedKey = String(key || '')
    if (!normalizedKey) return
    const text = String(value || '')
    if (text) drafts.set(normalizedKey, text)
    else drafts.delete(normalizedKey)
  }

  return {
    switchTo(key, currentValue = '') {
      const nextKey = String(key || '')
      if (visibleKey === nextKey) return String(currentValue || '')
      write(visibleKey, currentValue)
      visibleKey = nextKey
      return drafts.get(nextKey) || ''
    },
    update(key, value) {
      write(key, value)
    },
    value(key) {
      return drafts.get(String(key || '')) || ''
    },
    discard(key) {
      const normalizedKey = String(key || '')
      drafts.delete(normalizedKey)
      if (visibleKey === normalizedKey) visibleKey = null
    },
  }
}

const PREVIEWABLE_TEXT_EXTENSIONS = new Set([
  'adoc', 'asciidoc', 'bash', 'bat', 'c', 'cc', 'cfg', 'cjs', 'cmake', 'conf', 'cpp', 'cs', 'css',
  'csv', 'cxx', 'd2', 'diff', 'dist', 'editorconfig', 'env', 'example', 'fish', 'gitattributes', 'gitignore', 'go', 'gql', 'gradle', 'graphql',
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

const KNOWN_BINARY_EXTENSIONS = new Set([
  '7z', 'a', 'apk', 'avi', 'avif', 'bin', 'bmp', 'bz2', 'class', 'db', 'deb', 'dll', 'dmg',
  'doc', 'docx', 'eot', 'exe', 'flac', 'gz', 'ico', 'iso', 'jar', 'mov', 'mp3', 'mp4', 'o',
  'obj', 'ogg', 'otf', 'parquet', 'ppt', 'pptx', 'rar', 'rpm', 'so', 'sqlite', 'sqlite3',
  'tar', 'tif', 'tiff', 'ttf', 'wav', 'webm', 'woff', 'woff2', 'xls', 'xz', 'zip', 'zst',
])

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
  if (/\.epub$/iu.test(fuzzyFileLabel(file))) return 'epub'
  if (/\.pdf$/iu.test(fuzzyFileLabel(file))) return 'pdf'
  if (/\.xlsx$/iu.test(fuzzyFileLabel(file))) return 'table'
  if (/\.(csv|tsv)$/iu.test(fuzzyFileLabel(file))) return 'table'
  if (isPreviewableImageFile(file)) return 'image'
  if (isPreviewableTextFile(file)) return 'text'
  return null
}

export function reviewableFileKind(file) {
  const kind = previewableFileKind(file)
  if (kind) return kind
  const path = fuzzyFileLabel(file).replaceAll('\\', '/')
  const name = path.split('/').filter(Boolean).pop()?.toLowerCase() || ''
  if (!name) return null
  const extension = name.includes('.') ? name.split('.').pop() : ''
  return KNOWN_BINARY_EXTENSIONS.has(extension) ? null : 'text'
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
