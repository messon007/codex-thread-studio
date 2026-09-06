import type { EnvironmentApplyOptions } from '../ui-src/environment-application.mjs'

function check(options: EnvironmentApplyOptions) {
  // @ts-expect-error Revision identifies a saved profile; it is not a counter.
  options.profileRevision = 2
  // @ts-expect-error Force is an explicit boolean.
  options.force = 'true'
  // @ts-expect-error Selected thread ID must be a string or null.
  options.threadId = 123
}
void check
