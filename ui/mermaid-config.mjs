// Generated from ui-src; run npm run build:ui. Do not edit.
export const MERMAID_PREFERENCES_DEFAULTS = Object.freeze({
    style: 'auto',
    density: 'standard',
    curve: 'rounded',
    layout: 'auto',
    fontSize: 15,
});
const allowed = Object.freeze({
    style: new Set(['auto', 'classic', 'neo', 'handDrawn', 'document']),
    density: new Set(['compact', 'standard', 'loose']),
    curve: new Set(['rounded', 'linear', 'step', 'basis']),
    layout: new Set(['auto', 'dagre', 'elk']),
});
export function normalizeMermaidPreferences(value) {
    const source = value && typeof value === 'object' ? value : {};
    const fontSize = Number(source.fontSize);
    return {
        style: typeof source.style === 'string' && allowed.style.has(source.style) ? source.style : MERMAID_PREFERENCES_DEFAULTS.style,
        density: typeof source.density === 'string' && allowed.density.has(source.density) ? source.density : MERMAID_PREFERENCES_DEFAULTS.density,
        curve: typeof source.curve === 'string' && allowed.curve.has(source.curve) ? source.curve : MERMAID_PREFERENCES_DEFAULTS.curve,
        layout: typeof source.layout === 'string' && allowed.layout.has(source.layout) ? source.layout : MERMAID_PREFERENCES_DEFAULTS.layout,
        fontSize: Number.isInteger(fontSize) && fontSize >= 12 && fontSize <= 20
            ? fontSize
            : MERMAID_PREFERENCES_DEFAULTS.fontSize,
    };
}
export function mermaidInitializeConfig(preferences, { dark = false, fontFamily = '' } = {}) {
    const value = normalizeMermaidPreferences(preferences);
    const styles = {
        auto: { theme: dark ? 'neo-dark' : 'neo', look: 'neo' },
        classic: { theme: dark ? 'dark' : 'default', look: 'classic' },
        neo: { theme: dark ? 'neo-dark' : 'neo', look: 'neo' },
        handDrawn: { theme: dark ? 'dark' : 'neutral', look: 'handDrawn', handDrawnSeed: 1 },
        document: { theme: 'neutral', look: 'classic' },
    };
    const spacing = {
        compact: { nodeSpacing: 24, rankSpacing: 32, diagramPadding: 8, padding: 10 },
        standard: { nodeSpacing: 38, rankSpacing: 48, diagramPadding: 12, padding: 15 },
        loose: { nodeSpacing: 56, rankSpacing: 70, diagramPadding: 18, padding: 20 },
    };
    return {
        startOnLoad: false,
        securityLevel: 'strict',
        suppressErrorRendering: true,
        htmlLabels: false,
        ...styles[value.style],
        ...(value.layout === 'auto' ? {} : { layout: value.layout }),
        fontSize: value.fontSize,
        ...(String(fontFamily).trim() ? { fontFamily: String(fontFamily).trim() } : {}),
        flowchart: {
            ...spacing[value.density],
            curve: value.curve,
        },
    };
}
