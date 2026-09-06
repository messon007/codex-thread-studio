// Generated from ui-src; run npm run build:ui. Do not edit.
export function turnPromptPreview(turn, maxLength = 72) {
    const userMessage = (turn?.items || []).find((item) => item?.type === 'userMessage');
    const content = Array.isArray(userMessage?.content) ? userMessage.content : [];
    const textParts = content.filter((item) => item?.type === 'text').map((item) => item.text || '');
    return compactPromptParts(textParts, maxLength);
}
export function turnHasUserInput(turn) {
    return (turn?.items || []).some((item) => item?.type === 'userMessage');
}
export function navigableTurns(turns) {
    return (turns || []).filter(turnHasUserInput);
}
export function activeTurnAtMarker(positions, marker, atBottom = false) {
    const available = (positions || []).filter((position) => position?.id);
    if (!available.length)
        return null;
    if (atBottom)
        return available.at(-1).id;
    let active = available[0];
    for (const position of available) {
        if (position.top > marker)
            break;
        active = position;
    }
    return active.id;
}
export function turnNavigationLabel(turn, index) {
    const preview = turnPromptPreview(turn);
    return preview ? `User input ${index + 1}: ${preview}` : `User input ${index + 1}`;
}
function compactPromptParts(parts, limit) {
    const maximum = Math.max(1, Number.isFinite(limit) ? Math.floor(limit) : 72);
    const characters = [];
    let pendingSpace = false;
    let truncated = false;
    outer: for (const part of parts) {
        if (characters.length)
            pendingSpace = true;
        for (const character of String(part || '')) {
            if (/\s/u.test(character)) {
                if (characters.length)
                    pendingSpace = true;
                continue;
            }
            if (pendingSpace) {
                if (characters.length >= maximum) {
                    truncated = true;
                    break outer;
                }
                characters.push(' ');
                pendingSpace = false;
            }
            if (characters.length >= maximum) {
                truncated = true;
                break outer;
            }
            characters.push(character);
        }
    }
    if (!truncated)
        return characters.join('').trim();
    if (maximum === 1)
        return '…';
    return `${characters.slice(0, maximum - 1).join('').trimEnd()}…`;
}
