export function pdfFixture({ search = false } = {}) {
// Deterministic public 12-page PDF, including an outline and a rotated page.
const objects = [Buffer.from('<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>'),
  Buffer.from('<< /Type /Pages /Kids [' + Array.from({ length: 12 }, (_, n) => `${7 + n * 2} 0 R`).join(' ') + '] /Count 12 >>'),
  Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 31 0 R >>'),
  Buffer.from('<< /Type /Outlines /First 5 0 R /Last 6 0 R /Count 2 >>'),
  Buffer.from('<< /Title (First chapter) /Parent 4 0 R /Next 6 0 R /Dest [7 0 R /Fit] >>'),
  Buffer.from('<< /Title (Last chapter) /Parent 4 0 R /Prev 5 0 R /Dest [29 0 R /Fit] >>')]
for (let n = 0; n < 12; n++) {
  objects.push(Buffer.from(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] ${n === 9 ? '/Rotate 90 ' : ''}/Resources << /Font << /F1 3 0 R >> >> /Contents ${8 + n * 2} 0 R >>`))
  const content = search && n === 0
    ? 'BT /F1 24 Tf 40 700 Td (PDF sentinel page 1 \\200 NEEDLE) Tj 0 -620 Td (NEEDLE) Tj ET'
    : `BT /F1 24 Tf 40 700 Td (PDF sentinel page ${n + 1}) Tj ET`
  objects.push(Buffer.from(`<< /Length ${content.length} >>\nstream\n${content}\nendstream`))
}
const cmap = '/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /SearchUnicode def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfrange <00> <7F> <0000> endbfrange 1 beginbfchar <80> <0130> endbfchar endcmap CMapName currentdict /CMap defineresource pop end end'
objects.push(Buffer.from(`<< /Length ${cmap.length} >>\nstream\n${cmap}\nendstream`))
let fixture = Buffer.from('%PDF-1.4\n'), offsets = [0]
for (const [i, object] of objects.entries()) { offsets.push(fixture.length); fixture = Buffer.concat([fixture, Buffer.from(`${i + 1} 0 obj\n`), object, Buffer.from('\nendobj\n')]) }
const xref = fixture.length
fixture = Buffer.concat([fixture, Buffer.from(`xref\n0 ${objects.length + 1}\n0000000000 65535 f \n` + offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('') + `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`)])
return fixture
}
