// Generated from ui-src; run npm run build:ui. Do not edit.
export function normalizeStoredTurnOptions(value) {
    const normalized = {};
    if (!value || typeof value !== 'object' || Array.isArray(value))
        return normalized;
    for (const [key, entry] of Object.entries(value).slice(0, 2048)) {
        if (typeof key !== 'string' || !key.includes(':') || key.length > 320
            || !isRecord(entry))
            continue;
        const model = String(entry.model || '').trim().slice(0, 256);
        const effort = String(entry.effort || '').trim().slice(0, 64);
        if (!model && !effort)
            continue;
        if (/\p{Cc}/u.test(model) || /\p{Cc}/u.test(effort))
            continue;
        normalized[key] = {
            ...(model ? { model } : {}),
            ...(effort ? { effort } : {}),
        };
    }
    return normalized;
}
export function sessionModelPreferencePayload(sessionKey, options = {}) {
    return {
        sessionKey,
        model: String(options.model || ''),
        effort: String(options.effort || ''),
    };
}
/** Read at dispatch time, not enqueue time. Preserve transient turn options too. */
export function copySessionTurnOptions(stored, key, defaults) {
    return { ...(stored[key] || defaults) };
}
function isRecord(value) {
    return !!value && typeof value === 'object' && !Array.isArray(value);
}
