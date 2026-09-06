export interface OutlineTarget { kind?: string; href?: unknown; page?: unknown; offset?: number; line?: number; destination?: unknown }
export interface OutlineItem { id: string; label: string; depth: number; target: OutlineTarget | null; parentId?: string }
export interface InlineToken { type?: string; raw?: string; text?: string; depth?: number; tokens?: InlineToken[] }
export interface PdfOutlineEntry { title?: string; dest?: unknown; items?: PdfOutlineEntry[] }
export interface PdfOutlineDocument {
  getOutline(): Promise<PdfOutlineEntry[] | null>
  getDestination(name: string): Promise<unknown>
  getPageIndex(ref: unknown): Promise<number>
}
