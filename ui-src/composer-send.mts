/** Keep transport acceptance distinct from local rendering/persistence failures. */
export async function executeComposerSend<T>(effects: {
  send: () => Promise<T>
  acknowledged: (result: T) => void
  failed: (error: unknown, accepted: boolean) => void
  finished: () => void
}): Promise<boolean> {
  let accepted = false
  try {
    const result = await effects.send()
    accepted = true
    effects.acknowledged(result)
  } catch (error) {
    effects.failed(error, accepted)
  } finally {
    effects.finished()
  }
  return accepted
}
