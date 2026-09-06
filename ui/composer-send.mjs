// Generated from ui-src; run npm run build:ui. Do not edit.
/** Keep transport acceptance distinct from local rendering/persistence failures. */
export async function executeComposerSend(effects) {
    let accepted = false;
    try {
        const result = await effects.send();
        accepted = true;
        effects.acknowledged(result);
    }
    catch (error) {
        effects.failed(error, accepted);
    }
    finally {
        effects.finished();
    }
    return accepted;
}
