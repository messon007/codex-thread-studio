/** Untrusted payload fields are narrowed before use; unknown future statuses are retained. */
export interface LifecycleEvent {
  method: string
  threadId: string
  turnId: string
  notificationStatus: string
}
export type LifecycleStreamMessage =
  | { type: 'ready'; backends: string[] }
  | { type: 'event'; backend: string; message: Record<string, unknown> }
