/** Optional Teams surface; depends only on a gateway request and session navigation. */
type ObjectValue = Record<string, any>
type Request = (body: ObjectValue) => Promise<ObjectValue>
export function createTeamsClient(fetcher: typeof fetch) {
  const pendingMutations = new Map<string, string>()
  const storage = typeof sessionStorage === 'undefined' ? null : sessionStorage
  const request: Request = async body => {
    let mutationKey = ''
    if (['command', 'create_group', 'delete_group'].includes(String(body.action))) {
      const payload = { ...body }; delete payload.request_id
      mutationKey = JSON.stringify(payload)
      const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(mutationKey))
      mutationKey = 'studio.teams.pending.' + Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, '0')).join('')
      let saved: string | null = null
      try { saved = storage?.getItem(mutationKey) || null } catch { /* memory fallback */ }
      const requestId = pendingMutations.get(mutationKey) || saved || String(body.request_id || crypto.randomUUID())
      pendingMutations.set(mutationKey, requestId)
      try { storage?.setItem(mutationKey, requestId) } catch { /* keep memory receipt */ }
      body = { ...body, request_id: requestId }
    }
    const response = await fetcher(body.action === 'ordinary_start' ? '/studio/teams/dispatch' : '/studio/teams', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
    const value = await response.json() as ObjectValue
    if (!response.ok) throw new Error(String(value.error || response.statusText))
    if (mutationKey) { pendingMutations.delete(mutationKey); try { storage?.removeItem(mutationKey) } catch { /* no storage */ } }
    return value
  }
  async function reserve(backend: string, thread: string, reason: string) {
    const result = await request({ action: 'reserve', session: { backend, thread }, owner: crypto.randomUUID(), reason })
    if (!result.granted) {
      // This request has not sent anything. Releasing its own waiting reservation is safe.
      if (result.id) await request({ action: 'release', id: result.id })
      throw new Error('Teams has a pending or uncertain delivery for this session. Wait for it to finish, or inspect Teams before sending.')
    }
    return result.id as string | null
  }
  async function guarded<T>(backend: string, thread: string, reason: string, send: () => Promise<T>): Promise<T> {
    const id = await reserve(backend, thread, reason)
    const result = await send() // A failed/uncertain send deliberately retains its persistent reservation.
    if (id) await request({ action: 'release', id })
    return result
  }
  async function start<T>(backend: string, params: ObjectValue, fallback: () => Promise<T>, queueContext?: ObjectValue): Promise<T> {
    const response = await request({ action: 'ordinary_start', backend, params, ...(queueContext ? { queue_context: queueContext } : {}) })
    return response.managed ? response.result as T : fallback()
  }
  return { request, reserve, guarded, start, release: (id: string) => request({ action: 'release', id }) }
}

