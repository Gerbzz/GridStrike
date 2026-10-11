"""Create two small, self-authored XLSX cache-boundary fixtures (development tool)."""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parent
WORKBOOK = '''<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
 xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
 <sheets><sheet name="Sales" sheetId="1" r:id="rId1"/></sheets>
</workbook>'''
RELS = '''<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>'''
ROOT_RELS = '''<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>'''
TYPES = '''<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="xml" ContentType="application/xml"/>
 <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
 <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>'''

def make(path: Path, cached: bool) -> None:
    result = '<v>93</v>' if cached else ''
    sheet = f'''<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
 <row r="1"><c r="A1" t="inlineStr"><is><t>Month</t></is></c><c r="B1" t="inlineStr"><is><t>Units</t></is></c><c r="C1" t="inlineStr"><is><t>Price</t></is></c><c r="D1" t="inlineStr"><is><t>Revenue</t></is></c></row>
 <row r="2"><c r="A2" t="inlineStr"><is><t>Jan</t></is></c><c r="B2"><v>10</v></c><c r="C2"><v>2.5</v></c><c r="D2"><f>B2*C2</f><v>25</v></c></row>
 <row r="3"><c r="A3" t="inlineStr"><is><t>Feb</t></is></c><c r="B3"><v>12</v></c><c r="C3"><v>3</v></c><c r="D3"><f>B3*C3</f><v>36</v></c></row>
 <row r="4"><c r="A4" t="inlineStr"><is><t>Mar</t></is></c><c r="B4"><v>8</v></c><c r="C4"><v>4</v></c><c r="D4"><f>B4*C4</f><v>32</v></c></row>
 <row r="5"><c r="D5"><f>SUM(D2:D4)</f>{result}</c></row>
</sheetData></worksheet>'''
    with ZipFile(path, "w", ZIP_DEFLATED) as archive:
        archive.writestr("[Content_Types].xml", TYPES)
        archive.writestr("_rels/.rels", ROOT_RELS)
        archive.writestr("xl/workbook.xml", WORKBOOK)
        archive.writestr("xl/_rels/workbook.xml.rels", RELS)
        archive.writestr("xl/worksheets/sheet1.xml", sheet)

make(ROOT / "cached-sales.xlsx", True)
make(ROOT / "missing-cache-sales.xlsx", False)
