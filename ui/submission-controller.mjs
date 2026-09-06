// Generated from ui-src; run npm run build:ui. Do not edit.
import { executeQueuedMessage } from './queue-execution.mjs';
import { executeComposerSend } from './composer-send.mjs';
import { startTurnWithPreparation } from './session-dispatch.mjs';
import { turnStartParams } from './session-catalog.mjs';
import { composerImageInputs } from './composer-images.mjs';
import { completedQueueShouldAdvance } from './message-queue.mjs';
import { beginOptimisticCodexTurn, reconcileOptimisticCodexTurn, rollbackOptimisticCodexTurn, applyCodexNotification } from './codex-native.mjs';
export function createSubmissionController(state, services) {
    const { $, selectedStateKey, shellCommandFromComposer, matchingSlashCommands, executeSlashCommand, isRouterThread, threadRouter, rpc, dispatchBackendRpc, sessionDispatch, prepareComposerTurn, isCodexBackend, isSupportedBackend, backendDescriptor, currentBackend, threadForRef, configuredTurnOptions, queuedTurnOptions, messageQueueModel, setComposerDraftValue, composerDrafts, hideComposerMenu, renderComposerState, renderTranscript, beginTranscriptFollowingLatest, setCatalogThreadActivity, rollbackCatalogThreadActivity, markCachedModelValidated, beginTurnLatencyTrace, bindTurnLatencyTrace, markTurnLatency, finishTurnLatencyTrace, randomId, persistMessageQueue, showError, toast, t } = services;
    async function sendComposer(event) {
        event.preventDefault();
        const input = $('#composer-input');
        const text = input.value.trim();
        const initialStateKey = selectedStateKey();
        const shellCommand = shellCommandFromComposer(input.value);
        if (shellCommand !== null) {
            if (!shellCommand || !state.selectedId)
                return;
            if (state.model.activeTurnId) {
                showError(new Error('Wait for the current turn to finish or stop it before running a local shell command.'));
                return;
            }
            const button = $('#send-message');
            button.disabled = true;
            beginTranscriptFollowingLatest();
            try {
                await rpc('thread/shellCommand', { threadId: state.selectedId, command: shellCommand }, 120_000);
                setComposerDraftValue(initialStateKey, '');
                hideComposerMenu();
                renderComposerState();
                toast(t('Shell command sent to {backend}', { backend: currentBackend().name }));
            }
            catch (error) {
                showError(error);
            }
            finally {
                renderComposerState();
            }
            return;
        }
        const slashName = text.match(/^\/([\w-]+)$/)?.[1];
        const slash = slashName && matchingSlashCommands(slashName).find((command) => command.name === slashName);
        if (slash) {
            setComposerDraftValue(initialStateKey, '');
            hideComposerMenu();
            try {
                await executeSlashCommand(slash.action);
            }
            catch (error) {
                showError(error);
            }
            return;
        }
        if (!state.selectedId)
            return;
        const stateKey = selectedStateKey();
        const pendingImages = [...(state.pendingImages[stateKey] || [])];
        const imageInputs = composerImageInputs(pendingImages);
        if (!text && !imageInputs.length)
            return;
        if (isRouterThread()) {
            const button = $('#send-message');
            button.disabled = true;
            beginTranscriptFollowingLatest();
            try {
                await threadRouter.startTurn(text, imageInputs);
                setComposerDraftValue(stateKey, '');
                state.pendingImages[stateKey] = [];
                hideComposerMenu();
                renderComposerState();
                return true;
            }
            catch (error) {
                showError(error);
            }
            finally {
                renderComposerState();
            }
            return false;
        }
        const backend = state.backend;
        const threadId = state.selectedId;
        const targetModel = state.model;
        const skillInputs = [...(state.pendingSkills[stateKey] || [])];
        const fileInputs = [...(state.pendingFiles[stateKey] || [])];
        const turnInput = [...(text ? [{ type: 'text', text }] : []), ...imageInputs, ...skillInputs, ...fileInputs];
        const turnOptions = configuredTurnOptions();
        const button = $('#send-message');
        button.disabled = true;
        beginTranscriptFollowingLatest(targetModel);
        let optimisticTurnId = null;
        let latencyTrace = null;
        let composerCleared = false;
        let catalogActivity = null;
        return executeComposerSend({
            send: async () => {
                if (state.model.activeTurnId) {
                    catalogActivity = setCatalogThreadActivity(backend, threadId, { status: 'active', touch: true });
                    await rpc('turn/steer', {
                        threadId,
                        expectedTurnId: state.model.activeTurnId,
                        clientUserMessageId: randomId(),
                        input: turnInput,
                    });
                    return { mode: 'steer' };
                }
                else {
                    const clientUserMessageId = randomId();
                    const ref = { backend, id: threadId };
                    if (isCodexBackend(backend)) {
                        optimisticTurnId = beginOptimisticCodexTurn(targetModel, { clientUserMessageId, input: turnInput });
                        latencyTrace = beginTurnLatencyTrace(clientUserMessageId, threadId);
                        setComposerDraftValue(stateKey, '');
                        state.pendingSkills[stateKey] = [];
                        state.pendingFiles[stateKey] = [];
                        state.pendingImages[stateKey] = [];
                        composerCleared = true;
                        hideComposerMenu();
                        renderComposerState();
                        renderTranscript();
                    }
                    catalogActivity = setCatalogThreadActivity(backend, threadId, { status: 'active', touch: true });
                    const result = await startTurnWithPreparation({
                        registry: sessionDispatch,
                        ref,
                        prepare: () => prepareComposerTurn(ref),
                        start: () => dispatchBackendRpc(backend, 'turn/start', turnStartParams(backendDescriptor(backend).kind, threadForRef(ref), {
                            threadId,
                            clientUserMessageId,
                            input: turnInput,
                            ...turnOptions,
                        })),
                        recoverThreadNotFound: isCodexBackend(backend),
                    });
                    return { mode: 'start', result };
                }
            },
            acknowledged: ({ mode, result }) => {
                if (mode === 'steer')
                    toast('Message added to the current turn');
                if (result?.turn) {
                    if (isCodexBackend(backend) && optimisticTurnId) {
                        reconcileOptimisticCodexTurn(targetModel, optimisticTurnId, result.turn);
                        bindTurnLatencyTrace(latencyTrace, result.turn.id);
                        markTurnLatency(latencyTrace, 'turn_start_ack');
                    }
                    else
                        applyTurnAcknowledgement(targetModel, result.turn, threadId);
                    markCachedModelValidated(backend, targetModel);
                    if (targetModel === state.model)
                        renderTranscript();
                }
                if (!composerCleared) {
                    setComposerDraftValue(stateKey, '');
                    state.pendingSkills[stateKey] = [];
                    state.pendingFiles[stateKey] = [];
                    state.pendingImages[stateKey] = [];
                }
                hideComposerMenu();
                renderComposerState();
            },
            failed: (error, turnAccepted) => {
                if (!turnAccepted)
                    rollbackCatalogThreadActivity(catalogActivity);
                if (optimisticTurnId) {
                    rollbackOptimisticCodexTurn(targetModel, optimisticTurnId);
                    finishTurnLatencyTrace(latencyTrace, 'failed');
                    if (targetModel === state.model)
                        renderTranscript();
                }
                if (composerCleared) {
                    if (!composerDrafts.value(stateKey).trim())
                        setComposerDraftValue(stateKey, text);
                    if (!(state.pendingSkills[stateKey] || []).length)
                        state.pendingSkills[stateKey] = skillInputs;
                    if (!(state.pendingFiles[stateKey] || []).length)
                        state.pendingFiles[stateKey] = fileInputs;
                    if (!(state.pendingImages[stateKey] || []).length)
                        state.pendingImages[stateKey] = pendingImages;
                    if (state.backend === backend && state.selectedId === threadId)
                        renderComposerState();
                }
                showError(error);
            },
            finished: () => renderComposerState(),
        });
    }
    async function runNextQueuedMessage(ref) {
        if (!ref || !isSupportedBackend(ref.backend))
            return;
        const key = selectedStateKey(ref.id, ref.backend);
        const queue = state.messageQueues[key] || [];
        if (!queue.length || state.pausedMessageQueues.has(key) || state.runningMessageQueues.has(key))
            return;
        const model = messageQueueModel(ref);
        if (model?.activeTurnId)
            return;
        let catalogActivity = null;
        try {
            const acknowledgement = { result: null };
            const sent = await executeQueuedMessage(state, key, {
                send: async (message) => {
                    if (key === selectedStateKey()) {
                        beginTranscriptFollowingLatest(model);
                        renderComposerState();
                    }
                    const clientUserMessageId = randomId();
                    catalogActivity = setCatalogThreadActivity(ref.backend, ref.id, { status: 'active', touch: true });
                    const result = await startTurnWithPreparation({
                        registry: sessionDispatch,
                        ref,
                        prepare: () => prepareComposerTurn(ref),
                        start: () => dispatchBackendRpc(ref.backend, 'turn/start', turnStartParams(backendDescriptor(ref.backend).kind, threadForRef(ref), {
                            threadId: ref.id,
                            clientUserMessageId,
                            input: [...(message.text ? [{ type: 'text', text: message.text }] : []), ...message.input],
                            ...queuedTurnOptions(ref),
                        })),
                        recoverThreadNotFound: isCodexBackend(ref.backend),
                    });
                    return result;
                },
                acknowledged: (result) => {
                    acknowledgement.result = result;
                    if (result?.turn && model)
                        applyTurnAcknowledgement(model, result.turn, ref.id);
                    if (model)
                        markCachedModelValidated(ref.backend, model);
                },
                persist: () => persistMessageQueue(key),
                failed: (error, accepted) => {
                    if (!accepted)
                        rollbackCatalogThreadActivity(catalogActivity);
                    if (key === selectedStateKey())
                        showError(error);
                },
                acceptedPersistenceError: t('The message was accepted, but the queue could not be updated. Verify the conversation before resuming.'),
            });
            if (sent && key === selectedStateKey()) {
                if (acknowledgement.result?.turn)
                    renderTranscript();
                toast(t('Queued message sent'));
            }
        }
        finally {
            if (key === selectedStateKey())
                renderComposerState();
        }
    }
    function pauseMessageQueue(ref, reason = '') {
        if (!ref)
            return;
        const key = selectedStateKey(ref.id, ref.backend);
        if (!(state.messageQueues[key] || []).length)
            return;
        state.pausedMessageQueues.add(key);
        if (reason)
            state.messageQueueErrors.set(key, reason);
        if (key === selectedStateKey())
            renderComposerState();
    }
    function handleQueuedTurnCompletion(ref, status) {
        const key = selectedStateKey(ref.id, ref.backend);
        if (!(state.messageQueues[key] || []).length)
            return;
        if (!completedQueueShouldAdvance(status, state.pausedMessageQueues.has(key))) {
            if (state.pausedMessageQueues.has(key))
                return;
            pauseMessageQueue(ref, t('Previous turn did not complete normally'));
            return;
        }
        queueMicrotask(() => runNextQueuedMessage(ref).catch((error) => console.error('Queued turn failed', error)));
    }
    function applyTurnAcknowledgement(model, turn, threadId = '') {
        if (!turn?.id)
            return false;
        const existing = model.turns.find((candidate) => String(candidate?.id || '') === String(turn.id));
        const terminal = ['completed', 'failed', 'cancelled', 'interrupted'].includes(existing?.status || '');
        if (turn.status === 'inProgress' && terminal)
            return false;
        const method = turn.status === 'inProgress' ? 'turn/started' : 'turn/completed';
        return applyCodexNotification(model, { method, params: { threadId, turn } });
    }
    return { sendComposer, runNextQueuedMessage, pauseMessageQueue, handleQueuedTurnCompletion, applyTurnAcknowledgement };
}
