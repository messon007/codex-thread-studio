// Generated from ui-src; run npm run build:ui. Do not edit.
import { runHiddenUtilitySession } from './hidden-utility-session.mjs';
import { randomContinuePrompt } from './composer-tools.mjs';
import { CONTINUATION_DRAFT_INSTRUCTIONS, CONTINUATION_DRAFT_SCHEMA, continuationDraftInput, continuationDraftTurnState } from './continuation-draft.mjs';
export function createComposerActions(state, services) {
    const { $, selectedStateKey, composerDrafts, setCurrentComposerValue, hideComposerMenu, latestAgentResponseText, showError, t, currentBackend, renderComposerState, toast, setComposerDraftValue, gatewayFetch, truncateCharacters, selectedThread, currentTurnOptions, isCodexBackend, randomId, rpc, dispatchBackendRpc, sessionRefKey, routerRuntimeKey, persistMessageQueue, runNextQueuedMessage, sessionRefFromKey, pauseMessageQueue } = services;
    let editingQueuedMessage = null;
    function handleContinueAction() {
        if (state.continueBehavior === 'quickSend') {
            quickSendContinueMessage();
            return;
        }
        draftContinueMessage();
    }
    function quickSendContinueMessage() {
        const button = $('#continue-thread');
        if (button.disabled || button.classList.contains('hidden'))
            return;
        setCurrentComposerValue(randomContinuePrompt());
        hideComposerMenu();
        $('#composer-form').requestSubmit();
    }
    function composerHasPendingContent(key = selectedStateKey()) {
        const draft = key === selectedStateKey() ? $('#composer-input').value : composerDrafts.value(key);
        return Boolean(draft.trim()
            || state.pendingImages[key]?.length
            || state.pendingSkills[key]?.length
            || state.pendingFiles[key]?.length);
    }
    async function draftContinueMessage() {
        const button = $('#continue-thread');
        if (button.disabled || button.classList.contains('hidden'))
            return;
        const key = selectedStateKey();
        const source = latestAgentResponseText().trim();
        if (!source) {
            showError(new Error(t('This session has no {backend} response to continue.', { backend: currentBackend().name })));
            return;
        }
        state.continuationDraftLoads.add(key);
        renderComposerState();
        try {
            const behavior = state.continueBehavior;
            const prompt = behavior === 'ollamaDraft'
                ? await draftContinueWithOllama(source)
                : await draftContinueWithSessionModel(source, key);
            if (!prompt)
                throw new Error(t('The continuation backend returned an empty draft'));
            if (selectedStateKey() !== key)
                return;
            if (state.model.activeTurnId || latestAgentResponseText().trim() !== source || composerHasPendingContent(key)) {
                toast(t('The conversation changed before the continuation draft was ready'));
                return;
            }
            setComposerDraftValue(key, prompt);
            hideComposerMenu();
            const input = $('#composer-input');
            input.setSelectionRange(prompt.length, prompt.length);
            input.focus();
            toast(t('Continuation draft added to the composer'));
        }
        catch (error) {
            if (selectedStateKey() === key)
                showError(error);
        }
        finally {
            state.continuationDraftLoads.delete(key);
            renderComposerState();
        }
    }
    async function draftContinueWithOllama(source) {
        const response = await gatewayFetch('/studio/ollama/continue-draft', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model: state.translation.ollamaModel || 'gemma3:4b',
                assistantResponse: truncateCharacters(source, 32_000),
            }),
        });
        const payload = await response.json().catch(() => ({}));
        const result = payload && typeof payload === 'object' ? payload : {};
        const error = result.error && typeof result.error === 'object' ? result.error : {};
        if (!response.ok)
            throw new Error(typeof error.message === 'string' ? error.message : t('Local Ollama could not draft a continuation'));
        return String(result.prompt || '').trim();
    }
    async function draftContinueWithSessionModel(source, stateKey) {
        if (!state.ready || !state.selectedId)
            throw new Error(t('The current backend is not ready for continuation drafts'));
        const backend = state.backend;
        const generation = state.socketGeneration;
        const cwd = selectedThread()?.cwd || '';
        const options = currentTurnOptions();
        const selectedModel = selectedThread()?.model;
        const model = String(options.model || (typeof selectedModel === 'string' ? selectedModel : '')).trim();
        const effort = String(options.effort || '').trim();
        const draft = await runHiddenUtilitySession(state, {
            backend, codex: isCodexBackend(backend), cwd, model, effort,
            name: `Studio continuation ${randomId()}`,
            instructions: CONTINUATION_DRAFT_INSTRUCTIONS,
            input: continuationDraftInput(source),
            outputSchema: CONTINUATION_DRAFT_SCHEMA,
            validateBeforeStart: true,
            ensureCurrent: () => ensureContinuationBackend(backend, generation, stateKey),
            parse: continuationDraftTurnState,
            translateError: t,
            missingTaskMessage: t('The current backend did not create a continuation draft task'),
            timeoutMessage: t('Continuation draft timed out'),
            rpc,
            remove: (targetBackend, params, timeoutMs) => dispatchBackendRpc(targetBackend, 'thread/delete', params, timeoutMs),
            cleanupError: error => console.warn('Unable to remove the hidden continuation session', error),
            sessionKey: sessionRefKey,
            turnKey: routerRuntimeKey,
        });
        return draft.prompt;
    }
    function ensureContinuationBackend(backend, generation, stateKey) {
        if (state.backend !== backend || state.socketGeneration !== generation || !state.ready || selectedStateKey() !== stateKey) {
            throw new Error(t('Continuation draft stopped because the current session changed'));
        }
    }
    async function queueComposerMessage() {
        const key = selectedStateKey();
        const queue = state.messageQueues[key] || [];
        const wasPaused = state.pausedMessageQueues.has(key);
        const previousError = state.messageQueueErrors.get(key);
        if (queue.length >= state.queueDepth) {
            $('#composer-message-queue').scrollIntoView({ block: 'nearest', behavior: 'smooth' });
            return;
        }
        const input = $('#composer-input');
        const text = input.value.trim();
        if (text.length > 64 * 1024) {
            showError(new Error(t('Queued messages must be 65,536 characters or fewer.')));
            return;
        }
        const pendingImages = state.pendingImages[key] || [];
        if (pendingImages.length) {
            showError(new Error(t('Image messages cannot be queued. Send or steer them directly.')));
            return;
        }
        const skillInputs = [...(state.pendingSkills[key] || [])];
        const fileInputs = [...(state.pendingFiles[key] || [])];
        if (!text && !skillInputs.length && !fileInputs.length)
            return;
        const message = {
            id: randomId(),
            text,
            input: [...skillInputs, ...fileInputs],
            createdAt: Date.now(),
        };
        state.messageQueues[key] = [...queue, message];
        if (!queue.length) {
            state.pausedMessageQueues.delete(key);
            state.messageQueueErrors.delete(key);
        }
        try {
            await persistMessageQueue(key);
        }
        catch (error) {
            state.messageQueues[key] = queue;
            if (wasPaused)
                state.pausedMessageQueues.add(key);
            else
                state.pausedMessageQueues.delete(key);
            if (previousError)
                state.messageQueueErrors.set(key, previousError);
            else
                state.messageQueueErrors.delete(key);
            showError(error);
            renderComposerState();
            return;
        }
        setComposerDraftValue(key, '');
        state.pendingSkills[key] = [];
        state.pendingFiles[key] = [];
        hideComposerMenu();
        renderComposerState();
        toast(t('Message queued'));
    }
    function openQueuedMessageEditor(id) {
        const key = selectedStateKey();
        const message = (state.messageQueues[key] || []).find((entry) => entry.id === id);
        if (!message)
            return;
        editingQueuedMessage = { key, id };
        $('#edit-queued-message-text').value = message.text;
        const attachmentCount = message.input.filter((entry) => entry.type !== 'text').length;
        $('#edit-queued-message-note').textContent = attachmentCount ? t('{count} file or skill references will be kept.', { count: attachmentCount }) : '';
        $('#edit-queued-message-note').classList.toggle('hidden', !attachmentCount);
        $('#edit-queued-message-dialog').showModal();
        setTimeout(() => $('#edit-queued-message-text').focus(), 30);
    }
    function closeQueuedMessageEditor() {
        editingQueuedMessage = null;
        $('#edit-queued-message-dialog').close();
    }
    async function saveEditedQueuedMessage(event) {
        event.preventDefault();
        if (!editingQueuedMessage)
            return;
        const { key, id } = editingQueuedMessage;
        const message = (state.messageQueues[key] || []).find((entry) => entry.id === id);
        if (!message)
            return closeQueuedMessageEditor();
        const text = $('#edit-queued-message-text').value.trim();
        if (!text && !message.input.length)
            return;
        const previousText = message.text;
        const previousInput = message.input;
        message.text = text;
        message.input = message.input.filter((entry) => entry.type !== 'text');
        try {
            await persistMessageQueue(key);
            closeQueuedMessageEditor();
            if (key === selectedStateKey())
                renderComposerState();
        }
        catch (error) {
            message.text = previousText;
            message.input = previousInput;
            showError(error);
        }
    }
    async function deleteQueuedMessage(id) {
        const key = selectedStateKey();
        const previous = state.messageQueues[key] || [];
        const wasPaused = state.pausedMessageQueues.has(key);
        const previousError = state.messageQueueErrors.get(key);
        state.messageQueues[key] = previous.filter((message) => message.id !== id);
        if (!state.messageQueues[key].length) {
            delete state.messageQueues[key];
            state.pausedMessageQueues.delete(key);
            state.messageQueueErrors.delete(key);
        }
        try {
            await persistMessageQueue(key);
        }
        catch (error) {
            state.messageQueues[key] = previous;
            if (wasPaused)
                state.pausedMessageQueues.add(key);
            if (previousError)
                state.messageQueueErrors.set(key, previousError);
            throw error;
        }
        finally {
            renderComposerState();
        }
    }
    function resumeSelectedMessageQueue() {
        const key = selectedStateKey();
        state.pausedMessageQueues.delete(key);
        state.messageQueueErrors.delete(key);
        renderComposerState();
        runNextQueuedMessage(sessionRefFromKey(key)).catch((error) => console.error('Queue resume failed', error));
    }
    async function interruptTurn() {
        if (!state.selectedId || !state.model.activeTurnId)
            return;
        pauseMessageQueue({ backend: state.backend, id: state.selectedId });
        try {
            await rpc('turn/interrupt', { threadId: state.selectedId, turnId: state.model.activeTurnId });
            toast('Requested interruption of the current turn');
        }
        catch (error) {
            showError(error);
        }
    }
    return { handleContinueAction, quickSendContinueMessage, composerHasPendingContent, draftContinueMessage, draftContinueWithOllama, draftContinueWithSessionModel, ensureContinuationBackend, queueComposerMessage, openQueuedMessageEditor, closeQueuedMessageEditor, saveEditedQueuedMessage, deleteQueuedMessage, resumeSelectedMessageQueue, interruptTurn };
}
