// Generated from ui-src; run npm run build:ui. Do not edit.
export function createTeamsClient(fetcher) {
    const pendingMutations = new Map();
    const storage = typeof sessionStorage === 'undefined' ? null : sessionStorage;
    const request = async (body) => {
        let mutationKey = '';
        if (['command', 'create_group'].includes(String(body.action))) {
            const payload = { ...body };
            delete payload.request_id;
            mutationKey = JSON.stringify(payload);
            const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(mutationKey));
            mutationKey = 'studio.teams.pending.' + Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, '0')).join('');
            let saved = null;
            try {
                saved = storage?.getItem(mutationKey) || null;
            }
            catch { /* memory fallback */ }
            const requestId = pendingMutations.get(mutationKey) || saved || String(body.request_id || crypto.randomUUID());
            pendingMutations.set(mutationKey, requestId);
            try {
                storage?.setItem(mutationKey, requestId);
            }
            catch { /* keep memory receipt */ }
            body = { ...body, request_id: requestId };
        }
        const response = await fetcher(body.action === 'ordinary_start' ? '/studio/teams/dispatch' : '/studio/teams', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
        const value = await response.json();
        if (!response.ok)
            throw new Error(String(value.error || response.statusText));
        if (mutationKey) {
            pendingMutations.delete(mutationKey);
            try {
                storage?.removeItem(mutationKey);
            }
            catch { /* no storage */ }
        }
        return value;
    };
    async function reserve(backend, thread, reason) {
        const result = await request({ action: 'reserve', session: { backend, thread }, owner: crypto.randomUUID(), reason });
        if (!result.granted) {
            // This request has not sent anything. Releasing its own waiting reservation is safe.
            if (result.id)
                await request({ action: 'release', id: result.id });
            throw new Error('Teams has a pending or uncertain delivery for this session. Wait for it to finish, or inspect Teams before sending.');
        }
        return result.id;
    }
    async function guarded(backend, thread, reason, send) {
        const id = await reserve(backend, thread, reason);
        const result = await send(); // A failed/uncertain send deliberately retains its persistent reservation.
        if (id)
            await request({ action: 'release', id });
        return result;
    }
    async function start(backend, params, fallback, queueContext) {
        const response = await request({ action: 'ordinary_start', backend, params, ...(queueContext ? { queue_context: queueContext } : {}) });
        return response.managed ? response.result : fallback();
    }
    return { request, reserve, guarded, start, release: (id) => request({ action: 'release', id }) };
}
export function createTeamsPanel(client, navigate) {
    let dialog = null;
    let content;
    let message;
    let snapshot = {};
    let selected = '';
    let timer;
    let refreshing = false;
    const request = client.request;
    const id = () => crypto.randomUUID();
    const el = (tag, text = '') => { const e = document.createElement(tag); e.textContent = text; return e; };
    const button = (label, action) => {
        const b = el('button', label);
        b.type = 'button';
        b.addEventListener('click', () => { message.textContent = ''; b.disabled = true; void Promise.resolve().then(action).catch(error => { message.textContent = String(error instanceof Error ? error.message : error); }).finally(() => { b.disabled = false; }); });
        return b;
    };
    const field = (form, label, multiline = false, value = '') => {
        const wrapper = el('label', label);
        const input = multiline ? el('textarea') : el('input');
        input.value = value;
        input.required = true;
        wrapper.append(input);
        form.append(wrapper);
        return input;
    };
    const select = (form, label, options, initial = '') => {
        const wrapper = el('label', label), input = el('select');
        for (const option of options) {
            const o = el('option', option.text);
            o.value = option.value;
            input.append(o);
        }
        if (initial)
            input.value = initial;
        wrapper.append(input);
        form.append(wrapper);
        return input;
    };
    const command = async (group, value) => { await request({ action: 'command', group_id: group, request_id: id(), command: value }); await refresh(); };
    const version = (task) => ({ task_id: task.id, expected_revision: task.revision, attempt_id: task.attempt_id });
    function subform(title) {
        const form = el('section');
        form.className = 'teams-form';
        form.append(el('h3', title));
        content.replaceChildren(form);
        form.append(button('Back', refresh));
        return form;
    }
    async function installation(backend, removing = false) {
        const preview = await request({ action: 'install_preview', backend });
        const form = subform(removing ? 'Remove Teams tools' : 'Install Teams tools');
        form.append(el('p', String(preview.impact)), el('code', String(preview.file)));
        form.append(button(removing ? 'Confirm removal' : 'Confirm installation', async () => {
            await request({ action: removing ? 'uninstall' : 'install', backend, file: preview.file, version: preview.version });
            await refresh();
        }));
    }
    async function createGroup() {
        const form = subform('Create a group · 2–5 existing Codex sessions');
        const name = field(form, 'Group name');
        const rows = el('div');
        form.append(rows);
        const drafts = [];
        const add = async () => {
            if (drafts.length >= 5)
                return;
            const row = el('fieldset');
            row.append(el('legend', `Member ${drafts.length + 1}`));
            rows.append(row);
            const backend = select(row, 'Codex backend', snapshot.backends.map(value => ({ value, text: value })));
            const thread = select(row, 'Existing session', []);
            let cursor = null;
            let generation = 0;
            const more = button('Load more', () => load(true));
            more.hidden = true;
            row.append(more);
            const load = async (append = false) => {
                const currentGeneration = append ? generation : ++generation;
                if (!append) {
                    cursor = null;
                    thread.replaceChildren();
                    more.hidden = true;
                }
                const list = await request({ action: 'sessions', backend: backend.value, cursor });
                if (currentGeneration !== generation)
                    return;
                for (const t of list.data || []) {
                    const o = el('option', `${t.name || t.preview || t.id} · ${t.cwd || ''}`);
                    o.value = t.id;
                    thread.append(o);
                }
                cursor = list.nextCursor || null;
                more.hidden = !cursor;
            };
            backend.addEventListener('change', () => { void load().catch(e => { message.textContent = String(e); }); });
            const memberName = field(row, 'Display name'), responsibility = field(row, 'Responsibility', true);
            drafts.push({ backend, thread, name: memberName, responsibility });
            await load();
        };
        form.append(button('Add member', add), button('Create group', async () => {
            if (!name.value.trim() || drafts.some(d => !d.thread.value || !d.name.value.trim() || !d.responsibility.value.trim()))
                throw new Error('Fill in every member and responsibility.');
            const group = await request({ action: 'create_group', request_id: id(), name: name.value.trim(), members: drafts.map(d => ({ session: { backend: d.backend.value, thread: d.thread.value }, name: d.name.value.trim(), responsibility: d.responsibility.value.trim() })) });
            selected = group.id;
            await refresh();
        }));
        await add();
        await add();
    }
    function newTask(group) {
        const form = subform('Delegate work');
        const assignee = select(form, 'Owner', group.members.map((m) => ({ value: m.id, text: m.name })), group.orchestrator || '');
        const title = field(form, 'Title'), work = field(form, 'Request / reproduction steps', true), acceptance = field(form, 'Acceptance criteria · one per line', true);
        form.append(button('Delegate', async () => {
            if (!title.value.trim() || !work.value.trim() || !acceptance.value.trim())
                throw new Error('Provide a title, request and acceptance criteria.');
            await command(group.id, { op: 'Delegate', delegation: { to_member_id: assignee.value, title: title.value.trim(), request: work.value.trim(), acceptance: acceptance.value.split('\n').filter(s => s.trim()).map((text, i) => ({ id: `criterion-${i + 1}`, text: text.trim() })), dependencies: [], parent: null, continuation: null } });
        }));
    }
    function groupSettings(group) {
        const form = subform('Group settings');
        const name = field(form, 'Group name', false, group.name);
        const profiles = group.members.map((m) => {
            const row = el('fieldset');
            row.append(el('legend', m.name));
            form.append(row);
            const display = field(row, 'Display name', false, m.name), responsibility = field(row, 'Responsibility', true, m.responsibility);
            if (!group.archived)
                row.append(button('Remove member', () => command(group.id, { op: 'ChangeMembers', expected_revision: group.revision, add: [], remove_member_ids: [m.id] })));
            return { id: m.id, display, responsibility };
        });
        if (group.archived) {
            form.append(el('p', 'Archived groups retain their task history. Their sessions are available for other groups.'));
            return;
        }
        form.append(button('Save names and responsibilities', () => command(group.id, { op: 'SetGroupProfile', expected_revision: group.revision, name: name.value, members: profiles.map((m) => ({ member_id: m.id, display_name: m.display.value, responsibility: m.responsibility.value })) })));
        form.append(el('p', 'Pause the group and settle pending deliveries before changing members or archiving. Active tasks must be completed or cancelled.'));
        if (group.members.length < 5) {
            const backend = select(form, 'New member backend', snapshot.backends.map(value => ({ value, text: value })));
            const thread = field(form, 'Existing session ID'), display = field(form, 'New member name'), responsibility = field(form, 'New member responsibility', true);
            form.append(button('Add existing session', () => command(group.id, { op: 'ChangeMembers', expected_revision: group.revision, add: [{ session: { backend: backend.value, thread: thread.value.trim() }, name: display.value.trim(), responsibility: responsibility.value.trim() }], remove_member_ids: [] })));
        }
        form.append(button('Archive group', () => command(group.id, { op: 'ArchiveGroup', expected_revision: group.revision })));
    }
    function taskDetails(group, task) {
        const form = subform(task.title);
        form.append(el('p', `${task.status} · attempt ${task.attempts} · revision ${task.revision}`), el('pre', task.request));
        const criteria = el('ul');
        for (const c of task.acceptance)
            criteria.append(el('li', c.text));
        form.append(criteria);
        if (task.checkpoint)
            form.append(el('pre', `${task.checkpoint.summary}\nNext: ${task.checkpoint.next_action}`));
        for (const q of task.questions) {
            const section = el('section');
            section.append(el('h4', q.blocking ? 'Blocking question' : 'Question'), el('p', q.text));
            form.append(section);
            if (q.answer)
                section.append(el('p', q.answer));
            else if (q.to === 'User') {
                const answer = field(section, 'Your answer', true);
                section.append(button('Answer', () => command(group.id, { op: 'Answer', task: version(task), question_id: q.id, answer: answer.value })));
            }
        }
        for (const blocker of task.blockers.filter((b) => !b.resolution && b.needs_user)) {
            const section = el('section');
            section.append(el('p', blocker.detail));
            form.append(section);
            const resolution = field(section, 'Resolution', true);
            section.append(button('Resolve', () => command(group.id, { op: 'Resolve', task: version(task), blocker_id: blocker.id, resolution: resolution.value })));
        }
        const result = task.results.at(-1);
        if (result) {
            form.append(el('h4', 'Submitted result'), el('pre', result.submission.summary), el('p', result.submission.limitations));
            for (const evidence of result.submission.evidence)
                form.append(el('pre', `${evidence.kind}: ${evidence.locator}\n${evidence.description}`));
            const outcomes = el('ul');
            for (const outcome of result.submission.criteria) {
                const criterion = task.acceptance.find((c) => c.id === outcome.criterion_id);
                outcomes.append(el('li', `${criterion?.text || outcome.criterion_id}: ${outcome.status} — ${outcome.explanation}`));
            }
            form.append(outcomes);
            if (task.reviews.length) {
                const reviews = el('details');
                reviews.append(el('summary', 'Review history'));
                for (const review of task.reviews)
                    reviews.append(el('h4', review.accepted ? 'Accepted' : 'Changes requested'), el('p', review.assessment.summary), el('p', review.requested_changes || ''));
                form.append(reviews);
            }
            if (task.status === 'Submitted' && task.requester === 'User') {
                const notes = field(form, 'Verification summary / requested changes', true);
                const verdicts = task.acceptance.map((criterion) => ({ criterion, verdict: select(form, criterion.text, [{ value: 'NotChecked', text: 'Not checked' }, { value: 'Pass', text: 'Verified · pass' }, { value: 'Fail', text: 'Verified · fail' }]) }));
                const review = (accept) => command(group.id, { op: 'Review', task: version(task), result_id: result.id, accept, assessment: { summary: notes.value, criteria: verdicts.map(({ criterion, verdict }) => ({ criterion_id: criterion.id, verdict: verdict.value, explanation: notes.value, evidence: [] })) }, requested_changes: accept ? null : notes.value });
                form.append(button('Accept verified result', () => review(true)), button('Request changes', () => review(false)));
            }
        }
        const recovery = el('details');
        recovery.append(el('summary', 'Recovery and retry budget'));
        const verification = field(recovery, 'Evidence from inspecting the native session', true);
        const evidence = () => [{ kind: 'Note', locator: `thread:${task.claim?.session.thread || ''}`, description: verification.value }];
        recovery.append(el('p', 'Use only after inspecting the native session. These actions never retry an uncertain delivery automatically.'));
        if (task.claim)
            recovery.append(button('Confirm previous execution has settled', () => command(group.id, { op: 'ReconcileExecution', task: version(task), claim_id: task.claim_id || task.claim.id, confirmed_settled: true, evidence: evidence() })));
        if (result)
            recovery.append(button('Confirm result is ready for review', () => command(group.id, { op: 'ReconcileResult', task: version(task), result_id: result.id, allow_review: true, evidence: evidence() })));
        for (const review of task.reviews.filter((r) => r.turn))
            recovery.append(button('Reconcile review barrier', () => command(group.id, { op: 'ReconcileReview', task: version(task), result_id: review.result_id, barrier_turn_id: review.turn.id, release: true, evidence: evidence() })));
        const attempts = field(recovery, 'Additional rework attempts', false, '1');
        attempts.setAttribute('type', 'number');
        attempts.setAttribute('min', '1');
        attempts.setAttribute('max', '10');
        recovery.append(button('Grant rework attempts', () => command(group.id, { op: 'GrantReworkBudget', task: version(task), additional_attempts: Number(attempts.value), reason: verification.value })));
        form.append(recovery);
        if (!['Completed', 'Cancelled'].includes(task.status)) {
            const reason = field(form, 'Cancellation reason');
            form.append(button('Cancel task', () => command(group.id, { op: 'Cancel', task: version(task), reason: reason.value })));
        }
    }
    function render() {
        content.replaceChildren();
        const bar = el('div');
        bar.className = 'teams-toolbar';
        bar.append(el('strong', snapshot.phase === 'disabling' ? 'Teams stopping · awaiting execution receipts' : snapshot.enabled ? 'Teams enabled' : 'Teams disabled'));
        bar.append(button(snapshot.enabled ? 'Disable Teams' : 'Enable Teams', async () => { await request({ action: snapshot.enabled ? 'disable' : 'enable' }); await refresh(); }));
        if (snapshot.enabled)
            bar.append(button('Create group', createGroup));
        content.append(bar);
        const setup = el('details');
        setup.append(el('summary', 'Tools and backend configuration'));
        for (const backend of snapshot.backends) {
            const registration = snapshot.registrations.find((r) => r.backend === backend);
            const row = el('p', backend + (registration?.installed ? ' · installed ' : ' · not installed '));
            row.append(button(registration?.installed ? 'Uninstall tools' : 'Install tools', () => installation(backend, Boolean(registration?.installed))));
            if (registration?.file)
                row.append(el('code', registration.file));
            setup.append(row);
        }
        content.append(setup);
        if (Object.keys(snapshot.errors || {}).length) {
            const diagnostics = el('details');
            diagnostics.append(el('summary', 'Recent diagnostics'));
            for (const [key, error] of Object.entries(snapshot.errors || {}))
                diagnostics.append(el('p', `${key}: ${String(error)}`));
            content.append(diagnostics);
        }
        const groups = snapshot.groups;
        if (!groups.length) {
            content.append(el('p', 'Create a group from 2–5 existing sessions. Messages wait in the native queue; busy work is never interrupted. Waiting tasks do not block unrelated work.'));
            return;
        }
        const tabs = el('nav');
        for (const g of groups)
            tabs.append(button(g.name, () => { selected = g.id; render(); }));
        content.append(tabs);
        const group = groups.find(g => g.id === selected) || groups.find(g => !g.archived) || groups[0];
        selected = group.id;
        const header = el('div');
        header.className = 'teams-toolbar';
        header.append(el('h3', group.name));
        if (!group.archived)
            header.append(button(group.paused ? 'Resume group' : 'Pause group', () => command(group.id, { op: 'SetPaused', expected_revision: group.revision, paused: !group.paused })), button('Delegate work', () => newTask(group)));
        content.append(header);
        const owner = select(content, 'Optional orchestrator · group task owner', [{ value: '', text: 'None · peer collaboration' }, ...group.members.map((m) => ({ value: m.id, text: m.name }))], group.orchestrator || '');
        owner.disabled = Boolean(group.archived);
        owner.addEventListener('change', () => { void command(group.id, { op: 'SetOrchestrator', expected_revision: group.revision, member_id: owner.value || null }).catch(e => { message.textContent = String(e); }); });
        for (const m of group.members) {
            const row = el('p', `${m.name} — ${m.responsibility} `);
            row.append(button('Open session', async () => { await navigate(m.session.backend, m.session.thread); dialog?.close(); }));
            content.append(row);
        }
        header.append(button('Group settings', () => groupSettings(group)));
        const tasks = snapshot.tasks.filter((t) => t.group_id === group.id);
        if (!tasks.length)
            content.append(el('p', 'No tasks yet. Delegate work to a member or to the optional orchestrator.'));
        for (const task of tasks) {
            const row = el('article');
            row.className = 'teams-task';
            row.append(el('span', task.status), button(task.title, () => taskDetails(group, task)));
            content.append(row);
        }
        const queue = snapshot.queue.filter((q) => q.scope === group.id);
        if (queue.length) {
            const detail = el('details');
            detail.open = true;
            detail.append(el('summary', 'Pending deliveries'));
            for (const q of queue) {
                const row = el('p', `${q.state} · ${q.session.thread} · ${q.stable_id}`);
                if (['Unknown', 'NativeQueued', 'Started', 'Abandoned'].includes(q.state)) {
                    row.append(button('Check native receipt', async () => { const result = await request({ action: 'reconcile_delivery', entry_id: q.id }); await refresh(); message.textContent = result.message || 'Native receipts checked. Uncertain deliveries are never retried automatically.'; }));
                }
                if (q.state === 'Unknown') {
                    row.append(el('span', ' · No automatic retry. Inspect native history before reconciliation.'));
                    row.append(button('Record unresolved delivery', () => {
                        const form = subform('Record unresolved delivery');
                        form.append(el('p', 'Pause the group first. This records an unresolved delivery; it does not prove that Codex did not execute it. Strict group resume remains blocked until native receipts settle.'));
                        const reason = field(form, 'What you inspected and why the receipt is unresolved', true);
                        form.append(button('Record without retrying', async () => { await request({ action: 'abandon_delivery', entry_id: q.id, reason: reason.value }); await refresh(); }));
                    }));
                }
                detail.append(row);
            }
            content.append(detail);
        }
        for (const dispatch of snapshot.user_dispatches || []) {
            const row = el('p', `Uncertain user submission · ${dispatch.session.thread} · ${dispatch.id} `);
            row.append(button('Check native receipt', async () => { const result = await request({ action: 'reconcile_submission', session: dispatch.session, id: dispatch.id }); await refresh(); message.textContent = result.found ? '' : result.message || 'No positive native receipt was found.'; }));
            content.append(row);
        }
        for (const hold of snapshot.reservations || []) {
            const row = el('p', `Reserved: ${hold.reason} · ${hold.session.thread} `);
            row.append(button('Release after checking session', async () => { const form = subform('Reconcile reservation'); form.append(el('p', 'Only release after verifying that no sender or supervision job still owns this session. A failed request may already have executed.')); form.append(button('I verified the session; release', async () => { await client.release(hold.id); await refresh(); })); }));
            content.append(row);
        }
    }
    async function refresh() {
        if (refreshing)
            return;
        refreshing = true;
        try {
            snapshot = await request({ action: 'snapshot' });
            render();
        }
        finally {
            refreshing = false;
        }
    }
    async function open() {
        if (!dialog) {
            dialog = el('dialog');
            dialog.className = 'teams-dialog';
            dialog.setAttribute('aria-label', 'Agent Teams');
            const style = el('style');
            style.textContent = '.teams-dialog{width:min(900px,94vw);max-height:88vh;box-sizing:border-box;border:1px solid var(--border,#ccd4df);border-radius:12px;padding:22px;background:var(--panel,#fff);color:var(--text,#192330)}.teams-dialog::backdrop{background:#0005}.teams-dialog header,.teams-toolbar,.teams-dialog nav{display:flex;gap:12px;align-items:center;flex-wrap:wrap}.teams-dialog header{justify-content:space-between}.teams-dialog button{padding:7px 12px;margin:3px;cursor:pointer;border:1px solid var(--border);border-radius:7px;background:var(--panel);color:var(--text);font:inherit;transition:background .12s}.teams-dialog button:hover{background:var(--brand-soft);border-color:var(--brand)}.teams-dialog button:disabled{opacity:.55;cursor:default}.teams-dialog button:focus-visible,.teams-dialog select:focus-visible,.teams-dialog input:focus-visible,.teams-dialog textarea:focus-visible{outline:2px solid var(--brand);outline-offset:2px}.teams-dialog label{display:block;margin:12px 0}.teams-dialog input,.teams-dialog select,.teams-dialog textarea{display:block;width:100%;box-sizing:border-box;padding:8px;margin-top:5px;border:1px solid var(--border);border-radius:7px;background:var(--panel-soft);color:var(--text);font:inherit}.teams-dialog textarea{min-height:90px;resize:vertical}.teams-dialog pre{white-space:pre-wrap;overflow-wrap:anywhere;background:var(--panel-soft);border:1px solid var(--border);border-radius:7px;padding:12px}.teams-dialog code{display:block;overflow-wrap:anywhere}.teams-dialog fieldset{margin:12px 0;border:1px solid var(--border);border-radius:8px;padding:12px}.teams-dialog details{padding:10px 0}.teams-dialog summary{cursor:pointer;color:var(--muted)}.teams-dialog{box-shadow:var(--shadow-md);overscroll-behavior:contain}.teams-dialog h3{margin:16px 0}.teams-dialog nav{padding:12px 0;border-bottom:1px solid var(--border)}.teams-task{display:flex;gap:12px;align-items:center;border-bottom:1px solid #8884;padding:8px}.teams-task span{min-width:90px}.teams-error{color:var(--red,#ba3333);white-space:pre-wrap}.teams-dialog section{margin:10px 0}';
            const header = el('header');
            header.append(el('h2', 'Agent Teams'), button('Close', () => dialog?.close()));
            message = el('p');
            message.className = 'teams-error';
            message.setAttribute('role', 'status');
            content = el('div');
            dialog.append(style, header, message, content);
            document.body.append(dialog);
            dialog.addEventListener('close', () => { if (timer)
                clearInterval(timer); });
        }
        dialog.showModal();
        await refresh();
        // Do not replace an editor or review form while the user types.
        timer = setInterval(() => { if (dialog?.open && !content.querySelector('.teams-form') && !content.contains(document.activeElement))
            void refresh().catch(e => { message.textContent = String(e); }); }, 5000);
    }
    return { open };
}
