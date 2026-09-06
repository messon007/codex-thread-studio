export type MapState = 'notStarted' | 'active' | 'visited' | 'done' | 'paused'
export type AssistantMapState = 'notStarted' | 'active' | 'visited'
export type MapStructure = 'hierarchy' | 'path' | 'flow' | 'blank'
export interface SessionMapItem {
  id: string; parentId: string | null; kind: string; title: string; summary: string
  state: MapState; position: number; archived: boolean; createdAt: number; updatedAt: number
}
export interface SessionMap {
  id: string; backend: string; threadId: string; goal: string; definitionOfDone: string
  structure: MapStructure; revision: number; currentItemId: string | null
  lastSyncedTurnId: string | null; items: SessionMapItem[]; relations: unknown[]
  createdAt: number; updatedAt: number
}
export interface SessionMapWorker {
  key: string; threadId: string | null; generation: number | null
  chain: Promise<unknown>; disposed: boolean
}
export type AssistantMapOperation =
  | { op: 'addItem'; itemId: string; parentId: string | null; afterItemId: string | null; title: string; kind: string; summary: string; state: AssistantMapState }
  | { op: 'updateItem'; itemId: string; title: string | null; kind: string | null; summary: string | null }
  | { op: 'setCurrent'; itemId: string | null }
  | { op: 'setState'; itemId: string; state: AssistantMapState }
export type ParsedSessionMapUpdate =
  | { found: false; visibleText: string; update: null }
  | { found: true; visibleText: string; update: { baseRevision: number; operations: unknown[] } }
export interface StructuredWorkerTurn { items?: readonly { type: string; text?: unknown }[]; error?: unknown }