type TeamsSurface = {
  root: HTMLElement
  activate: () => void
  close: () => void
  translate: (text: string) => string
}
export function createTeamsPanel(client: ReturnType<typeof createTeamsClient>, navigate: (backend: string, thread: string) => Promise<unknown>, surface: TeamsSurface) {
  const content = surface.root, t = surface.translate, request = client.request
  let snapshot: ObjectValue = { groups: [], tasks: [], queue: [], reservations: [], registrations: [], backends: [] }
  let selected = '', section = 'tasks', taskId = '', taskSection = 'summary', filter = 'open', search = '', groupSearch = ''
  let archived = false, setup = false, loading = false, timer: ReturnType<typeof setInterval> | undefined
  let dialog: HTMLDialogElement | null = null, dirty = false, submitting = false, opener: HTMLElement | null = null
  let listScroll = 0, taskScroll = 0
  const terminal = (task: ObjectValue) => ['Completed', 'Cancelled'].includes(task.status)
  const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') => { const e = document.createElement(tag); e.textContent = text; e.className = cls; return e }
  const label = <K extends keyof HTMLElementTagNameMap>(tag: K, text: string, cls = '') => el(tag, t(text), cls)
  const prose = (text: string) => el('div', text, 'teams-prose')
  const memberName = (group: ObjectValue, id: string) => group.members.find((m: ObjectValue) => m.id === id)?.name || t('Former member')
  const sessionName = (group: ObjectValue, session: ObjectValue) => group.members.find((m: ObjectValue) => m.session.backend === session.backend && m.session.thread === session.thread)?.name || t('Former member')
  const tasks = (group: ObjectValue) => snapshot.tasks.filter((task: ObjectValue) => task.group_id === group.id)
  const groupNow = () => snapshot.groups.find((g: ObjectValue) => g.id === selected)
  function errorMessage(error: unknown) {
    const target = dialog?.querySelector<HTMLElement>('.teams-error') || content.querySelector<HTMLElement>('.teams-error')
    if (target) {
      const raw = error instanceof Error ? error.message : String(error)
      const messages: Record<string, string> = {
        RevisionConflict: 'This item changed. Your input is kept; reopen the editor after checking the latest changes.',
        InvalidArgument: 'Check the required fields and try again.',
        InvalidState: 'This action is unavailable in the current state. Check the group and task status first.',
        Blocked: 'Resolve outstanding tasks and deliveries before continuing.',
        GroupPaused: 'Resume the group before starting new work.',
        ResultNotReady: 'Wait for the submitting session to finish before reviewing this result.',
        ReconciliationRequired: 'Check the native execution receipt in task recovery before continuing.',
      }
      target.textContent = t(messages[raw] || raw); target.hidden = false
    }
  }
  const button = (text: string, action: () => unknown, kind = 'subtle-button') => {
    const b = label('button', text, kind); b.type = 'button'; b.dataset.focusKey = text
    b.addEventListener('click', () => {
      b.disabled = true
      void Promise.resolve().then(action).catch(errorMessage).finally(() => { if (b.isConnected) b.disabled = false })
    }); return b
  }
  const primary = (text: string, action: () => unknown) => button(text, action, 'primary-button')
  function field(parent: HTMLElement, text: string, multiline = false, value = '') {
    const wrapper = el('label', '', 'field'); wrapper.append(label('span', text))
    const input = multiline ? el('textarea') : el('input'); input.value = value
    wrapper.append(input); parent.append(wrapper); return input
  }
  function select(parent: HTMLElement, text: string, options: { value: string; text: string }[], initial = '') {
    const wrapper = el('label', '', 'field'), caption = label('span', text), input = el('select')
    caption.id = `teams-field-${crypto.randomUUID()}`; input.setAttribute('aria-labelledby', caption.id); wrapper.append(caption)
    for (const o of options) { const option = el('option', o.text); option.value = o.value; input.append(option) }
    input.value = initial || options[0]?.value || ''; wrapper.append(input); parent.append(wrapper); return input
  }
  function notice(parent: HTMLElement, text: string) { parent.append(label('p', text, 'teams-hint')) }
  function row(title: string, secondary = '') {
    const card = el('article', '', 'teams-card'), copy = el('div', '', 'teams-card-copy')
    copy.append(el('strong', title)); if (secondary) copy.append(el('p', secondary)); card.append(copy); return { card, copy }
  }
  function empty(title: string, text: string, action?: HTMLElement) {
    const box = el('div', '', 'teams-empty'); box.append(label('h3', title), label('p', text)); if (action) box.append(action); content.append(box)
  }
  function discard() { return !dirty || window.confirm(t('Discard unsaved changes?')) }
  function closeDialog(force = false) {
    if (submitting && !force) return
    if (!force && !discard()) return
    dirty = false; dialog?.close(); dialog?.remove(); dialog = null; opener?.focus()
  }
  function subform(title: string, description = '') {
    if (dialog) closeDialog(true)
    opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialog = el('dialog', '', 'dialog teams-dialog'); dialog.setAttribute('aria-label', t(title))
    const header = el('header', '', 'dialog-header'); const copy = el('div'); copy.append(label('h2', title)); if (description) copy.append(label('p', description))
    header.append(copy, button('Close', () => closeDialog()))
    const form = el('section', '', 'teams-form'); const error = el('p', '', 'teams-error form-error'); error.hidden = true; error.setAttribute('role', 'alert')
    form.append(error); dialog.append(header, form); document.body.append(dialog)
    dialog.addEventListener('input', () => { dirty = true }); dialog.addEventListener('change', () => { dirty = true })
    dialog.addEventListener('cancel', event => { event.preventDefault(); closeDialog() })
    dialog.showModal(); dirty = false; return form
  }
  async function mutate(action: () => Promise<unknown>) {
    if (submitting) return
    submitting = true
    try { await action(); closeDialog(true); await refresh() } finally { submitting = false }
  }
  const command = (group: string, command: ObjectValue) => mutate(() => request({ action: 'command', group_id: group, request_id: crypto.randomUUID(), command }))
  const version = (task: ObjectValue) => ({ task_id: task.id, expected_revision: task.revision, attempt_id: task.attempt_id })
  function tabs(parent: HTMLElement, entries: [string, string][], active: string, change: (key: string) => void, name: string) {
    const nav = el('nav', '', 'teams-tabs'); nav.setAttribute('aria-label', t(name))
    for (const [key, title] of entries) {
      const b = button(title, () => change(key), 'teams-tab'); b.setAttribute('aria-current', key === active ? 'page' : 'false'); nav.append(b)
    }
    parent.append(nav)
  }
  async function installation(backend: string, removing = false) {
    const preview = await request({ action: 'install_preview', backend })
    const form = subform(removing ? 'Remove Teams tools' : 'Install Teams tools')
    form.append(prose(String(preview.impact)), el('p', String(preview.file), 'teams-path'))
    form.append(primary(removing ? 'Confirm removal' : 'Confirm installation', () => mutate(() => request({ action: removing ? 'uninstall' : 'install', backend, file: preview.file, version: preview.version }))))
  }
  async function sessionPicker(parent: HTMLElement) {
    const backend = select(parent, 'Codex backend', snapshot.backends.map((value: string) => ({ value, text: value })))
    const thread = select(parent, 'Existing session', []); let cursor: string | null = null, generation = 0
    const more = button('Load more', () => load(true)); more.hidden = true; parent.append(more)
    const load = async (append = false) => {
      const gen = append ? generation : ++generation
      if (!append) { cursor = null; thread.replaceChildren(); more.hidden = true }
      const list = await request({ action: 'sessions', backend: backend.value, cursor })
      if (gen !== generation) return
      const bound = new Set(snapshot.groups.filter((g: ObjectValue) => !g.archived).flatMap((g: ObjectValue) => g.members.map((m: ObjectValue) => `${m.session.backend}:${m.session.thread}`)))
      for (const session of list.data || []) {
        if (bound.has(`${backend.value}:${session.id}`)) continue
        const option = el('option', `${session.name || session.preview || session.id} · ${session.cwd || ''}`); option.value = session.id; thread.append(option)
      }
      cursor = list.nextCursor || null; more.hidden = !cursor
    }
    backend.addEventListener('change', () => { void load().catch(errorMessage) }); await load(); return { backend, thread }
  }
  async function createGroup() {
    const form = subform('Create group', 'Create the group first, then add members one at a time from the group list.')
    const name = field(form, 'Group name')
    form.append(primary('Create group', () => {
      if (!name.value.trim()) throw Error('Enter a group name.')
      return mutate(async () => {
        await request({ action: 'create_group', name: name.value.trim(), members: [] })
        selected = ''; archived = false; setup = false; groupSearch = ''
      })
    }))
    name.focus()
  }
  function newTask(group: ObjectValue) {
    const form = subform('Delegate work'), owner = select(form, 'Owner', group.members.map((m: ObjectValue) => ({ value: m.id, text: m.name })), group.orchestrator || '')
    const title = field(form, 'Title'), work = field(form, 'Request / reproduction steps', true), acceptance = field(form, 'Acceptance criteria · one per line', true)
    form.append(primary('Delegate', () => {
      if (!title.value.trim() || !work.value.trim() || !acceptance.value.trim()) throw Error('Provide a title, request and acceptance criteria.')
      return command(group.id, { op: 'Delegate', delegation: { to_member_id: owner.value, title: title.value.trim(), request: work.value.trim(), acceptance: acceptance.value.split('\n').filter(s => s.trim()).map((text, i) => ({ id: `criterion-${i + 1}`, text: text.trim() })), dependencies: [], parent: null, continuation: null } })
    })); title.focus()
  }
  function editMembers(group: ObjectValue) {
    const form = subform('Edit responsibilities')
    const profiles = group.members.map((m: ObjectValue) => {
      const box = el('fieldset'); box.append(el('legend', m.name)); form.append(box)
      return { id: m.id, display: field(box, 'Display name', false, m.name), responsibility: field(box, 'Responsibility', true, m.responsibility) }
    })
    form.append(primary('Save names and responsibilities', () => command(group.id, { op: 'SetGroupProfile', expected_revision: group.revision, name: null, members: profiles.map((p: ObjectValue) => ({ member_id: p.id, display_name: p.display.value, responsibility: p.responsibility.value })) })))

  }
  async function addMember(group: ObjectValue) {
    const form = subform('Add member', group.name)
    const picker = await sessionPicker(form), name = field(form, 'Display name'), responsibility = field(form, 'Responsibility', true)
    const role = el('label', '', 'check-field')
    const owner = el('input'); owner.type = 'checkbox'; owner.disabled = Boolean(group.orchestrator)
    role.append(owner, label('span', 'Set as group orchestrator / owner')); form.append(role)
    if (group.orchestrator) form.append(el('p', `${t('Current orchestrator / owner')}: ${memberName(group, group.orchestrator)}`, 'teams-hint'))
    notice(form, 'A group has at most one orchestrator / owner. You can change it later in Members.')
    if (group.paused) notice(form, 'The group stays paused while you add members. Resume it when ready; at least two members are needed.')
    form.append(primary('Add member', () => {
      if (!picker.thread.value || !name.value.trim() || !responsibility.value.trim()) throw Error('Fill in every member and responsibility.')
      return command(group.id, { op: 'AddMember', expected_revision: group.revision,
        member: { session: { backend: picker.backend.value, thread: picker.thread.value }, name: name.value.trim(), responsibility: responsibility.value.trim() }, make_orchestrator: owner.checked })
    }))
  }
  function confirmAction(title: string, name: string, description: string, action: () => unknown) {
    const form = subform(title); form.append(el('strong', name), label('p', description))
    const actions = el('div', '', 'teams-actions'); const cancel = button('Cancel', () => closeDialog()); actions.append(cancel, button(title, action, 'subtle-button teams-danger')); form.append(actions); cancel.focus(); return form
  }
  function deleteGroup(group: ObjectValue) {
    const form = confirmAction('Delete group', group.name, 'This deletes the group and its task history. Sessions and workspace files stay intact. Delivery and command receipts are retained to prevent replay. This cannot be undone.', () => mutate(async () => {
      await request({ action: 'delete_group', group_id: group.id, expected_revision: group.revision }); selected = ''; taskId = ''; setup = false
    }))
    const unfinished = tasks(group).filter((task: ObjectValue) => !terminal(task))
    const pending = snapshot.queue.filter((q: ObjectValue) => q.scope === group.id)
    const holds = group.archived ? [] : snapshot.reservations.filter((h: ObjectValue) => group.members.some((m: ObjectValue) => m.session.backend === h.session.backend && m.session.thread === h.session.thread))
    if (!unfinished.length && tasks(group).length) form.append(button('Review task executions', () => { closeDialog(true); taskId = ''; section = 'tasks'; filter = 'all'; render() }))
    if (unfinished.length || pending.length || holds.length) {
      form.querySelector<HTMLButtonElement>('.teams-danger')!.disabled = true
      notice(form, 'Resolve the following work before deleting this group:')
      for (const task of unfinished) form.append(button(task.title, () => { closeDialog(true); openTask(task) }))
      if (pending.length || holds.length) form.append(button('Review activity', () => { closeDialog(true); section = 'activity'; taskId = ''; render() }))
    }
  }
  function groupList() {
    const controls = el('div', '', 'teams-list-controls'); const input = el('input'); input.type = 'search'; input.placeholder = t('Search groups'); input.setAttribute('aria-label', t('Search groups')); input.value = groupSearch
    controls.append(input); if (snapshot.enabled) controls.append(primary('Create group', createGroup)); content.append(controls)
    tabs(content, [['active', 'Active groups'], ['archived', 'Archived groups']], archived ? 'archived' : 'active', key => { archived = key === 'archived'; render() }, 'Group filters')
    const list = el('div', '', 'teams-list'); content.append(list)
    const fill = () => {
      list.replaceChildren()
      const groups = snapshot.groups.filter((g: ObjectValue) => Boolean(g.archived) === archived && g.name.toLowerCase().includes(groupSearch.toLowerCase()))
      if (!groups.length) { const box = el('div', '', 'teams-empty'); box.append(label('h3', groupSearch ? 'No matching groups' : archived ? 'No archived groups' : 'No groups yet'), label('p', 'Create a group to coordinate existing sessions without interrupting their work.')); if (!snapshot.enabled && !archived && !groupSearch) box.append(primary('Set up collaboration', () => { setup = true; render() })); list.append(box) }
      for (const group of groups) {
        const active = tasks(group).filter((task: ObjectValue) => !terminal(task)).length
        const { card, copy } = row('', group.members.length ? group.members.map((m: ObjectValue) => m.name).join(' · ') : t('No members yet'))
        const open = button(group.name, () => { listScroll = content.scrollTop; selected = group.id; section = 'tasks'; render(); content.scrollTop = 0 }, 'teams-row-link'); open.dataset.focusKey = group.id; copy.prepend(open); copy.querySelector('strong')?.remove()
        const meta = el('div', '', 'teams-row-meta'); const count = el('span', '', 'teams-task-count teams-hint'); count.append(label('span', 'Open tasks'), el('span', String(active))); meta.append(label('span', group.archived ? 'Archived' : group.paused ? 'Paused' : 'Group enabled', 'teams-status'), count)
        const menu = el('details', '', 'teams-row-menu'), trigger = el('summary', '⋯')
        trigger.setAttribute('aria-label', `${t('Group actions')} · ${group.name}`); trigger.title = t('Group actions')
        const options = el('div', '', 'teams-row-menu-options')
        options.append(button('Group settings', () => { selected = group.id; section = 'settings'; render() }))
        if (!group.archived) options.append(button(group.paused ? 'Resume group' : 'Pause group', () => command(group.id, { op: 'SetPaused', expected_revision: group.revision, paused: !group.paused })))
        options.append(button('Delete group', () => { menu.open = false; deleteGroup(group) }, 'subtle-button teams-danger'))
        menu.append(trigger, options); menu.addEventListener('keydown', event => { if (event.key === 'Escape') { menu.open = false; trigger.focus(); event.stopPropagation() } })
        card.append(meta); if (!group.archived && group.members.length < 5) card.append(button('Add member', () => addMember(group))); card.append(menu); list.append(card)
      }
    }
    input.addEventListener('input', () => { groupSearch = input.value; fill() }); fill()
    const footer = el('div', '', 'teams-list-footer'); footer.append(label('span', snapshot.enabled ? 'Teams enabled' : 'Teams disabled', 'teams-hint'), button('Collaboration settings', () => { setup = true; render() })); content.append(footer)
  }
  function showMembers(group: ObjectValue) {
    const actions = el('div', '', 'teams-actions')
    if (!group.archived && group.members.length) { actions.append(button('Edit responsibilities', () => editMembers(group)), button('Choose orchestrator', () => {
      const form = subform('Choose orchestrator'), owner = select(form, 'Optional orchestrator · group task owner', [{ value: '', text: t('None · peer collaboration') }, ...group.members.map((m: ObjectValue) => ({ value: m.id, text: m.name }))], group.orchestrator || '')
      form.append(primary('Save', () => command(group.id, { op: 'SetOrchestrator', expected_revision: group.revision, member_id: owner.value || null })))
    })); if (group.members.length < 5) actions.append(primary('Add member', () => addMember(group))) }
    if (!group.archived && !group.members.length) actions.append(primary('Add member', () => addMember(group)))
    content.append(actions)
    for (const member of group.members) {
      const { card, copy } = row(member.name, member.responsibility)
      if (group.orchestrator === member.id) copy.append(label('span', 'Orchestrator', 'teams-status'))
      const controls = el('div', '', 'teams-member-actions'); controls.append(button('Open session', async () => { await navigate(member.session.backend, member.session.thread); surface.close() }))
      if (!group.archived) controls.append(button('Remove member', () => confirmAction('Remove member', member.name, 'This removes the member from the group. The original session is kept. Finish its tasks and pause the group first.', () => command(group.id, { op: 'ChangeMembers', expected_revision: group.revision, add: [], remove_member_ids: [member.id] })), 'teams-quiet-action'))
      card.append(controls); content.append(card)
    }
  }
  function showTasks(group: ObjectValue) {
    tabs(content, [['open', 'Open tasks'], ['attention', 'Needs attention'], ['completed', 'Finished tasks'], ['all', 'All tasks']], filter, key => { filter = key; render() }, 'Task filters')
    const input = el('input'); input.type = 'search'; input.placeholder = t('Search tasks'); input.setAttribute('aria-label', t('Search tasks')); input.value = search; input.className = 'teams-search'; content.append(input)
    const list = el('div', '', 'teams-list'); content.append(list)
    const fill = () => {
      list.replaceChildren()
      const visible = tasks(group).filter((task: ObjectValue) => (filter === 'all' || filter === 'completed' && terminal(task) || filter === 'open' && !terminal(task) || filter === 'attention' && ['Waiting', 'Submitted'].includes(task.status)) && task.title.toLowerCase().includes(search.toLowerCase()))
      if (!visible.length) { list.append(label('p', 'No tasks in this view.', 'teams-empty')); return }
      for (const task of visible) {
        const { card, copy } = row('', memberName(group, task.assignee)); const open = button(task.title, () => openTask(task), 'teams-row-link'); open.dataset.focusKey = task.id; copy.prepend(open); copy.querySelector('strong')?.remove()
        card.append(label('span', task.status, 'teams-status')); list.append(card)
      }
    }
    input.addEventListener('input', () => { search = input.value; fill() }); fill()
  }
  function showSettings(group: ObjectValue) {
    const identity = row(group.name, t('Group name')); if (!group.archived) identity.card.append(button('Rename', () => {
      const form = subform('Rename group'); const name = field(form, 'Group name', false, group.name)
      form.append(primary('Save', () => command(group.id, { op: 'SetGroupProfile', expected_revision: group.revision, name: name.value.trim(), members: [] }))); name.focus()
    })); content.append(identity.card)
    if (!group.archived) {
      const pause = row(t(group.paused ? 'Resume group' : 'Pause group'), t('Pausing stops new deliveries without interrupting running work.')); pause.card.append(button(group.paused ? 'Resume group' : 'Pause group', () => command(group.id, { op: 'SetPaused', expected_revision: group.revision, paused: !group.paused }))); content.append(pause.card)
      const archive = row(t('Archive group'), t('Keep task history and release member sessions for other groups.')); archive.card.append(button('Archive group', () => confirmAction('Archive group', group.name, 'Pause the group and settle pending deliveries before changing members or archiving.', () => command(group.id, { op: 'ArchiveGroup', expected_revision: group.revision })))); content.append(archive.card)
    }
    const remove = row(t('Delete group'), t('Remove the group and task history. Original sessions and files are kept.')); remove.card.classList.add('teams-danger-zone'); remove.card.append(button('Delete group', () => deleteGroup(group), 'subtle-button teams-danger')); content.append(remove.card)
  }
  function showSetup() {
    content.append(button('Back to groups', () => { setup = false; selected = ''; render() }), label('h3', 'Collaboration settings'))
    const enabled = row(t(snapshot.phase === 'disabling' ? 'Teams stopping · awaiting execution receipts' : snapshot.enabled ? 'Teams enabled' : 'Teams disabled'), t('Enable collaboration, then install tools for the backends your members use.'))
    enabled.card.append(primary(snapshot.enabled ? 'Disable Teams' : 'Enable Teams', () => mutate(() => request({ action: snapshot.enabled ? 'disable' : 'enable' })))); content.append(enabled.card)
    content.append(label('h4', 'Tools and backend configuration'))
    for (const backend of snapshot.backends) {
      const registration = snapshot.registrations.find((r: ObjectValue) => r.backend === backend)
      const item = row(backend, t(registration?.installed ? 'Installed' : 'Not installed')); item.card.append(button(registration?.installed ? 'Uninstall tools' : 'Install tools', () => installation(backend, Boolean(registration?.installed)))); content.append(item.card)
    }
    if (Object.keys(snapshot.errors || {}).length) { const details = el('details'); details.append(label('summary', 'Recent diagnostics')); for (const [key, value] of Object.entries(snapshot.errors)) details.append(prose(`${key}: ${String(value)}`)); content.append(details) }
  }
  function openTask(task: ObjectValue) { taskScroll = content.scrollTop; taskId = task.id; taskSection = task.status === 'Submitted' ? 'results' : 'summary'; render(); content.scrollTop = 0 }
  function taskDetails(group: ObjectValue, task: ObjectValue) {
    content.append(button('Back to tasks', () => { taskId = ''; render(); content.scrollTop = taskScroll }))
    const header = row(task.title, memberName(group, task.assignee)); header.card.classList.add('teams-detail-heading'); header.card.append(label('span', task.status, 'teams-status')); content.append(header.card)
    tabs(content, [['summary', 'Task summary'], ['discussion', 'Discussion'], ['results', 'Results']], taskSection, key => { taskSection = key; render() }, 'Task sections')
    if (taskSection === 'summary') {
      content.append(prose(task.request), label('h4', 'Acceptance criteria')); const list = el('ul'); for (const c of task.acceptance) list.append(el('li', c.text)); content.append(list)
      if (task.checkpoint) { content.append(label('h4', 'Latest checkpoint'), prose(task.checkpoint.summary), prose(task.checkpoint.next_action)) }
      const actions = el('div', '', 'teams-actions'); actions.append(button('Recovery and retry budget', () => recovery(group, task)))
      if (!terminal(task)) actions.append(button('Cancel task', () => { const form = subform('Cancel task'); notice(form, 'Cancelling a task does not interrupt an already running session.'); const reason = field(form, 'Cancellation reason', true); form.append(button('Cancel task', () => command(group.id, { op: 'Cancel', task: version(task), reason: reason.value }), 'subtle-button teams-danger')) }))
      content.append(actions)
    } else if (taskSection === 'discussion') {
      if (!task.questions.length && !task.blockers.length) notice(content, 'No questions or blockers.')
      for (const q of task.questions) {
        const { card, copy } = row(t(q.blocking ? 'Blocking question' : 'Question'), q.text); if (q.answer) copy.append(prose(q.answer))
        else if (q.to === 'User') card.append(button('Answer', () => { const form = subform('Answer'); form.append(prose(q.text)); const answer = field(form, 'Your answer', true); form.append(primary('Send answer', () => command(group.id, { op: 'Answer', task: version(task), question_id: q.id, answer: answer.value }))) })); content.append(card)
      }
      for (const blocker of task.blockers.filter((b: ObjectValue) => !b.resolution && b.needs_user)) { const item = row(t('Needs attention'), blocker.detail); item.card.append(button('Resolve', () => { const form = subform('Resolve'); form.append(prose(blocker.detail)); const resolution = field(form, 'Resolution', true); form.append(primary('Resolve', () => command(group.id, { op: 'Resolve', task: version(task), blocker_id: blocker.id, resolution: resolution.value }))) })); content.append(item.card) }
    } else {
      const result = task.results.at(-1)
      if (!result) { notice(content, 'No result has been submitted yet.'); return }
      content.append(label('h4', 'Submitted result'), prose(result.submission.summary))
      for (const evidence of result.submission.evidence) { const item = row(t(evidence.kind), evidence.description); item.copy.append(el('span', evidence.locator, 'teams-path')); content.append(item.card) }
      const list = el('ul'); for (const outcome of result.submission.criteria) list.append(el('li', `${task.acceptance.find((c: ObjectValue) => c.id === outcome.criterion_id)?.text || outcome.criterion_id} · ${t(outcome.status)} — ${outcome.explanation}`)); content.append(list)
      if (result.submission.limitations) { content.append(label('h4', 'Limitations'), prose(result.submission.limitations)) }
      if (task.status === 'Submitted' && task.requester === 'User') content.append(primary('Review result', () => reviewResult(group, task, result)))
      if (task.reviews.length) { const history = el('details'); history.append(label('summary', 'Review history')); for (const review of task.reviews) { history.append(label('h4', review.accepted ? 'Accepted' : 'Changes requested'), prose(review.assessment.summary)); if (review.requested_changes) history.append(prose(review.requested_changes)) } content.append(history) }
    }
  }
  function reviewResult(group: ObjectValue, task: ObjectValue, result: ObjectValue) {
    const form = subform('Review result'); form.append(prose(result.submission.summary)); const notes = field(form, 'Verification summary / requested changes', true)
    const verdicts = task.acceptance.map((criterion: ObjectValue) => ({ criterion, verdict: select(form, criterion.text, [{ value: 'NotChecked', text: t('Not checked') }, { value: 'Pass', text: t('Verified · pass') }, { value: 'Fail', text: t('Verified · fail') }]) }))
    const review = (accept: boolean) => command(group.id, { op: 'Review', task: version(task), result_id: result.id, accept, assessment: { summary: notes.value, criteria: verdicts.map(({ criterion, verdict }: ObjectValue) => ({ criterion_id: criterion.id, verdict: verdict.value, explanation: notes.value, evidence: [] })) }, requested_changes: accept ? null : notes.value })
    form.append(primary('Accept verified result', () => review(true)), button('Request changes', () => review(false)))
  }

  function recovery(group: ObjectValue, task: ObjectValue) {
    const result = task.results.at(-1)
    const recovery = subform('Recovery and retry budget')
    const verification = field(recovery, 'Evidence from inspecting the native session', true)
    const evidence = () => [{ kind: 'Note', locator: `thread:${task.claim?.session.thread || ''}`, description: verification.value }]
    recovery.append(el('p', 'Use only after inspecting the native session. These actions never retry an uncertain delivery automatically.'))
    if (task.claim) recovery.append(button('Confirm previous execution has settled', () => command(group.id, { op: 'ReconcileExecution', task: version(task), claim_id: task.claim_id || task.claim.id, confirmed_settled: true, evidence: evidence() })))
    if (result) recovery.append(button('Confirm result is ready for review', () => command(group.id, { op: 'ReconcileResult', task: version(task), result_id: result.id, allow_review: true, evidence: evidence() })))
    for (const review of task.reviews.filter((r: ObjectValue) => r.turn)) recovery.append(button('Reconcile review barrier', () => command(group.id, { op: 'ReconcileReview', task: version(task), result_id: review.result_id, barrier_turn_id: review.turn.id, release: true, evidence: evidence() })))
    const attempts = field(recovery, 'Additional rework attempts', false, '1'); attempts.setAttribute('type', 'number'); attempts.setAttribute('min', '1'); attempts.setAttribute('max', '10')
    recovery.append(button('Grant rework attempts', () => command(group.id, { op: 'GrantReworkBudget', task: version(task), additional_attempts: Number(attempts.value), reason: verification.value })))
  }
  function showActivity(group: ObjectValue) {
    const queue = snapshot.queue.filter((q: ObjectValue) => q.scope === group.id)
    const belongs = (item: ObjectValue) => !group.archived && group.members.some((m: ObjectValue) => m.session.backend === item.session.backend && m.session.thread === item.session.thread)
    const dispatches = (snapshot.user_dispatches || []).filter(belongs), holds = snapshot.reservations.filter(belongs)
    if (!queue.length && !dispatches.length && !holds.length) { empty('No pending activity', 'Deliveries and reservations that need attention appear here.'); return }
    for (const q of queue) {
      const { card, copy } = row(sessionName(group, q.session), t(q.state))
      const details = el('details'); details.append(label('summary', 'Receipt details'), el('p', q.stable_id, 'teams-path')); copy.append(details)
      const actions = el('div', '', 'teams-member-actions')
      if (['Unknown', 'NativeQueued', 'Started', 'Abandoned'].includes(q.state)) actions.append(button('Check native receipt', async () => { const result = await request({ action: 'reconcile_delivery', entry_id: q.id }); await refresh(); if (result.message) errorMessage(result.message) }))
      if (q.state === 'Unknown') actions.append(button('Record unresolved delivery', () => {
        const form = subform('Record unresolved delivery'); notice(form, 'Pause the group first. This records an unresolved delivery; it does not prove that Codex did not execute it. Strict group resume remains blocked until native receipts settle.')
        const reason = field(form, 'What you inspected and why the receipt is unresolved', true)
        form.append(primary('Record without retrying', () => mutate(() => request({ action: 'abandon_delivery', entry_id: q.id, reason: reason.value }))))
      })); card.append(actions); content.append(card)
    }
    for (const dispatch of dispatches) {
      const item = row(sessionName(group, dispatch.session), t('Uncertain user submission'))
      item.card.append(button('Check native receipt', async () => { const result = await request({ action: 'reconcile_submission', session: dispatch.session, id: dispatch.id }); await refresh(); if (!result.found) errorMessage(result.message || t('No positive native receipt was found.')) })); content.append(item.card)
    }
    for (const hold of holds) {
      const item = row(sessionName(group, hold.session), hold.reason)
      item.card.append(button('Release after checking session', () => confirmAction('Reconcile reservation', sessionName(group, hold.session), 'Only release after verifying that no sender or supervision job still owns this session. A failed request may already have executed.', () => mutate(() => client.release(hold.id))))); content.append(item.card)
    }
  }
  function render() {
    const top = content.scrollTop
    const focus = content.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.focusKey : ''
    content.replaceChildren()
    const error = el('p', '', 'teams-error form-error'); error.hidden = true; error.setAttribute('role', 'alert'); content.append(error)
    if (setup) showSetup()
    else if (!selected || !groupNow()) { selected = ''; taskId = ''; groupList() }
    else {
      const group = groupNow()!
      if (taskId) {
        const task = tasks(group).find((task: ObjectValue) => task.id === taskId)
        if (task) taskDetails(group, task)
        else { taskId = ''; render(); return }
      } else {
        content.append(button('Back to groups', () => { selected = ''; render(); content.scrollTop = listScroll }))
        const heading = row(group.name, `${group.members.length} ${t('Members')} · ${t(group.archived ? 'Archived' : group.paused ? 'Paused' : 'Group enabled')}`); heading.card.classList.add('teams-detail-heading')
        if (section === 'tasks' && !group.archived && !group.paused && snapshot.enabled) heading.card.append(primary('Delegate work', () => newTask(group)))
        content.append(heading.card)
        tabs(content, [['tasks', 'Tasks'], ['members', 'Members'], ['activity', 'Activity'], ['settings', 'Group settings']], section, key => { section = key; render(); content.scrollTop = 0 }, 'Group sections')
        if (section === 'tasks') showTasks(group)
        else if (section === 'members') showMembers(group)
        else if (section === 'activity') showActivity(group)
        else showSettings(group)
      }
    }
    content.scrollTop = top
    if (focus) Array.from(content.querySelectorAll<HTMLElement>('[data-focus-key]')).find(node => node.dataset.focusKey === focus)?.focus({ preventScroll: true })
  }
  async function refresh() {
    if (loading) return
    loading = true
    try { snapshot = await request({ action: 'snapshot' }); if (!dialog) render() } finally { loading = false }
  }
  async function open() {
    surface.activate(); selected = ''; taskId = ''; setup = false
    if (!snapshot.groups.length) { content.replaceChildren(label('p', 'Loading groups…', 'teams-empty')) }
    try { await refresh() } catch (error) {
      content.replaceChildren(label('p', 'Unable to load groups.', 'teams-empty'), button('Retry', refresh)); const msg = el('p', '', 'teams-error form-error'); content.append(msg); errorMessage(error)
    }
    if (timer) clearInterval(timer)
    timer = setInterval(() => {
      if (content.closest('.hidden') || dialog || content.querySelector('input:focus,textarea:focus,select:focus,.teams-row-menu[open]')) return
      void refresh().catch(errorMessage)
    }, 5000)
  }
  function close() { surface.close(); if (timer) clearInterval(timer) }
  return { open, close, refresh }
}
