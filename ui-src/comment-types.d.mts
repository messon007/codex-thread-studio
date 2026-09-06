export type CommentRecord = Record<string, unknown>
export interface CommentSource { provider: string; version: number; anchor: CommentRecord }
export interface CommentDraft { id: string; excerpt: string; note: string; createdAt: string; source: CommentSource }
export interface CommentContext {
  index?: number
  unknownLabel?: string
  translate?: (message: string, values?: Record<string, string | number>) => string
  contentForSource?: (source: CommentSource) => string | null | undefined
  openDocument?: (target: CommentRecord, excerpt: string) => unknown
  openWebSource?: (url: string) => unknown
  openPdfSource?: (anchor: PdfAnchor, excerpt: string) => unknown
  openEpubSource?: (anchor: EpubAnchor, excerpt: string) => unknown
  openTableSource?: (anchor: TableAnchor, excerpt: string) => unknown
}
export interface CommentProvider {
  id: string
  normalizeAnchor?: (anchor: unknown) => CommentRecord
  describe?: (draft: CommentDraft, context: CommentContext) => string
  promptAnchor?: (draft: CommentDraft, context: CommentContext) => string
  promptInstruction?: (draft: CommentDraft, context: CommentContext) => string
  reopen?: (draft: CommentDraft, context: CommentContext) => unknown
}
export interface FileRangeTarget {
  kind: 'fileRange'
  filePath: string
  root: string
  baseHash: string
  startOffset: number | null
  endOffset: number | null
  startLine: number | null
  endLine: number | null
  prefix: string
  suffix: string
}
export interface PdfAnchor extends CommentRecord {
  root: string; filePath: string; documentHash: string; page: number
  rects: { x: number; y: number; width: number; height: number }[]
}
export interface EpubAnchor extends CommentRecord {
  root: string; filePath: string; bookHash: string; cfiRange: string; href: string; chapterLabel: string
}
export interface TableAnchor extends CommentRecord {
  root: string; filePath: string; documentHash: string; sheet: string; range: string
}
