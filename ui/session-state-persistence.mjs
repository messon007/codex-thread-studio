// Generated from ui-src; run npm run build:ui. Do not edit.
import { sessionModelPreferencePayload } from './session-model-preferences.mjs';
/** The writer serializes synchronously; later state changes cannot alter a queued write. */
export function createSessionStatePersistence(state, writer, ready) {
    function write(key, path, body, method = 'PUT') {
        if (!key || !ready())
            return Promise.resolve();
        return writer.write(path, body, method);
    }
    return {
        annotations(key) {
            return write(key, '/studio/session-state/annotations', {
                sessionKey: key, drafts: state.annotationDrafts[key] || [], additional: state.annotationAdditional[key] || '',
            });
        },
        openingMessage(key) {
            return write(key, '/studio/session-state/opening-message', { sessionKey: key, message: state.openingMessages[key] || null });
        },
        turnOptions(key) {
            return write(key, '/studio/session-state/turn-options', sessionModelPreferencePayload(key, state.turnOptions[key] || {}));
        },
        messageQueue(key) {
            return write(key, '/studio/session-state/message-queue', { sessionKey: key, messages: state.messageQueues[key] || [] });
        },
        remove(key) {
            return write(key, '/studio/session-state/session', { sessionKey: key }, 'DELETE');
        },
        pin(key, pinned) {
            return write(key, '/studio/session-state/pin', { sessionKey: key, pinned });
        },
    };
}
