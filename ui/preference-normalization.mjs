// Generated from ui-src; run npm run build:ui. Do not edit.
import { BACKEND_IDS, isSupportedBackend, isCodexBackend } from './backends.mjs';
const record = (value) => value !== null && typeof value === 'object' ? value : {};
export function normalizeTranslationPreferences(value) {
    const source = record(value);
    const engine = source.engine === 'ollama' ? 'ollama' : 'backend';
    const candidateOllamaModel = String(source.ollamaModel || '').trim();
    const ollamaModel = candidateOllamaModel
        && candidateOllamaModel.length <= 256
        && !/[\u0000-\u001f]/u.test(candidateOllamaModel)
        ? candidateOllamaModel
        : 'gemma3:4b';
    const models = {};
    for (const [backend, model] of Object.entries(record(source.models))) {
        const normalized = String(model || '').trim();
        if (isSupportedBackend(backend) && normalized && normalized.length <= 256 && !/[\u0000-\u001f]/u.test(normalized)) {
            models[backend] = normalized;
        }
    }
    const efforts = {};
    const supportedEfforts = new Set(['minimal', 'low', 'medium', 'high', 'xhigh']);
    for (const [backend, effort] of Object.entries(record(source.efforts))) {
        if (isSupportedBackend(backend) && typeof effort === 'string' && supportedEfforts.has(effort))
            efforts[backend] = effort;
    }
    for (const backend of BACKEND_IDS) {
        if (isCodexBackend(backend) && !efforts[backend])
            efforts[backend] = 'low';
    }
    return { engine, ollamaModel, models, efforts };
}
export function normalizeContinueBehavior(value) {
    return ['ollamaDraft', 'quickSend'].includes(String(value)) ? String(value) : 'sessionModelDraft';
}
export function normalizeTypography(input, typographyDefaults) {
    const value = record(input);
    const weights = [400, 500, 600];
    const uiFontFamily = String(value.uiFontFamily || typographyDefaults.uiFontFamily).trim().slice(0, 512) || typographyDefaults.uiFontFamily;
    const uiFontWeight = weights.includes(Number(value.uiFontWeight)) ? Number(value.uiFontWeight) : typographyDefaults.uiFontWeight;
    return {
        uiFontFamily,
        uiFontSize: Math.min(20, Math.max(11, Number(value.uiFontSize) || typographyDefaults.uiFontSize)),
        uiFontWeight,
        contentFontFamily: String(value.contentFontFamily || uiFontFamily).trim().slice(0, 512) || uiFontFamily,
        contentFontSize: Math.min(24, Math.max(11, Number(value.contentFontSize) || typographyDefaults.contentFontSize)),
        contentFontWeight: weights.includes(Number(value.contentFontWeight)) ? Number(value.contentFontWeight) : uiFontWeight,
        codeFontFamily: String(value.codeFontFamily || typographyDefaults.codeFontFamily).trim().slice(0, 512) || typographyDefaults.codeFontFamily,
        codeFontSize: Math.min(20, Math.max(11, Number(value.codeFontSize) || typographyDefaults.codeFontSize)),
        codeFontWeight: weights.includes(Number(value.codeFontWeight)) ? Number(value.codeFontWeight) : typographyDefaults.codeFontWeight,
        highContrast: value.highContrast === undefined ? typographyDefaults.highContrast : Boolean(value.highContrast),
    };
}
export function normalizeContentWidth(value) {
    return ['comfortable', 'wide', 'full'].includes(String(value)) ? String(value) : 'comfortable';
}
export function normalizeLanguage(value) {
    return ['system', 'zh-CN', 'en-US'].includes(String(value)) ? String(value) : 'system';
}
export function normalizeAdditional(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value))
        return {};
    return Object.fromEntries(Object.entries(value).flatMap(([id, text]) => id && typeof text === 'string' ? [[id.includes(':') ? id : `codex:${id}`, text.slice(0, 32000)]] : []));
}
export function normalizeOpeningMessages(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value))
        return {};
    return Object.fromEntries(Object.entries(value).slice(0, 2048).flatMap(([key, raw]) => {
        const message = record(raw);
        if (!key || !message || typeof message !== 'object')
            return [];
        const text = truncateUtf8(String(message.text || '').trim(), 16 * 1024);
        const responsibility = truncateCharacters(String(message.responsibility || '').trim(), 4096);
        return text || responsibility ? [[key.includes(':') ? key : `codex:${key}`, {
                    text,
                    responsibility,
                    source: String(message.source || 'history').slice(0, 64),
                    capturedAt: String(message.capturedAt || new Date().toISOString()).slice(0, 128),
                    truncated: Boolean(message.truncated),
                }]] : [];
    }));
}
function truncateUtf8(value, limit) {
    const text = String(value || '');
    const encoded = new TextEncoder().encode(text);
    if (encoded.length <= limit)
        return text;
    return new TextDecoder().decode(encoded.slice(0, limit)).replace(/\uFFFD$/u, '');
}
function truncateCharacters(value, limit) { return [...String(value || '')].slice(0, limit).join(''); }
