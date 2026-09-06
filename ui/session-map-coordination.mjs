// Generated from ui-src; run npm run build:ui. Do not edit.
import { normalizeSessionMap } from './session-map.mjs';
/** Per-session coordination; selection is only consulted by the injected renderer. */
export class SessionMapCoordinator {
    state;
    changed;
    lifetimes = new Map();
    syncs = new Map();
    removals = new Set();
    constructor(state, changed) {
        this.state = state;
        this.changed = changed;
    }
    current(key) { return this.state.sessionMaps.get(key) || null; }
    guard(key) {
        const lifetime = this.lifetime(key);
        const id = this.current(key)?.id;
        return () => this.lifetime(key) === lifetime && this.current(key)?.id === id;
    }
    lifetime(key) {
        let token = this.lifetimes.get(key);
        if (!token) {
            token = {};
            this.lifetimes.set(key, token);
        }
        return token;
    }
    publish(key, map, notify = true) {
        if (this.current(key)?.id !== map?.id)
            this.lifetimes.set(key, {});
        // A write is newer evidence than any already-started GET.
        this.state.sessionMapLoads.delete(key);
        this.state.sessionMaps.set(key, map);
        if (notify)
            this.changed(key);
        return map;
    }
    load(key, fetch, force = false) {
        if (!key)
            return Promise.resolve(null);
        if (!force && this.state.sessionMaps.has(key))
            return Promise.resolve(this.current(key));
        const existing = this.state.sessionMapLoads.get(key);
        if (existing)
            return existing;
        const load = Promise.resolve().then(fetch)
            .then(normalizeSessionMap)
            .catch((error) => {
            if (typeof error === 'object' && error !== null && 'status' in error && error.status === 404)
                return null;
            if (this.state.sessionMapLoads.get(key) !== load)
                return this.current(key);
            throw error;
        })
            .then(map => this.state.sessionMapLoads.get(key) === load ? this.publish(key, map) : this.current(key))
            .finally(() => { if (this.state.sessionMapLoads.get(key) === load)
            this.state.sessionMapLoads.delete(key); });
        this.state.sessionMapLoads.set(key, load);
        return load;
    }
    async update(key, execute) {
        const map = this.current(key);
        if (!map)
            return null;
        const lifetime = this.lifetime(key);
        const updated = normalizeSessionMap(await execute(map));
        const current = this.current(key);
        if (this.lifetime(key) !== lifetime || current?.id !== map.id)
            return current;
        if (updated && current && updated.id === current.id && updated.revision < current.revision)
            return current;
        return this.publish(key, updated);
    }
    async remove(key, execute) {
        const map = this.current(key);
        if (!map || this.removals.has(key))
            return false;
        const lifetime = this.lifetime(key);
        this.removals.add(key);
        try {
            await execute(map);
            if (this.lifetime(key) !== lifetime || this.current(key)?.id !== map.id)
                return false;
            this.publish(key, null, false);
            this.state.sessionMapSync.delete(key);
            return true;
        }
        finally {
            this.removals.delete(key);
        }
    }
    synchronize(key, execute, messages) {
        const map = this.current(key);
        if (!map)
            return Promise.resolve(null);
        const lifetime = this.lifetime(key);
        const existing = this.syncs.get(key);
        if (existing?.lifetime === lifetime)
            return existing.promise;
        const isCurrent = this.guard(key);
        this.state.sessionMapSync.set(key, { state: 'syncing', message: messages.syncing });
        this.changed(key);
        const promise = Promise.resolve().then(() => isCurrent() ? execute(map, isCurrent) : this.current(key))
            .then(updated => {
            if (!isCurrent())
                return this.current(key);
            this.state.sessionMapSync.set(key, { state: 'synced', message: messages.synced });
            this.changed(key);
            return updated;
        })
            .catch((error) => {
            if (!isCurrent())
                return this.current(key);
            this.state.sessionMapSync.set(key, { state: 'error', message: error instanceof Error ? error.message : String(error) });
            this.changed(key);
            throw error;
        })
            .finally(() => { if (this.syncs.get(key)?.promise === promise)
            this.syncs.delete(key); });
        this.syncs.set(key, { lifetime, promise });
        return promise;
    }
}
