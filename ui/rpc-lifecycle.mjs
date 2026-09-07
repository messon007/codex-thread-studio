// Generated from ui-src; run npm run build:ui. Do not edit.
const record = (value) => value !== null && typeof value === 'object' ? value : {};
export class PendingRpcRequests {
    pending;
    constructor(pending) {
        this.pending = pending;
    }
    request(id, method, send, timeoutMs, timeoutMessage) {
        const key = String(id);
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => { this.pending.delete(key); reject(new Error(timeoutMessage)); }, timeoutMs);
            this.pending.set(key, { resolve, reject, timer, method });
            try {
                send();
            }
            catch (error) {
                clearTimeout(timer);
                this.pending.delete(key);
                reject(error);
            }
        });
    }
    settle(value) {
        const message = record(value);
        const key = String(message.id);
        const pending = this.pending.get(key);
        if (!pending)
            return false;
        this.pending.delete(key);
        clearTimeout(pending.timer);
        if (message.error)
            pending.reject(new Error(String(record(message.error).message || JSON.stringify(message.error))));
        else
            pending.resolve(message.result);
        return true;
    }
    rejectAll(error) {
        for (const pending of this.pending.values()) {
            clearTimeout(pending.timer);
            pending.reject(error);
        }
        this.pending.clear();
    }
}
export function requestSocketRpc(options) {
    const { socket, id, method, params } = options;
    return new Promise((resolve, reject) => {
        let requested = false;
        let settled = false;
        const finish = (error, value) => {
            if (settled)
                return;
            settled = true;
            clearTimeout(timer);
            socket.onclose = null;
            socket.close();
            if (error)
                reject(error);
            else
                resolve(value);
        };
        const timer = setTimeout(() => finish(new Error(options.timeoutMessage)), options.timeoutMs);
        socket.onmessage = event => {
            if (settled)
                return;
            let message;
            try {
                message = record(JSON.parse(event.data));
            }
            catch {
                return;
            }
            const status = record(message.params);
            if (message.method === 'studio/appServer/status' && status.state === 'error') {
                finish(new Error(String(status.message || options.unavailableMessage)));
            }
            else if (message.method === 'studio/appServer/status' && status.state === 'ready' && !requested) {
                requested = true;
                try {
                    socket.send(JSON.stringify({ id, method, params }));
                }
                catch (error) {
                    finish(error instanceof Error ? error : new Error(String(error)));
                }
            }
            else if (message.id === id) {
                if (message.error)
                    finish(new Error(options.responseError(message.error)));
                else
                    finish(null, message.result);
            }
        };
        socket.onerror = () => finish(new Error(options.connectMessage));
        socket.onclose = () => finish(new Error(options.closeMessage));
    });
}
export function connectSelectedSocket(state, effects) {
    if (state.reconnectTimer != null)
        clearTimeout(state.reconnectTimer);
    effects.cleanup();
    state.ready = false;
    const generation = ++state.socketGeneration;
    effects.starting();
    const socket = effects.open();
    state.socket = socket;
    socket.onmessage = event => { if (generation === state.socketGeneration)
        effects.message(event.data); };
    socket.onerror = () => {
        if (generation !== state.socketGeneration)
            return;
        state.ready = false;
        effects.error();
    };
    socket.onclose = () => {
        if (generation !== state.socketGeneration)
            return;
        state.ready = false;
        effects.closed();
        state.reconnectTimer = setTimeout(effects.reconnect, 1800);
    };
}
