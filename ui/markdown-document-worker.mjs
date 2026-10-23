import { parseMarkdownDocument } from './markdown-document-parser.mjs'

self.onmessage = async ({ data }) => {
  try {
    self.postMessage({ result: await parseMarkdownDocument(data.source) })
  } catch (error) {
    self.postMessage({ error: error?.message || String(error) })
  }
}
