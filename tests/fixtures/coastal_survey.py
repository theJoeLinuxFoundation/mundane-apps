"""Regenerate the suite's original two-page, vector-and-text PDF fixture."""
from pathlib import Path
import sys

with_outline = len(sys.argv) == 2

objects = []
def add(data):
    objects.append(data.encode("ascii"))
    return len(objects)

add("<< /Type /Catalog /Pages 2 0 R /Outlines 9 0 R >>" if with_outline else "<< /Type /Catalog /Pages 2 0 R >>")
add("<< /Type /Pages /Count 2 /Kids [5 0 R 7 0 R] >>")
add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>")

def page(number, title, label, depths):
    add(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 280] "
        f"/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {len(objects)+2} 0 R >>")
    ops = ["0.98 0.97 0.93 rg 0 0 420 280 re f", "0.13 0.29 0.28 rg 0 262 420 18 re f"]
    def text(x, y, size, value, bold=False):
        ops.append(f"BT /F{2 if bold else 1} {size} Tf {x} {y} Td ({value}) Tj ET")
    text(25,232,9,"NORTH POINT   /   COASTAL SURVEY",True)
    text(25,201,25,title,True)
    text(25,178,12,label)
    for y in range(60,156,24):
        ops.append(f"0.78 0.81 0.74 RG 0.6 w 25 {y} m 393 {y} l S")
    line = " ".join(f"{25+i*46} {y} {'m' if i==0 else 'l'}" for i,y in enumerate(depths))
    ops.append(f"0.39 0.62 0.59 rg {line} 393 51 l 25 51 l h f")
    ops.append(f"0.13 0.29 0.28 RG 2 w {line} S")
    ops.append("0.13 0.29 0.28 rg")
    text(25,33,8,"06:00"); text(365,33,8,"18:00")
    text(25,12,8,"FIELD OBSERVATIONS"); text(375,12,8,f"0{number} / 02")
    content="\n".join(ops)+"\n"
    add(f"<< /Length {len(content)} >>\nstream\n{content}endstream")

page(1,"Survey overview","West channel observations, collected along the shore.",[93,102,125,143,131,102,81,88,105])
page(2,"Tidal range","The east channel reaches high water at noon.",[72,79,99,129,147,135,109,87,73])
if with_outline:
    add("<< /Type /Outlines /Count 2 /First 10 0 R /Last 11 0 R >>")
    add("<< /Title (Survey overview) /Parent 9 0 R /Next 11 0 R /Dest [5 0 R /Fit] >>")
    add("<< /Title (Tidal range) /Parent 9 0 R /Prev 10 0 R /Dest [7 0 R /Fit] >>")
pdf=bytearray(b"%PDF-1.4\n")
offsets=[0]
for i,obj in enumerate(objects,1):
    offsets.append(len(pdf))
    pdf.extend(f"{i} 0 obj\n".encode()+obj+b"\nendobj\n")
start=len(pdf)
pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
for offset in offsets[1:]:
    pdf.extend(f"{offset:010d} 00000 n \n".encode())
pdf.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n".encode())
(Path(sys.argv[1]) if with_outline else Path(__file__).with_name("Coastal survey.pdf")).write_bytes(pdf)
