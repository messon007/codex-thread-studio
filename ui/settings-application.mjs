// Generated from ui-src; run npm run build:ui. Do not edit.
import { normalizeTranslationPreferences, normalizeContinueBehavior, normalizeContentWidth, normalizeLanguage, normalizeAdditional, normalizeOpeningMessages } from './preference-normalization.mjs';
import { normalizeMermaidPreferences } from './mermaid-config.mjs';
import { normalizeHiddenSessionDirectories } from './thread-catalog.mjs';
import { normalizeThreadRouter, migrateLegacyResponsibilities } from './thread-router.mjs';
import { normalizeStoredTurnOptions } from './session-model-preferences.mjs';
import { normalizeStoredMessageQueues, normalizeQueueDepth } from './message-queue.mjs';
const record = (value) => value && typeof value === 'object' && !Array.isArray(value) ? value : {};
export function createSettingsApplication(state, services) {
    const { $, gatewayFetch, normalizeRightRailWidthRatio, normalizeTypography, typographyDefaults, migrateDefaultFontFamilies, isSupportedBackend, emptyBackendSelections, normalizeAnnotationDrafts, resolveLanguage, normalizeLocalizedTemplates, defaultAnnotationPrompt, markPreferencesReady, persistOpeningMessageState, applySidebarState, getLocale, setLanguage, syncEmbeddedBrowserTranslations, t, applyAppearance, persistPreferences, sessionResources, renderLocalizedUI, toast, annotationPromptDefaults, populateSettingsForm } = services;
    async function loadPreferences() {
        const loadJson = async (path, label) => {
            try {
                const response = await gatewayFetch(path, { cache: 'no-store' });
                if (response.ok)
                    return record(await response.json());
                console.warn(`Unable to load ${label}: HTTP ${response.status}`);
            }
            catch (error) {
                console.warn(`Unable to load ${label}`, error);
            }
            return {};
        };
        const [saved, storedSessionState] = await Promise.all([
            loadJson('/studio/preferences', 'preferences'),
            loadJson('/studio/session-state', 'session state'),
        ]);
        state.language = normalizeLanguage(saved.language);
        state.theme = saved.theme === 'dark' ? 'dark' : 'light';
        state.contentWidth = normalizeContentWidth(saved.contentWidth);
        state.hiddenSessionDirectories = normalizeHiddenSessionDirectories(saved.hiddenSessionDirectories);
        state.sharedDocumentDirectories = normalizeSharedDocumentDirectories(saved.sharedDocumentDirectories);
        state.sessionDirectoryIgnore = Array.isArray(saved.sessionDirectoryIgnore)
            ? saved.sessionDirectoryIgnore.map((value) => String(value)).filter((value) => value.length <= 4096)
            : [];
        state.wsl = {
            distribution: String(saved.wslDistribution || '').trim(),
            user: String(saved.wslUser || '').trim(),
            codexBinary: String(saved.wslCodexBinary || 'codex').trim() || 'codex',
            opencodeBinary: String(saved.wslOpencodeBinary || 'opencode').trim() || 'opencode',
        };
        state.sidebarCollapsed = Boolean(saved.sidebarCollapsed);
        state.rightRailWidthRatio = normalizeRightRailWidthRatio(saved.rightRailWidthRatio ?? saved.artifactWidthRatio);
        state.typography = normalizeTypography({ ...typographyDefaults, ...migrateDefaultFontFamilies(saved.typography) });
        state.mermaid = normalizeMermaidPreferences(saved.mermaid);
        const markdownMode = record(saved.markdown).mode;
        state.markdown = { mode: typeof markdownMode === 'string' && ['reading', 'technical', 'compact'].includes(markdownMode) ? markdownMode : 'technical' };
        state.translation = normalizeTranslationPreferences(saved.translation);
        state.desktopNotifications = Boolean(saved.desktopNotifications);
        state.queueDepth = normalizeQueueDepth(saved.queueDepth);
        state.continueBehavior = normalizeContinueBehavior(saved.continueBehavior);
        state.browser = {
            enabled: true,
            restoreTabs: false,
            allowHttp: true,
            allowPrivateNetwork: false,
            allowLocalhost: true,
            previewJavaScript: true,
            embeddedWidth: 720,
            agent: { enabled: false, provider: 'playwright-mcp', profile: 'persistent', approval: 'interactive', allowedOrigins: [] },
            ...record(saved.browser),
        };
        state.router = normalizeThreadRouter(saved.router);
        state.backend = isSupportedBackend(state.router.controllerBackend)
            ? state.router.controllerBackend
            : 'codex';
        state.selectedByBackend = emptyBackendSelections();
        const routerId = state.router.controllers[state.backend];
        if (typeof routerId === 'string' && routerId)
            state.selectedByBackend[state.backend] = routerId;
        state.startupRouterSelectionPending = true;
        state.attentionThreads = new Set();
        state.selectedId = state.selectedByBackend[state.backend];
        state.turnOptions = normalizeStoredTurnOptions(storedSessionState.turnOptions);
        state.messageQueues = normalizeStoredMessageQueues(storedSessionState.messageQueues);
        state.pausedMessageQueues = new Set(Object.keys(state.messageQueues));
        state.runningMessageQueues = new Set();
        state.messageQueueErrors = new Map();
        state.annotationDrafts = normalizeAnnotationDrafts(storedSessionState.annotationDrafts);
        state.annotationAdditional = normalizeAdditional(storedSessionState.annotationAdditional);
        state.pinnedSessions = new Set(Array.isArray(storedSessionState.pinnedSessions)
            ? storedSessionState.pinnedSessions.filter((key) => typeof key === 'string' && key.includes(':')).slice(0, 10)
            : []);
        const initialLocale = resolveLanguage(state.language);
        state.annotationPromptTemplates = normalizeLocalizedTemplates(saved.annotationPromptTemplates);
        if (!state.annotationPromptTemplates[initialLocale]) {
            state.annotationPromptTemplates[initialLocale] = defaultAnnotationPrompt(initialLocale);
        }
        state.activeAnnotationPromptTemplate = state.annotationPromptTemplates[initialLocale];
        const storedOpeningMessages = normalizeOpeningMessages(storedSessionState.openingMessages);
        state.openingMessages = migrateLegacyResponsibilities(storedOpeningMessages, saved.router);
        markPreferencesReady();
        for (const [key, message] of Object.entries(state.openingMessages)) {
            if (JSON.stringify(message) !== JSON.stringify(storedOpeningMessages[key])) {
                persistOpeningMessageState(key);
            }
        }
        applySidebarState();
    }
    async function saveSettings(event) {
        event.preventDefault();
        const template = $('#annotation-template').value.trim();
        if (!template.includes('{{annotations}}')) {
            $('#settings-error').textContent = 'The comment template must contain {{annotations}}.';
            $('#settings-error').classList.remove('hidden');
            return;
        }
        const previousLocale = getLocale();
        const sharedDocumentDirectories = normalizeSharedDocumentDirectories($('#shared-document-directories').value.split(/\r?\n/u));
        if (sharedDocumentDirectories.some((directory) => !isAbsoluteDocumentDirectory(directory))) {
            $('#settings-error').textContent = t('Shared document directories must use absolute paths.');
            $('#settings-error').classList.remove('hidden');
            return;
        }
        state.annotationPromptTemplates[previousLocale] = template.slice(0, 32000);
        state.language = normalizeLanguage($('#language-select').value);
        setLanguage(state.language);
        syncEmbeddedBrowserTranslations();
        state.theme = $('#theme-select').value === 'dark' ? 'dark' : 'light';
        state.contentWidth = normalizeContentWidth($('#content-width').value);
        state.queueDepth = normalizeQueueDepth($('#queue-depth').value);
        state.continueBehavior = normalizeContinueBehavior($('#continue-behavior').value);
        state.sharedDocumentDirectories = sharedDocumentDirectories;
        const translationModel = $('#translation-model').value.trim().slice(0, 256);
        const translationEffort = $('#translation-effort').value;
        state.translation.engine = $('#translation-engine').value === 'ollama' ? 'ollama' : 'backend';
        state.translation.ollamaModel = $('#translation-ollama-model').value.trim().slice(0, 256) || 'gemma3:4b';
        if (translationModel)
            state.translation.models[state.backend] = translationModel;
        else
            delete state.translation.models[state.backend];
        if (translationEffort)
            state.translation.efforts[state.backend] = translationEffort;
        else
            delete state.translation.efforts[state.backend];
        state.typography = normalizeTypography({
            uiFontFamily: $('#ui-font-family').value.trim(),
            uiFontSize: Number($('#ui-font-size').value),
            uiFontWeight: Number($('#ui-font-weight').value),
            contentFontFamily: $('#content-font-family').value.trim(),
            contentFontSize: Number($('#content-font-size').value),
            contentFontWeight: Number($('#content-font-weight').value),
            codeFontFamily: $('#code-font-family').value.trim(),
            codeFontSize: Number($('#code-font-size').value),
            codeFontWeight: Number($('#code-font-weight').value),
            highContrast: $('#high-contrast').checked,
        });
        const wantsNotifications = $('#desktop-notifications').checked;
        if (wantsNotifications && 'Notification' in window && Notification.permission === 'default') {
            await Notification.requestPermission().catch(() => 'denied');
        }
        state.desktopNotifications = wantsNotifications && (!('Notification' in window) || Notification.permission !== 'denied');
        const previousWsl = JSON.stringify(state.wsl);
        if (state.hostPlatform === 'windows') {
            state.wsl = {
                distribution: $('#wsl-distribution').value.trim(),
                user: $('#wsl-user').value.trim(),
                codexBinary: $('#wsl-codex-binary').value.trim() || 'codex',
                opencodeBinary: $('#wsl-opencode-binary').value.trim() || 'opencode',
            };
        }
        const nextLocale = getLocale();
        state.activeAnnotationPromptTemplate = nextLocale === previousLocale
            ? template.slice(0, 32000)
            : state.annotationPromptTemplates[nextLocale] || defaultAnnotationPrompt(nextLocale);
        state.annotationPromptTemplates[nextLocale] = state.activeAnnotationPromptTemplate;
        applyAppearance();
        try {
            await persistPreferences();
        }
        catch (error) {
            $('#settings-error').textContent = t('Unable to save settings');
            $('#settings-error').classList.remove('hidden');
            return;
        }
        sessionResources.invalidate(state.backend, state.selectedId, { force: true });
        $('#settings-dialog').close();
        renderLocalizedUI();
        if (state.hostPlatform === 'windows' && JSON.stringify(state.wsl) !== previousWsl) {
            toast(t('WSL settings saved. Restart Studio to apply them.'));
        }
    }
    function resetSettings() {
        state.language = 'system';
        setLanguage(state.language);
        syncEmbeddedBrowserTranslations();
        state.theme = 'light';
        state.contentWidth = 'comfortable';
        state.typography = { ...typographyDefaults };
        state.translation = normalizeTranslationPreferences(null);
        state.desktopNotifications = false;
        state.queueDepth = 1;
        state.continueBehavior = 'sessionModelDraft';
        state.wsl = { distribution: '', user: '', codexBinary: 'codex', opencodeBinary: 'opencode' };
        state.annotationPromptTemplates = { ...annotationPromptDefaults };
        state.activeAnnotationPromptTemplate = defaultAnnotationPrompt();
        populateSettingsForm();
        applyAppearance();
    }
    function normalizeSharedDocumentDirectories(values = []) {
        if (!Array.isArray(values))
            return [];
        return [...new Set(values
                .map((value) => String(value || '').trim())
                .filter((value) => value && value.length <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)))]
            .slice(0, 256);
    }
    function isAbsoluteDocumentDirectory(value) {
        return value.startsWith('/') || /^[a-z]:[\\/]/iu.test(value) || /^\\\\[^\\]/u.test(value);
    }
    return { loadPreferences, saveSettings, resetSettings, normalizeSharedDocumentDirectories, isAbsoluteDocumentDirectory };
}
