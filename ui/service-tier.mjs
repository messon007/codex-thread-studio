// Generated from ui-src; run npm run build:ui. Do not edit.
function modelId(value) {
    if (typeof value === 'string')
        return value.trim();
    return String(value?.model || value?.id || value?.modelID || '').trim();
}
export function selectedCatalogModel(models, overrideModel = '', sessionModel = '') {
    const catalog = Array.isArray(models) ? models : [];
    const selectedId = modelId(overrideModel) || modelId(sessionModel);
    return catalog.find((model) => modelId(model) === selectedId)
        || (!selectedId ? catalog.find((model) => model.isDefault) : undefined);
}
export function modelSupportsFast(model) {
    const serviceTiers = Array.isArray(model?.serviceTiers)
        ? model.serviceTiers.map((tier) => String(tier?.id || '').trim().toLowerCase())
        : [];
    const legacyTiers = Array.isArray(model?.additionalSpeedTiers)
        ? model.additionalSpeedTiers.map((tier) => String(tier || '').trim().toLowerCase())
        : [];
    return [...serviceTiers, ...legacyTiers].includes('fast');
}
export function effectiveServiceTier(overrideServiceTier, sessionServiceTier) {
    return String(overrideServiceTier || sessionServiceTier || 'default').trim().toLowerCase() || 'default';
}
export function resolveFastMode({ models = [], overrideModel = '', sessionModel = '', overrideServiceTier = '', sessionServiceTier = '', } = {}) {
    const selectedModel = selectedCatalogModel(models, overrideModel, sessionModel);
    const serviceTier = effectiveServiceTier(overrideServiceTier, sessionServiceTier);
    return {
        model: modelId(overrideModel) || modelId(sessionModel) || modelId(selectedModel),
        selectedModel,
        supported: modelSupportsFast(selectedModel),
        enabled: serviceTier === 'fast',
        serviceTier,
    };
}
