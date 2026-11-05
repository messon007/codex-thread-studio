"""Synthetic EPUB source entries; generated containers stay in private test dirs."""
def entries(png):
    return {
        'mimetype': 'application/epub+zip',
        'META-INF/container.xml': '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>',
        'OPS/book.opf': '''<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="book-id">marknoria-public-fixture</dc:identifier><dc:title>Public Book 中文</dc:title><dc:creator>Public Author</dc:creator><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="a" href="one.xhtml" media-type="application/xhtml+xml"/><item id="b" href="two.xhtml" media-type="application/xhtml+xml"/><item id="cover" href="cover.png" media-type="image/png"/><item id="style" href="style.css" media-type="text/css"/></manifest><spine><itemref idref="a"/><itemref idref="b"/></spine></package>''',
        'OPS/nav.xhtml': '''<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc" id="toc"><ol><li><a href="one.xhtml#start">First chapter</a></li><li><a href="two.xhtml#target">Second chapter</a></li></ol></nav></body></html>''',
        'OPS/one.xhtml': '''<!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml"><head><title>First chapter</title><link rel="stylesheet" type="text/css" href="style.css"/></head><body><script>parent.compromised=true</script><h1 id="start">First chapter</h1><p contenteditable="true">EPUB sentinel one</p><img src="cover.png" alt="EPUB image"/><p><a href="two.xhtml#target">Second chapter link</a></p><p><a href="https://forbidden.invalid/">External link</a></p>''' + ''.join(f'<p>Public pagination paragraph {n}. Readable text 中文😀.</p>' for n in range(40)) + '</body></html>',
        'OPS/two.xhtml': '''<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Second chapter</title><link rel="stylesheet" type="text/css" href="style.css"/></head><body><h1>Second chapter</h1><p id="target">EPUB sentinel two 中文😀.</p><table><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody><tr><td>first</td><td>001</td></tr><tr><td>second</td><td>002</td></tr></tbody></table></body></html>''',
        'OPS/style.css': 'p { line-height:1.7 } img { width:32px; height:32px } table { border-collapse:collapse } td,th { border:1px solid #ccc; padding:4px }',
        'OPS/cover.png': png,
    }

def progress_entries(png):
    """One corpus for the component server and the actual ZIP/Rust service."""
    import re
    original = entries(png)
    names = ['one' if i == 0 else 'two' if i == 1 else f'chapter-{i}' for i in range(64)]
    manifest = '<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>' + ''.join(f'<item id="c{i}" href="{name}.xhtml" media-type="application/xhtml+xml"/>' for i, name in enumerate(names)) + '</manifest>'
    spine = '<spine>' + ''.join(f'<itemref idref="c{i}"/>' for i in range(64)) + '</spine>'
    package = re.sub(r'<manifest>[\s\S]*?</manifest>', manifest, original['OPS/book.opf'])
    package = re.sub(r'<spine>[\s\S]*?</spine>', spine, package)
    result = {name: original[name] for name in ['mimetype', 'META-INF/container.xml', 'OPS/nav.xhtml']}
    result['OPS/book.opf'] = package
    paragraphs = ''.join('<p>' + 'Readable progress corpus text. ' * 16 + '</p>' for _ in range(128))
    for i, name in enumerate(names):
        result[f'OPS/{name}.xhtml'] = '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Progress corpus</title></head><body><h1>Chapter ' + str(i) + '</h1>' + paragraphs + '</body></html>'
    return {name: value.encode('utf-8') if isinstance(value, str) else value for name, value in result.items()}

if __name__ == '__main__':
    import base64, json, pathlib, sys, zipfile
    root = pathlib.Path(sys.argv[1]); root.mkdir(parents=True, exist_ok=True)
    png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a2ioAAAAASUVORK5CYII=')
    content = {name: value.encode('utf-8') if isinstance(value, str) else value for name, value in entries(png).items()}
    path = root / 'public.epub'
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, value in content.items():
            archive.writestr(name, value)
    payload = {'book': str(path), 'entries': {name: base64.b64encode(value).decode() for name, value in content.items()}}
    if '--corpus' in sys.argv:
        corpus = progress_entries(png)
        corpus_path = root / 'progress-corpus.epub'
        with zipfile.ZipFile(corpus_path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, value in corpus.items():
                archive.writestr(name, value)
        payload.update(corpusBook=str(corpus_path), corpusEntries={name: base64.b64encode(value).decode() for name, value in corpus.items()})
    print(json.dumps(payload))
