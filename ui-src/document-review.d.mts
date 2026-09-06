// Contract for the existing JavaScript document helpers, not a checked implementation.
import type { FileRangeTarget } from './comment-types.mjs'
export function fileDisplayName(path: unknown): string
export function normalizeAnnotationTarget(draft?: unknown): FileRangeTarget | { kind: 'chatRange'; itemId: string | null; turnId: string | null }
export function createFileRangeTarget(file: unknown, quote: unknown, hintOffset?: number): FileRangeTarget & { ambiguous: boolean }
export function lineRangeForTarget(target: unknown, content?: string | null): { startLine: number | null; endLine: number | null }
export function fileAnnotationAnchor(target: unknown, content?: string | null): string
