// Generated from ui-src; run npm run build:ui. Do not edit.
import { codexLifecycleStreamMessage } from './codex-lifecycle-diagnostics.mjs';
export function waitForEventStream(state, timeoutMs = 1_500) {
    if (state.ready)
        return Promise.resolve(true);
    return new Promise(resolve => {
        let settled = false;
        const finish = (ready) => {
            if (settled)
                return;
            settled = true;
            clearTimeout(timer);
            state.waiters.delete(finish);
            resolve(ready);
        };
        const timer = setTimeout(() => finish(false), timeoutMs);
        state.waiters.add(finish);
    });
}
export function connectEventStream(state, effects) {
    if (state.stream)
        return state.stream;
    const events = effects.open();
    state.stream = events;
    events.onopen = () => {
        const needsReconciliation = state.openCount > 0 || state.missedBarrier;
        state.openCount += 1;
        state.ready = true;
        state.missedBarrier = false;
        for (const waiter of [...state.waiters])
            waiter(true);
        effects.connected(needsReconciliation);
    };
    events.onmessage = event => {
        try {
            effects.message(JSON.parse(event.data));
        }
        catch (error) {
            effects.invalid(error, event.data);
        }
    };
    events.onerror = () => {
        state.ready = false;
        effects.disconnected();
    };
    return events;
}
export function connectLifecycleStream(state, effects) {
    if (state.reconnectTimer !== null)
        clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;
    if (state.socket) {
        state.socket.onclose = null;
        state.socket.close();
    }
    state.ready = false;
    state.generation += 1;
    const generation = state.generation;
    const socket = effects.open();
    state.socket = socket;
    return new Promise((resolve) => {
        let settled = false;
        const settle = (ready) => {
            if (settled)
                return;
            settled = true;
            clearTimeout(barrierTimer);
            resolve(ready);
        };
        // This is only an initialization ordering barrier. The connection stays
        // alive and can become authoritative after a slow local WebSocket upgrade.
        const barrierTimer = setTimeout(() => settle(false), 750);
        socket.onmessage = (event) => {
            if (generation !== state.generation)
                return;
            let envelope;
            try {
                envelope = codexLifecycleStreamMessage(JSON.parse(event.data));
            }
            catch (error) {
                effects.invalid(error, event.data);
                return;
            }
            if (!envelope)
                return;
            if (envelope.type === 'ready') {
                const reconnected = state.openCount > 0;
                state.openCount += 1;
                state.ready = true;
                settle(true);
                effects.report({
                    state: reconnected ? 'reconnected' : 'connected',
                    backendCount: envelope.backends.length,
                });
                if (reconnected)
                    effects.reconcile();
                return;
            }
            if (!effects.supports(envelope.backend))
                return;
            if (envelope.message.method === 'studio/appServer/status') {
                effects.status(envelope.backend, envelope.message);
            }
            else if (envelope.message.method === 'studio/appServer/lagged') {
                effects.lag(envelope.backend, envelope.message);
            }
            else {
                effects.notification(envelope.backend, envelope.message);
            }
        };
        socket.onerror = () => {
            if (generation !== state.generation)
                return;
            state.ready = false;
        };
        socket.onclose = () => {
            if (generation !== state.generation)
                return;
            state.ready = false;
            state.socket = null;
            settle(false);
            effects.report({ state: 'disconnected' });
            state.reconnectTimer = setTimeout(() => {
                if (generation !== state.generation)
                    return;
                connectLifecycleStream(state, effects);
            }, 1_800);
        };
    });
}
