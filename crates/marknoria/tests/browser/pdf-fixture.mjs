export function pdfFixture() {
// Deterministic public 12-page PDF, including an outline and a rotated page.
const objects = [Buffer.from('<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>'),
  Buffer.from('<< /Type /Pages /Kids [' + Array.from({ length: 12 }, (_, n) => `${7 + n * 2} 0 R`).join(' ') + '] /Count 12 >>'),
  Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'),
  Buffer.from('<< /Type /Outlines /First 5 0 R /Last 6 0 R /Count 2 >>'),
  Buffer.from('<< /Title (First chapter) /Parent 4 0 R /Next 6 0 R /Dest [7 0 R /Fit] >>'),
  Buffer.from('<< /Title (Last chapter) /Parent 4 0 R /Prev 5 0 R /Dest [29 0 R /Fit] >>')]
for (let n = 0; n < 12; n++) {
  objects.push(Buffer.from(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] ${n === 9 ? '/Rotate 90 ' : ''}/Resources << /Font << /F1 3 0 R >> >> /Contents ${8 + n * 2} 0 R >>`))
  const content = `BT /F1 24 Tf 40 700 Td (PDF sentinel page ${n + 1}) Tj ET`
  objects.push(Buffer.from(`<< /Length ${content.length} >>\nstream\n${content}\nendstream`))
}
let fixture = Buffer.from('%PDF-1.4\n'), offsets = [0]
for (const [i, object] of objects.entries()) { offsets.push(fixture.length); fixture = Buffer.concat([fixture, Buffer.from(`${i + 1} 0 obj\n`), object, Buffer.from('\nendobj\n')]) }
const xref = fixture.length
fixture = Buffer.concat([fixture, Buffer.from(`xref\n0 ${objects.length + 1}\n0000000000 65535 f \n` + offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('') + `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`)])
return fixture
}
