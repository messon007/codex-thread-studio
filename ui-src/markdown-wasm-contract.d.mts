declare module '*vendor/comrak.js' {
  export default function initialize(): Promise<unknown>
  export function parse_document(source: string): { take_html(): string; headings_json(): string; free(): void }
}
