// Generated from ui-src; run npm run build:ui. Do not edit.
import { applyCodexNotification, createCodexViewModel } from './codex-native.mjs';
import { waitForUtilityResult } from './utility-task.mjs';
/** One hidden task, with backend-pinned cleanup that never blocks its result. */
export async function runHiddenUtilitySession(state, options) {
    const { backend, codex, cwd, model, effort, name } = options;
    const nameKey = `${backend}:${name}`;
    state.hiddenUtilityThreadNames.add(nameKey);
    let threadId = '';
    let task = null;
    try {
        if (options.validateBeforeStart)
            options.ensureCurrent();
        const started = await options.rpc('thread/start', {
            cwd,
            ...(model ? { model } : {}),
            ...(codex ? {
                ephemeral: true, approvalPolicy: 'never', sandbox: 'read-only', developerInstructions: options.instructions,
            } : { name }),
        }, 30_000);
        threadId = String(started?.thread?.id || '');
        if (!threadId)
            throw new Error(options.missingTaskMessage);
        const key = options.sessionKey(backend, threadId);
        state.hiddenUtilityThreads.add(key);
        if (codex)
            state.hiddenCodexThreads.add(key);
        options.ensureCurrent();
        if (codex) {
            const utilityModel = createCodexViewModel();
            utilityModel.threadId = threadId;
            task = { backend, threadId, turnId: '', model: utilityModel };
            state.structuredUtilityTasks.set(key, task);
        }
        const startedTurn = await options.rpc('turn/start', {
            threadId, cwd, input: [{ type: 'text', text: options.input }],
            ...(!codex ? { developerInstructions: options.instructions } : {}),
            outputSchema: options.outputSchema,
            ...(model ? { model } : {}),
            ...(effort ? { effort } : {}),
        }, 150_000);
        if (task && startedTurn?.turn?.id) {
            const turnId = String(startedTurn.turn.id);
            state.hiddenCodexTurns.add(options.turnKey(backend, turnId));
            task.turnId ||= turnId;
            if (!task.model.turns.some(turn => String(turn.id) === turnId)) {
                applyCodexNotification(task.model, { method: 'turn/started', params: { threadId, turn: startedTurn.turn } });
            }
        }
        return await waitForUtilityResult({
            ensureCurrent: options.ensureCurrent,
            read: async () => {
                // Offscreen lifecycle streams may omit item deltas. Read once at terminal
                // completion when callers need a complete structured result.
                const completed = task?.model.turns.at(-1)?.status === 'completed';
                const thread = codex && !(options.readCompletedThread && completed)
                    ? task?.model
                    : (await options.rpc('thread/read', { threadId, includeTurns: true, cwd }, 30_000))?.thread;
                return options.parse(thread);
            },
            intervalMs: codex ? 100 : 350,
            timeoutMs: 150_000, timeoutMessage: options.timeoutMessage, errorMessage: options.translateError,
        });
    }
    finally {
        if (threadId) {
            const key = options.sessionKey(backend, threadId);
            state.structuredUtilityTasks.delete(key);
            options.remove(backend, { threadId, ...(!codex ? { cwd } : {}) }, 15_000)
                .catch(options.cleanupError)
                .finally(() => {
                state.hiddenUtilityThreads.delete(key);
                state.hiddenCodexThreads.delete(key);
                if (task?.turnId)
                    state.hiddenCodexTurns.delete(options.turnKey(backend, task.turnId));
                state.hiddenUtilityThreadNames.delete(nameKey);
            });
        }
        else
            state.hiddenUtilityThreadNames.delete(nameKey);
    }
}
