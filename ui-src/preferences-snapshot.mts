import type { TypographyProfile, normalizeTranslationPreferences } from './preference-normalization.mjs'
import type { normalizeMermaidPreferences } from './mermaid-config.mjs'
import type { RouterConfiguration } from './router-types.mjs'

export interface PreferencesState {
  language: string
  theme: string
  contentWidth: string
  hiddenSessionDirectories: string[]
  sharedDocumentDirectories: string[]
  sessionDirectoryIgnore: string[]
  wsl: { distribution: string; user: string; codexBinary: string; opencodeBinary: string }
  sidebarCollapsed: boolean
  rightRailWidthRatio: number
  typography: TypographyProfile
  mermaid: ReturnType<typeof normalizeMermaidPreferences>
  markdown: { mode: string }
  translation: ReturnType<typeof normalizeTranslationPreferences>
  desktopNotifications: boolean
  queueDepth: number
  continueBehavior: string
  browser: Record<string, unknown> | null
  annotationPromptTemplates: Record<string, string>
  router: RouterConfiguration
}

/** Explicit allowlist: session drafts, queues, pins and model choices belong in SQLite. */
export function preferencesSnapshot(state: PreferencesState) {
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
  }
}
