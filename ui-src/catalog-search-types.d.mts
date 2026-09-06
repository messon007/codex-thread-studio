export interface CatalogThread {
  id: string
  cwd?: string
  name?: string
  title?: string
  preview?: string
  status?: string | { type?: string } | null
  activityAt?: unknown
  updatedAt?: unknown
  updated_at?: unknown
  createdAt?: unknown
  [field: string]: unknown
}
export interface CatalogEntry { backend: string; thread: CatalogThread }
export type BackendCatalogs = Record<string, CatalogThread[]>
export interface CatalogFilter {
  filter?: string
  search?: string
  attention?: Set<string>
  hiddenDirectories?: string[]
  ignorePatterns?: string[]
}
export interface IgnoreRule { negated: boolean; regex: RegExp }
export interface RemoteOccurrence {
  turnId?: string
  itemId?: string
  type?: 'user' | 'assistant' | 'activity'
  snippet?: string
  snippetMatchRange?: { start: number; end: number }
  [field: string]: unknown
}
export interface SessionOccurrence extends RemoteOccurrence {
  type: 'user' | 'assistant' | 'activity'
  turnIndex: number
  itemIndex: number
  source: 'local' | 'remote'
}
