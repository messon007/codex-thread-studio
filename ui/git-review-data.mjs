// Generated from ui-src; run npm run build:ui. Do not edit.
export function parseUnifiedDiff(source) {
    let oldLine = null;
    let newLine = null;
    return String(source || '').replace(/\r\n?/gu, '\n').split('\n').map((text) => {
        const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/u.exec(text);
        if (hunk) {
            oldLine = Number(hunk[1]);
            newLine = Number(hunk[2]);
            return { kind: 'hunk', oldLine: null, newLine: null, text };
        }
        if (oldLine == null || /^(?:diff --git|index |--- |\+\+\+ )/u.test(text)) {
            return { kind: 'meta', oldLine: null, newLine: null, text };
        }
        if (text.startsWith('+')) {
            const row = { kind: 'addition', oldLine: null, newLine, text: text.slice(1) };
            newLine = (newLine ?? 0) + 1;
            return row;
        }
        if (text.startsWith('-')) {
            const row = { kind: 'deletion', oldLine, newLine: null, text: text.slice(1) };
            oldLine += 1;
            return row;
        }
        if (text.startsWith(' ')) {
            const row = { kind: 'context', oldLine, newLine, text: text.slice(1) };
            oldLine += 1;
            newLine = (newLine ?? 0) + 1;
            return row;
        }
        return { kind: 'note', oldLine: null, newLine: null, text };
    });
}
export function reviewFileStatus(file) {
    if (file?.conflicted)
        return { label: '!', title: 'Conflict', tone: 'conflict' };
    if (file?.untracked)
        return { label: 'U', title: 'Untracked', tone: 'untracked' };
    const code = file?.worktreeStatus?.trim() || file?.indexStatus?.trim() || 'M';
    const labels = { A: 'Added', D: 'Delete', M: 'Modified', R: 'Rename', C: 'Copy', T: 'Type changed' };
    return { label: code, title: labels[code] || 'Changed', tone: code.toLowerCase() };
}
export function visibleReviewFiles(files, scope = 'all', filter = '') {
    const query = String(filter || '').trim().toLowerCase();
    return (files || []).filter((file) => {
        if (scope === 'staged' && !file.staged)
            return false;
        if (scope === 'unstaged' && !file.unstaged)
            return false;
        return !query || file.path.toLowerCase().includes(query) || String(file.previousPath || '').toLowerCase().includes(query);
    });
}
// Coalesce only concurrent reads; completed results are never cached because
// file contents can change without changing Git's porcelain status.
export function createPendingGitReads() {
    const pending = new Map();
    return (key, read) => {
        if (pending.has(key))
            return pending.get(key);
        const promise = Promise.resolve().then(read).finally(() => {
            if (pending.get(key) === promise)
                pending.delete(key);
        });
        pending.set(key, promise);
        return promise;
    };
}
