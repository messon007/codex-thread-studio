export interface ResourceItem { id?: unknown; type?: string; text?: unknown; content?: unknown; completedAt?: unknown; createdAt?: unknown; updatedAt?: unknown }
export interface ResourceTurn { id?: unknown; items?: ResourceItem[] }
export interface ResourceModel { turns?: ResourceTurn[] }
export interface ResourceContext { backend?: string; threadId?: string; root?: string; sharedDocumentDirectories?: string[] }
export interface ResourceSource extends ResourceContext { turnId?: string; itemId?: string; itemType?: string; item?: ResourceItem; text?: string }
export interface ResourceCandidate { raw: string; kind?: string; providerId?: string; priority?: number; field?: string; start?: number; end?: number; confidence?: number }
export interface RankedCandidate extends ResourceCandidate { priority: number; providerId: string }
export interface ResourceExtractor { id: string; priority?: number; supports?: (source: ResourceSource, context: ResourceContext) => boolean; extract: (source: ResourceSource, context: ResourceContext) => ResourceCandidate[] }
export interface SessionResource {
  schemaVersion: number; id: string; kind: string; raw: string; canonical: string; display: string; providerId: string; priority: number; confidence: number
  state: string; reason: string; target: { url?: string; backend?: string; workspaceRoot?: string; path?: string; line?: number; column?: number }
  firstSeenAt: string; lastSeenAt: string
}
export interface IndexedResource extends SessionResource { firstSeenOrder: number; lastSeenOrder: number }
export interface ResourceOccurrence { id: string; resourceId: string; threadKey: string; turnId: string | undefined; itemId: string | undefined; itemType: string | undefined; field: string; start?: number; end?: number; excerpt: string; observedOrder: number; observedAt: string }
