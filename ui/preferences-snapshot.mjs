// Generated from ui-src; run npm run build:ui. Do not edit.
/** Explicit allowlist: session drafts, queues, pins and model choices belong in SQLite. */
export function preferencesSnapshot(state) {
    return {
        language: state.language,
        theme: state.theme,
        contentWidth: state.contentWidth,
        hiddenSessionDirectories: state.hiddenSessionDirectories,
        sharedDocumentDirectories: state.sharedDocumentDirectories,
        sessionDirectoryIgnore: state.sessionDirectoryIgnore,
        wslDistribution: state.wsl.distribution || null,
        wslUser: state.wsl.user || null,
        wslCodexBinary: state.wsl.codexBinary || 'codex',
        wslOpencodeBinary: state.wsl.opencodeBinary || 'opencode',
        sidebarCollapsed: state.sidebarCollapsed,
        rightRailWidthRatio: state.rightRailWidthRatio,
        typography: state.typography,
        mermaid: state.mermaid,
        markdown: state.markdown,
        translation: state.translation,
        desktopNotifications: state.desktopNotifications,
        queueDepth: state.queueDepth,
        continueBehavior: state.continueBehavior,
        browser: state.browser,
        annotationPromptTemplates: state.annotationPromptTemplates,
        router: Object.keys(state.router.controllers).length || state.router.fallbacks.length ? state.router : null,
    };
}
