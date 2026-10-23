// Generated from ui-src; run npm run build:ui. Do not edit.
import { Marked } from './vendor/marked.esm.js';
const markdownOptions = { async: false, breaks: true, gfm: true };
const plainMarkdownParser = new Marked(markdownOptions);
const plantumlMarkdownParser = new Marked(markdownOptions, { extensions: [plantumlMarkdownExtension()] });
export function markdownParserForSource(source) {
    return /@startuml\b/i.test(source) ? plantumlMarkdownParser : plainMarkdownParser;
}
export function diagramKind(language, source) {
    const normalized = language.toLowerCase();
    if (normalized === 'mermaid')
        return 'mermaid';
    if (['plantuml', 'puml', 'uml'].includes(normalized))
        return 'plantuml';
    if (normalized === 'd2')
        return 'd2';
    if (normalized === 'code' && /^@startuml\b/im.test(source))
        return 'plantuml';
    return null;
}
export function plantumlLines(source) {
    const trimmed = source.trim();
    if (/^@startuml\b/im.test(trimmed))
        return trimmed.split(/\r\n|\r|\n/);
    return ['@startuml', ...trimmed.split(/\r\n|\r|\n/), '@enduml'];
}
export function hasUnsafePlantumlDirective(source) {
    return /^\s*!(?:include\w*|import|theme)\b|%\s*load_(?:json|yaml|data)\s*\(/im.test(source);
}
export function plantumlMarkdownExtension() {
    return {
        name: 'plantumlBlock',
        level: 'block',
        start(source) {
            const match = /(?:^|\n)(@startuml\b)/im.exec(source);
            return match ? match.index + match[0].length - (match[1]?.length || 0) : undefined;
        },
        tokenizer(source) {
            if (!/^@startuml\b/i.test(source))
                return undefined;
            const match = /^@startuml\b[^\r\n]*(?:\r?\n|\r)[\s\S]*?^@enduml\b[^\r\n]*(?:\r?\n|\r|$)/im.exec(source);
            if (!match)
                return undefined;
            return { type: 'plantumlBlock', raw: match[0], text: match[0].trimEnd() };
        },
        renderer(token) {
            const escaped = token.text.replace(/[&<>"']/g, (character) => ({
                '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
            })[character] || character);
            return `<pre><code class="language-plantuml">${escaped}</code></pre>`;
        },
    };
}
