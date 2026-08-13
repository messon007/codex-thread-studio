export const RIGHT_RAIL_DEFAULT_RATIO = 0.44
export const RIGHT_RAIL_MAX_RATIO = 0.65

export function rightRailWidthBounds({
  containerWidth,
  sidebarWidth = 0,
  dividerWidth = 0,
  nominalMinWidth = 310,
  maxRatio = RIGHT_RAIL_MAX_RATIO,
}) {
  const available = Math.max(1, Number(containerWidth) - Number(sidebarWidth) - Number(dividerWidth))
  const min = Math.min(available, Math.max(1, Number(nominalMinWidth) || 310))
  const max = Math.max(min, available * maxRatio)
  return { available, min, max }
}

export function clampRightRailWidth(value, bounds) {
  return Math.round(Math.max(bounds.min, Math.min(bounds.max, Number(value) || bounds.min)))
}
