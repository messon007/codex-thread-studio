"""Private, bounded three-worksheet fixture for native recovery acceptance."""
import zipfile
from xml.sax.saxutils import escape


def workbook(path):
    ns = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
    rel = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
    names = ['First 中文', 'Second', 'Review']
    files = {
        '[Content_Types].xml':'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>',
        '_rels/.rels':'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        'xl/workbook.xml':f'<workbook xmlns="{ns}" xmlns:r="{rel}"><sheets>'+''.join(f'<sheet name="{escape(name)}" sheetId="{i+1}" r:id="rId{i+1}"/>' for i,name in enumerate(names))+'</sheets></workbook>',
        'xl/_rels/workbook.xml.rels':'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'+''.join(f'<Relationship Id="rId{i+1}" Type="{rel}/worksheet" Target="worksheets/sheet{i+1}.xml"/>' for i in range(3))+'</Relationships>',
    }
    def text(cell, value):
        return f'<c r="{cell}" t="inlineStr"><is><t>{escape(value)}</t></is></c>'
    for sheet in range(3):
        rows=['<row r="1">'+''.join(text(col+'1',name) for col,name in zip('ABC',['n','label','note']))+'</row>']
        for n in range(150):
            row=n+2;value=sheet*1000+n
            rows.append(f'<row r="{row}"><c r="A{row}"><v>{value}</v></c>'+text(f'B{row}',f'Sheet {sheet+1} row {n} 中文😀')+text(f'C{row}',f'Long native workbook note {value} for wrapping and column widths')+'</row>')
        files[f'xl/worksheets/sheet{sheet+1}.xml']=f'<worksheet xmlns="{ns}"><dimension ref="A1:C151"/><sheetData>'+''.join(rows)+'</sheetData></worksheet>'
    with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED) as archive:
        for name,body in files.items():archive.writestr(name,body)
