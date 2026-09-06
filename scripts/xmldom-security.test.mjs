import test from 'node:test'
import assert from 'node:assert/strict'
import { DOMImplementation, DOMParser, XMLSerializer } from '@xmldom/xmldom'

test('xmldom rejects invalid entity names and strict serialization of mutated names', () => {
  const document = new DOMImplementation().createDocument(null, 'root', null)
  const serializer = new XMLSerializer()
  for (const name of ['safe; <injected/> &x', 'x<injected', 'x y']) {
    assert.throws(() => document.createEntityReference(name))
    const reference = document.createEntityReference('safe')
    reference.nodeName = name
    // The 0.8 API takes serialization options as the fourth argument.
    assert.throws(() => serializer.serializeToString(reference, false, null, { requireWellFormed: true }))
  }
  const valid = document.createEntityReference('valid')
  assert.equal(serializer.serializeToString(valid, false, null, { requireWellFormed: true }), '&valid;')
})

test('xmldom retains EPUB namespaces, Unicode and escaped content when round-tripped', () => {
  const xml = '<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>中文 &amp; English</dc:title></metadata><manifest><item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest></package>'
  const parser = new DOMParser()
  const document = parser.parseFromString(xml, 'application/xml')
  const serialized = new XMLSerializer().serializeToString(document)
  const restored = parser.parseFromString(serialized, 'application/xml')
  assert.equal(restored.documentElement.namespaceURI, 'http://www.idpf.org/2007/opf')
  assert.equal(restored.getElementsByTagNameNS('http://purl.org/dc/elements/1.1/', 'title')[0].textContent, '中文 & English')
  assert.equal(restored.getElementsByTagNameNS('http://www.idpf.org/2007/opf', 'item')[0].getAttribute('href'), 'chapter.xhtml')
})
