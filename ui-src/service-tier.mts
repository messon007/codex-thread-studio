import type { CatalogModel, ModelReference } from './session-types.mjs'

function modelId(value: ModelReference | undefined) {
  if (typeof value === 'string') return value.trim()
  return String(value?.model || value?.id || value?.modelID || '').trim()
}

export function selectedCatalogModel(
  models: readonly CatalogModel[] | null | undefined,
  overrideModel: ModelReference = '',
  sessionModel: ModelReference = '',
) {
  const catalog = Array.isArray(models) ? models : []
  const selectedId = modelId(overrideModel) || modelId(sessionModel)
  return catalog.find((model) => modelId(model) === selectedId)
    || (!selectedId ? catalog.find((model) => model.isDefault) : undefined)
}

export function modelSupportsFast(model: CatalogModel | null | undefined) {
  const serviceTiers = Array.isArray(model?.serviceTiers)
    ? model.serviceTiers.map((tier) => String(tier?.id || '').trim().toLowerCase())
    : []
  const legacyTiers = Array.isArray(model?.additionalSpeedTiers)
    ? model.additionalSpeedTiers.map((tier) => String(tier || '').trim().toLowerCase())
    : []
  return [...serviceTiers, ...legacyTiers].includes('fast')
}

export function effectiveServiceTier(overrideServiceTier: unknown, sessionServiceTier: unknown) {
  return String(overrideServiceTier || sessionServiceTier || 'default').trim().toLowerCase() || 'default'
}

export function resolveFastMode({
  models = [],
  overrideModel = '',
  sessionModel = '',
  overrideServiceTier = '',
  sessionServiceTier = '',
}: {
  models?: readonly CatalogModel[]
  overrideModel?: ModelReference
  sessionModel?: ModelReference
  overrideServiceTier?: unknown
  sessionServiceTier?: unknown
} = {}) {
  const selectedModel = selectedCatalogModel(models, overrideModel, sessionModel)
  const serviceTier = effectiveServiceTier(overrideServiceTier, sessionServiceTier)
  return {
    model: modelId(overrideModel) || modelId(sessionModel) || modelId(selectedModel),
    selectedModel,
    supported: modelSupportsFast(selectedModel),
    enabled: serviceTier === 'fast',
    serviceTier,
  }
}
