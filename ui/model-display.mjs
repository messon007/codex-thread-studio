// Generated from ui-src; run npm run build:ui. Do not edit.
function modelId(value) {
    if (typeof value === 'string')
        return value.trim();
    return String(value?.model || value?.id || value?.modelID || '').trim();
}
export function resolveModelDisplay({ overrideModel = '', overrideEffort = '', sessionModel = '', models = [], fallback = 'Default model', } = {}) {
    const catalog = Array.isArray(models) ? models : [];
    const selectedId = modelId(overrideModel) || modelId(sessionModel);
    const selected = catalog.find((model) => modelId(model) === selectedId)
        || (!selectedId ? catalog.find((model) => model?.isDefault) : null);
    const model = selectedId || modelId(selected) || fallback;
    const effort = String(overrideEffort || selected?.defaultReasoningEffort || '').trim();
    return {
        model,
        effort,
        label: effort ? `${model}/${effort}` : model,
    };
}
