import { emptyBackendCatalogs } from '../ui-src/backends.mjs'
import { filterCatalogEntries, partitionPinnedCatalogEntries } from '../ui-src/thread-catalog.mjs'
import { localSessionOccurrences, mergeSessionOccurrences } from '../ui-src/session-search.mjs'
import { createCodexViewModel } from '../ui-src/codex-native.mjs'
import { parseEnvironmentLines } from '../ui-src/environment-profile.mjs'
const entries = filterCatalogEntries(emptyBackendCatalogs(), { filter: 'attention', attention: new Set() })
const pinned = partitionPinnedCatalogEntries(entries, new Set(), { order: 'activity' })
const id: string | undefined = pinned.pinnedEntries[0]?.thread.id
void id
// @ts-expect-error Sort policies are explicit, not arbitrary strings.
partitionPinnedCatalogEntries(entries, new Set(), { order: 'random' })
const occurrences = localSessionOccurrences(createCodexViewModel(), 'query')
const merged = mergeSessionOccurrences([], occurrences)
const turnIndex: number | undefined = merged[0]?.turnIndex
void turnIndex
// @ts-expect-error Local occurrence positions remain numeric.
const invalid: string | undefined = merged[0]?.turnIndex
void invalid
const env: Record<string, string> = parseEnvironmentLines('NAME=value')
void env
