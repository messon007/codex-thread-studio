import type { SubmissionServices, SubmissionState } from '../ui-src/submission-controller.mjs'
import type { ManagementServices } from '../ui-src/session-operations.mjs'
import type { BackgroundServices, BackgroundState } from '../ui-src/background-sessions.mjs'

function check(send: SubmissionServices, state: SubmissionState, manage: ManagementServices, background: BackgroundServices, lifecycle: BackgroundState) {
  // @ts-expect-error Sending requires a backend-qualified session reference.
  send.prepareComposerTurn({ id: 'a' })
  // @ts-expect-error Pending images cannot be plain path strings.
  state.pendingImages.a = ['/tmp/image.png']
  // @ts-expect-error Created sessions retain structured thread metadata.
  manage.rememberStartedThread('codex', 'a', 'new')
  // @ts-expect-error A queue completion needs its owning backend.
  background.handleQueuedTurnCompletion({ id: 'a' }, 'completed')
  // @ts-expect-error Cache validation time is a numeric timestamp.
  lifecycle.threadModels.set('codex:a', { model: lifecycle.model, validatedAt: 'now' })
  lifecycle.appServerGenerations.codex = null
}
void check
