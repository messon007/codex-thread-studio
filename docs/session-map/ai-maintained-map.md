# AI-maintained session maps

Session Map belongs to one source conversation, not the Router's aggregate view.

- Creating a map does not require a written goal. A blank goal lets AI infer the
  scope from the opening and recent conversation, then generate the outline.
- The map menu offers AI generation/update and confirmed delete. Manual add/edit
  node and edit-goal dialogs have been removed.
- Node menus offer only **Set current** and **Done**. Clicking a node also selects
  it as current. New nodes default to incomplete (`notStarted`).
- At most one visible node is current. Selecting another node resets the previous
  current node to incomplete. Completing the current node clears the pointer.
  A user can select a completed node as current to reopen it.
- AI cannot mark a node completed or reopen a completed node automatically.
  Existing visited/paused states are presented as incomplete; completed states
  are retained. Read, write, undo and frontend normalization enforce a consistent
  current pointer and active state.
- Creation/explicit AI update uses a temporary model worker. Existing automatic
  synchronization reuses conversation turns; it does not add a monitoring loop.

AI generation currently supports Codex-family backends, including EPT Codex.
OpenCode AI generation is not implemented. Existing OpenCode maps remain readable
and their current/completed states can be changed; new AI maps are not offered
as if that capability were available.

Validation covers default states, exclusive current selection, completion and
reopening, protected user completion, removed editor menus, optional goal input,
and real authenticated SQLite endpoints in the isolated browser smoke profile.
These checks do not measure the quality of a real model-generated learning outline.
