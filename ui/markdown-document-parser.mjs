// Generated from ui-src; run npm run build:ui. Do not edit.
import { markdownParserForSource } from './diagram-source.mjs';
import { extractMarkdownOutline, outlineFromMarkdownHeadings } from './document-outline.mjs';
export async function parseMarkdownDocument(source, loadWasm = () => import('./vendor/comrak.js')) {
    if (/@startuml\b/i.test(source)) {
        const parser = markdownParserForSource(source);
        const tokens = parser.lexer(source);
        return { html: parser.parser(tokens), outline: extractMarkdownOutline(source, tokens), engine: 'marked' };
    }
    const wasm = await loadWasm();
    await wasm.default();
    const document = wasm.parse_document(source);
    try {
        return { html: document.take_html(), outline: outlineFromMarkdownHeadings(JSON.parse(document.headings_json())), engine: 'comrak-wasm' };
    }
    finally {
        document.free();
    }
}
