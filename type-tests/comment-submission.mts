import { CommentSubmissionCoordinator } from '../ui-src/comment-submission.mjs'
const coordinator = new CommentSubmissionCoordinator()
coordinator.submit({
  key: 'codex:a', store: { annotationDrafts: {}, annotationAdditional: {} },
  prompt: '', composerText: '', insert() {}, persist() {},
  // @ts-expect-error A DOM event dispatch is not a send acknowledgement.
  send: () => {},
})
