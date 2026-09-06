import type { SelectionEnvironment } from '../ui-src/selection-coordinator.mjs'
import { completeSessionSelection } from '../ui-src/selection-coordinator.mjs'

// @ts-expect-error Revision identifiers must be strings.
const environment: SelectionEnvironment = { revision: 123 }
void environment

void completeSessionSelection({
  fresh: true, cached: true, codex: true,
  mapLoad: Promise.resolve(), environmentLoad: Promise.resolve(null),
  isCurrent: () => true, status() {}, bootstrap() {},
  resume: async options => {
    // @ts-expect-error Root is a path string, not a configured flag.
    const configured: boolean = options.environmentRoot
    void configured
  },
})
