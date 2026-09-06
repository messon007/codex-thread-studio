// Generated from ui-src; run npm run build:ui. Do not edit.
export async function executeQueuedMessage(state, key, effects) {
    const message = state.messageQueues[key]?.[0];
    if (!message || state.runningMessageQueues.has(key) || state.pausedMessageQueues.has(key))
        return false;
    state.runningMessageQueues.add(key);
    state.messageQueueErrors.delete(key);
    let accepted = false;
    try {
        const result = await effects.send(message);
        accepted = true;
        effects.acknowledged(result);
        const current = state.messageQueues[key] || [];
        if (current[0]?.id === message.id)
            current.shift();
        if (!current.length) {
            delete state.messageQueues[key];
            state.pausedMessageQueues.delete(key);
        }
        await effects.persist();
        return true;
    }
    catch (error) {
        if (accepted) {
            const current = state.messageQueues[key] || (state.messageQueues[key] = []);
            if (!current.some(entry => entry.id === message.id))
                current.unshift(message);
        }
        state.pausedMessageQueues.add(key);
        state.messageQueueErrors.set(key, accepted ? effects.acceptedPersistenceError : error instanceof Error ? error.message : String(error));
        effects.failed(error, accepted);
        throw error;
    }
    finally {
        state.runningMessageQueues.delete(key);
    }
}
