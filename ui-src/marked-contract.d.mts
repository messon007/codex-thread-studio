// Only the lexer surface used by the checked outline extractor.
declare module '*vendor/marked.esm.js' {
  interface Token { type?: string; raw?: string; text?: string; depth?: number; tokens?: Token[] }
  export const marked: { lexer(source: string): Token[] }
}
