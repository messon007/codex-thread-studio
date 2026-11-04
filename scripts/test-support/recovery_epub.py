"""Extend the private three-chapter reader fixture without changing its CFIs."""
import zipfile
import xml.etree.ElementTree as ET


def extend_search_book(path, count=140):
    with zipfile.ZipFile(path) as archive:
        files = {name: archive.read(name) for name in archive.namelist()}
    package = ET.fromstring(files['OPS/package.opf'])
    ns = 'http://www.idpf.org/2007/opf'
    manifest, spine = package.find('{'+ns+'}manifest'), package.find('{'+ns+'}spine')
    assert len(spine) == 3 and count == 140
    nav = ET.fromstring(files['OPS/nav.xhtml'])
    html = 'http://www.w3.org/1999/xhtml'
    contents = nav.find('.//{'+html+'}ol')
    for index in range(3, count):
        identity = f'additional{index}'
        href = f'Text/additional{index}.xhtml'
        ET.SubElement(manifest, '{'+ns+'}item', id=identity, href=href,
                      attrib={'media-type':'application/xhtml+xml'})
        ET.SubElement(spine, '{'+ns+'}itemref', idref=identity)
        item = ET.SubElement(contents, '{'+html+'}li')
        ET.SubElement(item, '{'+html+'}a', href=href).text = f'Chapter {index+1}'
        files['OPS/'+href] = (f'<html xmlns="{html}"><body><h1>Chapter {index+1}</h1>'
                             '<p>Needle · Private search fixture 中文😀.</p></body></html>').encode()
    files['OPS/package.opf'] = ET.tostring(package)
    files['OPS/nav.xhtml'] = ET.tostring(nav)
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('mimetype', files.pop('mimetype'), compress_type=zipfile.ZIP_STORED)
        for name, content in files.items():
            archive.writestr(name, content)


def add_supplement(path):
    """Non-spine XHTML with two successive fragment links and distinct anchors."""
    with zipfile.ZipFile(path) as archive:
        files = {name: archive.read(name) for name in archive.namelist()}
    ns = 'http://www.idpf.org/2007/opf'
    html = 'http://www.w3.org/1999/xhtml'
    package = ET.fromstring(files['OPS/package.opf'])
    manifest = package.find('{'+ns+'}manifest')
    href = 'Text/Supplement%20notes.xhtml'
    ET.SubElement(manifest, '{'+ns+'}item', id='supplement', href=href,
                  attrib={'media-type':'application/xhtml+xml'})
    nav = ET.fromstring(files['OPS/nav.xhtml'])
    item = ET.SubElement(nav.find('.//{'+html+'}ol'), '{'+html+'}li')
    ET.SubElement(item, '{'+html+'}a', href=href+'#s35').text = 'Supplement 中文😀'
    chapter = files['OPS/Text/two.xhtml'].decode()
    chapter = chapter.replace('</h1>', '</h1><p><a id="supplement-link" href="Supplement%20notes.xhtml#s35">Supplement 中文😀</a></p>', 1)
    files['OPS/Text/two.xhtml'] = chapter.encode()
    paragraphs = ''.join(f'<p id="s{n}">Supplement paragraph {n}. Private non-spine content 中文😀. '
                         'A second fragment must remain selected across gateway replacement.</p>' for n in range(1,101))
    files['OPS/Text/Supplement notes.xhtml'] = (f'<html xmlns="{html}"><body>'
        '<h1>Supplement 中文😀</h1><p><a id="second-fragment" href="#s70">Later supplement passage</a></p>'
        + paragraphs + '</body></html>').encode()
    files['OPS/package.opf'] = ET.tostring(package)
    files['OPS/nav.xhtml'] = ET.tostring(nav)
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('mimetype', files.pop('mimetype'), compress_type=zipfile.ZIP_STORED)
        for name, content in files.items():
            archive.writestr(name, content)
