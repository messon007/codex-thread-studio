// Generated from ui-src; run npm run build:ui. Do not edit.
/** Polls the existing source (in-memory notifications or remote history), never both. */
export async function waitForUtilityResult(options) {
    const now = options.now || Date.now;
    const sleep = options.sleep || ((milliseconds) => new Promise(resolve => setTimeout(resolve, milliseconds)));
    const deadline = now() + options.timeoutMs;
    while (now() < deadline) {
        options.ensureCurrent();
        const result = await options.read();
        options.ensureCurrent();
        if (result.status === 'completed')
            return result;
        if (result.status === 'failed')
            throw new Error(options.errorMessage(result.error));
        await sleep(options.intervalMs);
    }
    throw new Error(options.timeoutMessage);
}
