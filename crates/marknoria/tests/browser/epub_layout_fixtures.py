"""Private generated book covering long listings and continuous spine boundaries."""
import base64
import json
import pathlib
import re
import sys
import zipfile
from epub_fixtures import entries

root = pathlib.Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=True)
png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a2ioAAAAASUVORK5CYII=')
content = entries(png)
names = [f'listing-{i}' for i in range(8)]
manifest = '<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="style" href="style.css" media-type="text/css"/>' + ''.join(f'<item id="c{i}" href="{name}.xhtml" media-type="application/xhtml+xml"/>' for i, name in enumerate(names)) + '</manifest>'
spine = '<spine>' + ''.join(f'<itemref idref="c{i}"/>' for i in range(len(names))) + '</spine>'
content['OPS/book.opf'] = re.sub(r'<manifest>[\s\S]*?</manifest>', manifest, content['OPS/book.opf'])
content['OPS/book.opf'] = re.sub(r'<spine>[\s\S]*?</spine>', spine, content['OPS/book.opf'])
links = ''.join(f'<li><a href="{name}.xhtml#start">Listing chapter {i}</a></li>' for i, name in enumerate(names))
content['OPS/nav.xhtml'] = '<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc" id="toc"><ol>' + links + '</ol></nav></body></html>'
content['OPS/style.css'] = '.book{display:block}.programlisting{display:block;font-family:monospace;font-size:.75em;line-height:normal;white-space:pre-wrap;margin:8px 0} .example-title{padding:4px 8px;background:silver} p{line-height:1.7}'
for i, name in enumerate(names):
    listing = '\n'.join(f'  auto line_{n:03} = value_{n:03}; // chapter {i} 中文😀' for n in range(100))
    content[f'OPS/{name}.xhtml'] = '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Listing chapter</title><link rel="stylesheet" href="style.css"/></head><body><h1 id="start">Listing chapter ' + str(i) + '</h1><p>Before the complete listing.</p><div class="example"><div class="example-title">A long complete code example</div><div class="book"><pre class="programlisting">int main()\n{\n' + listing + '\n}</pre></div></div><p id="after">AFTER_LISTING_' + str(i) + '</p></body></html>'
content = {name: value.encode() if isinstance(value, str) else value for name, value in content.items()}
path = root / 'long-listings.epub'
with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
    for name, value in content.items():
        archive.writestr(name, value)
print(json.dumps({'book': str(path), 'package': 'OPS/book.opf', 'chapter': 'OPS/listing-0.xhtml', 'entries': {name: base64.b64encode(value).decode() for name, value in content.items()}}))
