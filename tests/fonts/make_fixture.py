"""Rebuild the suite's two original test faces with fontTools."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection

collection = TTCollection()

for weight, style in [(400, "Regular"), (700, "Bold")]:
    font = FontBuilder(1000, isTTF=True)
    glyphs = {}
    for name in (".notdef", "space", "A", "omega"):
        pen = TTGlyphPen(None)
        if name != "space":
            width = 80 if weight == 400 else 180
            for x, y in [(50, 0), (50, 700), (500, 700), (500, 700-width),
                         (50+width, 700-width), (50+width, 0)]:
                if (x, y) == (50, 0):
                    pen.moveTo((x, y))
                else:
                    pen.lineTo((x, y))
            pen.closePath()
        glyphs[name] = pen.glyph()
    font.setupGlyphOrder(list(glyphs))
    font.setupCharacterMap({32: "space", 65: "A", 0x3A9: "omega"})
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics({g: (600, 50) for g in glyphs})
    font.setupHorizontalHeader(ascent=800, descent=-200)
    font.setupNameTable(dict(familyName="Survey Fixture", styleName=style,
                             fullName=f"Survey Fixture {style}", psName=f"SurveyFixture-{style}"))
    font.setupOS2(usWeightClass=weight, sTypoAscender=800, sTypoDescender=-200,
                  usWinAscent=800, usWinDescent=200, fsSelection=32 if weight == 700 else 64)
    font.setupPost()
    font.setupMaxp()
    font.font["head"].macStyle = int(weight == 700)
    font.font["head"].created = font.font["head"].modified = 3800000000
    font.font.recalcTimestamp = False
    collection.fonts.append(font.font)
collection.save(Path(__file__).with_name("Survey.ttc"))
