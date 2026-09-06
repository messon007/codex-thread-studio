// Generated from ui-src; run npm run build:ui. Do not edit.
import { collectOpenCodeMessageHistory, collectOpenCodeMessageTail, collectOpenCodeRootSessions, fetchOpenCodeDirectoryStatuses, mergeOpenCodeMessagePages, normalizeOpenCodeSessions, openCodeCommandTurn, openCodeModelList, openCodeThreadFromHistory, selectOpenCodeStartedUserMessage, sliceOpenCodeMessageTail, splitOpenCodeModel } from './opencode-native.mjs';
import { textFromUserContent } from './codex-native.mjs';
import { openCodeImagePart } from './composer-images.mjs';
import { openCodeForkBody } from './thread-fork.mjs';
import { isSessionDirectoryHidden } from './thread-catalog.mjs';
function serverError(value) {
    if (!value || typeof value !== 'object')
        return undefined;
    const object = value;
    const error = object.error;
    if (error && typeof error === 'object' && typeof error.message === 'string')
        return error.message;
    return typeof object.message === 'string' ? object.message : typeof error === 'string' ? error : undefined;
}
// Endpoint result annotations describe existing wire contracts. JSON is decoded
// here; existing downstream shape guards remain responsible for malformed data.
export function createOpenCodeProtocol(state, services) {
    const { gatewayFetch, selectedThread, currentTurnOptions, t, studioPerformance, threadCatalogKey, openCodeHistoryEventSequences } = services;
    async function ensureOpenCodeAvailable() {
        const response = await gatewayFetch('/studio/opencode', { cache: 'no-store' });
        if (!response.ok) {
            const info = await response.json().catch(() => ({}));
            throw new Error(serverError(info) || `OpenCode Server HTTP ${response.status}`);
        }
    }
    async function openCodeFetch(path, { method = 'GET', body, timeoutMs = 30_000, allowInactive = false, includeHeaders = false } = {}) {
        if (!allowInactive && (state.backend !== 'opencode' || !state.ready))
            throw new Error('OpenCode Server is not ready');
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        try {
            const response = await gatewayFetch(`/opencode${path}`, {
                method,
                headers: body == null ? {} : { 'Content-Type': 'application/json' },
                ...(body == null ? {} : { body: JSON.stringify(body) }),
                signal: controller.signal,
                cache: 'no-store',
            });
            if (response.status === 204)
                return includeHeaders ? { value: null, headers: response.headers } : null;
            const text = await response.text();
            let value = null;
            try {
                value = text ? JSON.parse(text) : null;
            }
            catch {
                value = text;
            }
            if (!response.ok)
                throw new Error(serverError(value) || `${method} ${path} failed: HTTP ${response.status}`);
            return includeHeaders ? { value: value, headers: response.headers } : value;
        }
        catch (error) {
            if (error instanceof Error && error.name === 'AbortError')
                throw new Error(t('{method} {path} request timed out', { method, path }));
            throw error;
        }
        finally {
            clearTimeout(timer);
        }
    }
    async function fetchOpenCodeMessageHistory(threadId, directory, fetchOptions, { anchorTurnIds = [] } = {}) {
        const path = `/session/${encodeURIComponent(String(threadId))}/message`;
        const key = String(threadId || '');
        const finishHistoryFetch = studioPerformance.start('opencode.history.fetch', {
            backend: 'opencode',
            threadKey: threadCatalogKey('opencode', threadId),
        });
        let pageCount = 0;
        let historyMode = 'full';
        let latestSnapshot = { before: openCodeHistoryEventSequences.get(key) || 0, after: openCodeHistoryEventSequences.get(key) || 0 };
        const messageSnapshots = {};
        const fetchPage = async ({ limit, before }) => {
            pageCount += 1;
            const sequenceBefore = openCodeHistoryEventSequences.get(key) || 0;
            const query = new URLSearchParams({ limit: String(limit) });
            if (before)
                query.set('before', before);
            const page = await openCodeFetch(withDirectory(path, directory, query.toString()), {
                ...fetchOptions,
                includeHeaders: true,
            });
            const result = {
                messages: page.value,
                cursor: page.headers.get('x-next-cursor'),
            };
            const sequenceAfter = openCodeHistoryEventSequences.get(key) || 0;
            for (const message of Array.isArray(result.messages) ? result.messages : []) {
                const messageId = String(message?.info?.id || '');
                if (!messageId)
                    continue;
                messageSnapshots[messageId] = {
                    afterSequence: sequenceBefore === sequenceAfter ? sequenceAfter : sequenceBefore,
                    ambiguousThroughSequence: sequenceAfter,
                };
            }
            if (!before)
                latestSnapshot = {
                    before: sequenceBefore,
                    after: sequenceAfter,
                };
            return result;
        };
        try {
            const anchors = (Array.isArray(anchorTurnIds) ? anchorTurnIds : []).filter(Boolean);
            let history = anchors.length
                ? await collectOpenCodeMessageTail(fetchPage, anchors)
                : await collectOpenCodeMessageHistory(fetchPage);
            if (history.matched)
                historyMode = 'tail';
            else if (anchors.length && history.complete)
                historyMode = 'full-tail-scan';
            else if (anchors.length) {
                history = await collectOpenCodeMessageHistory(fetchPage);
                const recoveredTail = sliceOpenCodeMessageTail(history.messages, anchors);
                if (recoveredTail.matched) {
                    history = { ...history, ...recoveredTail, complete: false };
                    historyMode = 'tail-wide-fallback';
                }
                else {
                    historyMode = 'full-fallback';
                }
            }
            for (let attempt = 0; latestSnapshot.before !== latestSnapshot.after && attempt < 2; attempt += 1) {
                const latest = await fetchPage({ limit: history.matched ? 80 : 500 });
                if (Array.isArray(latest.messages)) {
                    history.messages = mergeOpenCodeMessagePages([history.messages, latest.messages]);
                    if (history.matched) {
                        const anchorIndex = history.messages.findIndex((message) => message?.info?.role === 'user'
                            && String(message.info.id || '') === String(history.anchorTurnId || ''));
                        if (anchorIndex >= 0)
                            history.messages = history.messages.slice(anchorIndex);
                    }
                }
            }
            finishHistoryFetch({
                outcome: 'loaded',
                historyMode,
                pageCount,
                messageCount: Array.isArray(history.messages) ? history.messages.length : 0,
                complete: history.complete !== false,
            });
            return {
                ...history,
                historyMode,
                historyMessageSnapshots: messageSnapshots,
            };
        }
        catch (error) {
            finishHistoryFetch({ outcome: 'failed', pageCount });
            throw error;
        }
    }
    async function fetchOpenCodeStatusSnapshot(threadId, directory, fetchOptions) {
        const key = String(threadId || '');
        let value = {};
        let snapshot = { before: -1, after: Number.POSITIVE_INFINITY };
        for (let attempt = 0; attempt < 3; attempt += 1) {
            const before = openCodeHistoryEventSequences.get(key) || 0;
            try {
                value = await openCodeFetch(withDirectory('/session/status', directory), fetchOptions);
            }
            catch {
                return { value: {}, statusEventSequence: -1 };
            }
            snapshot = { before, after: openCodeHistoryEventSequences.get(key) || 0 };
            if (snapshot.before === snapshot.after)
                break;
        }
        return {
            value,
            statusEventSequence: snapshot.before === snapshot.after ? snapshot.after : snapshot.before,
        };
    }
    async function fetchOpenCodeCatalog(limit = 100, { allowInactive = false, includeStatuses = true } = {}) {
        if (allowInactive) {
            const response = await gatewayFetch('/studio/opencode', { cache: 'no-store' });
            if (!response.ok) {
                const info = await response.json().catch(() => ({}));
                throw new Error(serverError(info) || `OpenCode Server HTTP ${response.status}`);
            }
        }
        const options = { timeoutMs: 30_000, allowInactive };
        const sessions = await collectOpenCodeRootSessions(({ limit: pageLimit, archived, roots, cursor }) => {
            const query = new URLSearchParams({
                limit: String(pageLimit),
                archived: String(archived),
                roots: String(roots),
            });
            if (cursor != null)
                query.set('cursor', String(cursor));
            return openCodeFetch(`/experimental/session?${query}`, options);
        }, limit);
        if (!includeStatuses)
            return normalizeOpenCodeSessions(sessions, {});
        const directories = [...new Set((sessions || []).map((session) => session.directory).filter((cwd) => Boolean(cwd) && !isSessionDirectoryHidden(cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore)))];
        const statuses = await fetchOpenCodeDirectoryStatuses(directories, (cwd) => openCodeFetch(withDirectory('/session/status', cwd), options));
        return normalizeOpenCodeSessions(sessions, statuses);
    }
    function directoryQuery(directory = selectedThread()?.cwd) {
        return directory ? `directory=${encodeURIComponent(directory)}` : '';
    }
    function withDirectory(path, directory, extra = '') {
        return `${path}?${[directoryQuery(directory), extra].filter(Boolean).join('&')}`;
    }
    async function openCodeRpc(method, params = {}, timeoutMs = 30_000, { allowInactive = false } = {}) {
        const catalog = allowInactive ? state.threadsByBackend.opencode : state.threads;
        const thread = catalog.find((candidate) => candidate.id === (params.threadId || state.selectedId));
        const directory = params.cwd || thread?.cwd || '';
        const fetchOptions = { timeoutMs, allowInactive };
        if (method === 'thread/list') {
            return { data: await fetchOpenCodeCatalog(Number(params.limit || 100), { allowInactive }) };
        }
        if (method === 'thread/unsubscribe')
            return {};
        if (method === 'thread/resume' || method === 'thread/read') {
            const session = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}`, directory), fetchOptions);
            const history = await fetchOpenCodeMessageHistory(params.threadId, session.directory, fetchOptions, {
                anchorTurnIds: params.historyAnchorTurnIds,
            });
            const statuses = await fetchOpenCodeStatusSnapshot(params.threadId, session.directory, fetchOptions);
            return {
                thread: openCodeThreadFromHistory(session, history.messages, statuses.value?.[session.id] || 'idle'),
                historyComplete: history.complete,
                historyMessageSnapshots: history.historyMessageSnapshots,
                historyStatusSequence: statuses.statusEventSequence,
                historyAnchorTurnId: history.anchorTurnId,
                historyMode: history.historyMode,
            };
        }
        if (method === 'thread/start') {
            const model = splitOpenCodeModel(params.model);
            const session = await openCodeFetch(withDirectory('/session', params.cwd), { method: 'POST', body: { ...(params.name ? { title: params.name } : {}), ...(model ? { model: { id: model.modelID, providerID: model.providerID } } : {}) }, timeoutMs, allowInactive });
            return { thread: normalizeOpenCodeSessions([session], {})[0] };
        }
        if (method === 'thread/name/set') {
            return openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}`, directory), { method: 'PATCH', body: { title: params.name }, timeoutMs });
        }
        if (method === 'thread/fork') {
            const session = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/fork`, directory), { method: 'POST', body: openCodeForkBody(params), timeoutMs });
            return { thread: normalizeOpenCodeSessions([session], {})[0] };
        }
        if (method === 'thread/delete')
            return openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}`, directory), { method: 'DELETE', timeoutMs });
        if (method === 'thread/archive')
            throw new Error('OpenCode does not provide a separate archive action. You can rename, fork, or delete the session.');
        if (method === 'turn/start') {
            const text = textFromUserContent(params.input);
            const model = splitOpenCodeModel(params.model);
            const skill = params.input?.find((item) => item?.type === 'skill');
            if (skill?.name) {
                if (params.input?.some((item) => item?.type === 'image' || item?.type === 'localImage')) {
                    throw new Error(t('OpenCode cannot combine a skill command with image attachments. Remove the skill or images and try again.'));
                }
                const result = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/command`, directory), {
                    method: 'POST',
                    body: { command: skill.name, arguments: text, agent: 'build' },
                    timeoutMs: Math.max(timeoutMs, 300_000),
                });
                const turn = openCodeCommandTurn(result, params.input);
                if (!turn)
                    throw new Error(t('OpenCode completed the command but did not return its user message ID.'));
                return { turn };
            }
            const imageParts = (params.input || []).map(openCodeImagePart).filter(Boolean);
            const parts = [
                ...(text ? [{ type: 'text', text }] : []),
                ...(params.input || []).filter((item) => item?.type === 'file').map(openCodeFilePart),
                ...imageParts,
            ];
            const baselineIds = await openCodeUserMessageBaseline(params.threadId, directory, fetchOptions);
            await openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/prompt_async`, directory), {
                method: 'POST',
                body: {
                    parts,
                    ...(model ? { model } : {}),
                    ...(params.developerInstructions ? { system: params.developerInstructions } : {}),
                    ...(params.outputSchema ? { format: { type: 'json_schema', schema: params.outputSchema, retryCount: 2 } } : {}),
                },
                timeoutMs,
                allowInactive,
            });
            // OpenCode owns message identity. Older servers compare their monotonic
            // IDs to decide whether a prompt loop is complete, so a client UUID can
            // keep the same user prompt running forever. Read back the authoritative
            // user ID for Router/optimistic correlation instead of supplying one.
            return openCodeStartedTurn(params.threadId, directory, params.input, baselineIds, {
                ...fetchOptions,
                // The server has already accepted the prompt. Keep resolving its
                // authoritative user ID even if the operator switches backends now.
                allowInactive: true,
            });
        }
        if (method === 'turn/steer')
            throw new Error('Messages cannot be added while OpenCode is running. Wait for completion or stop it first.');
        if (method === 'turn/interrupt')
            return openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/abort`, directory), { method: 'POST', timeoutMs });
        if (method === 'thread/shellCommand') {
            const options = currentTurnOptions();
            const model = splitOpenCodeModel(options.model || thread?.model);
            if (!model)
                throw new Error('Choose a model with /model before running an OpenCode shell command.');
            return openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/shell`, directory), { method: 'POST', body: { agent: 'build', model, command: params.command }, timeoutMs });
        }
        if (method === 'fuzzyFileSearch') {
            const query = encodeURIComponent(params.query || '');
            const files = await openCodeFetch(withDirectory('/find/file', params.roots?.[0] || directory, `query=${query}&type=file&dirs=false&limit=200`), { timeoutMs });
            return { files: (files || []).map((path) => ({ path, root: params.roots?.[0] || directory })) };
        }
        if (method === 'model/list') {
            const providers = await openCodeFetch(withDirectory('/config/providers', directory), { timeoutMs });
            return { data: openCodeModelList(providers) };
        }
        if (method === 'skills/list') {
            const commands = await openCodeFetch(withDirectory('/command', params.cwds?.[0] || directory), { timeoutMs });
            return { data: [{ skills: (commands || []).filter((command) => command.source === 'skill').map((command) => ({ name: command.name, path: command.name, description: command.description || '', enabled: true })) }] };
        }
        if (method === 'mcpServerStatus/list') {
            const servers = await openCodeFetch(withDirectory('/mcp', directory), { timeoutMs });
            return { data: Object.entries(servers || {}).map(([name, status]) => ({ name, status })) };
        }
        if (method === 'thread/compact/start' || method === 'review/start') {
            const command = method === 'review/start' ? 'review' : 'compact';
            return openCodeFetch(withDirectory(`/session/${encodeURIComponent(String(params.threadId))}/command`, directory), { method: 'POST', body: { command, arguments: '', agent: 'build' }, timeoutMs });
        }
        throw new Error(t('The OpenCode backend does not support {method} yet', { method }));
    }
    function openCodeFilePart(file) {
        const root = String(file.root || selectedThread()?.cwd || '').replace(/\/$/u, '');
        const path = String(file.path || '').replace(/^\.\//u, '');
        const absolute = path.startsWith('/') ? path : `${root}/${path}`;
        const label = `@${path}`;
        return {
            type: 'file',
            mime: 'text/plain',
            filename: path,
            url: `file://${encodeURI(absolute)}`,
            source: { type: 'file', path, text: { value: label, start: 0, end: label.length } },
        };
    }
    function openCodeTurnResult(userId, input) {
        return {
            turn: {
                id: userId,
                status: 'inProgress',
                items: [{ id: userId, type: 'userMessage', content: input }],
            },
        };
    }
    async function openCodeUserMessageBaseline(threadId, directory, fetchOptions) {
        const path = `/session/${encodeURIComponent(String(threadId))}/message`;
        const messages = await openCodeFetch(withDirectory(path, directory, 'limit=20'), fetchOptions);
        return new Set((Array.isArray(messages) ? messages : [])
            .filter((message) => message?.info?.role === 'user' && message.info.id)
            .map((message) => String(message.info?.id)));
    }
    async function openCodeStartedTurn(threadId, directory, input, baselineIds, fetchOptions) {
        const path = `/session/${encodeURIComponent(String(threadId))}/message`;
        const expectedText = textFromUserContent(input).trim();
        for (let attempt = 0; attempt < 8; attempt += 1) {
            const messages = await openCodeFetch(withDirectory(path, directory, 'limit=20'), fetchOptions);
            const message = selectOpenCodeStartedUserMessage(messages, { baselineIds: [...baselineIds], expectedText });
            const info = message?.info;
            if (info?.id)
                return openCodeTurnResult(info.id, input);
            await new Promise((resolve) => setTimeout(resolve, 30 * (attempt + 1)));
        }
        throw new Error(t('OpenCode accepted the prompt but did not return its user message ID.'));
    }
    return { ensureOpenCodeAvailable, openCodeFetch, fetchOpenCodeMessageHistory, fetchOpenCodeStatusSnapshot, fetchOpenCodeCatalog, directoryQuery, withDirectory, openCodeRpc, openCodeFilePart, openCodeTurnResult, openCodeUserMessageBaseline, openCodeStartedTurn };
}
