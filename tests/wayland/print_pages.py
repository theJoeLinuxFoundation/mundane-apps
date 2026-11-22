"""Read the private print job with the suite's system PDFium."""
import ctypes as c
from pathlib import Path
import os


def pages(path: Path):
    lib = c.CDLL("libpdfium.so")
    def bind(name, result, *args):
        func = getattr(lib, name)
        func.restype, func.argtypes = result, args
        return func
    handle = c.c_void_p
    load = bind("FPDF_LoadDocument", handle, c.c_char_p, c.c_char_p)
    count = bind("FPDF_GetPageCount", c.c_int, handle)
    page = bind("FPDF_LoadPage", handle, handle, c.c_int)
    text_page = bind("FPDFText_LoadPage", handle, handle)
    chars = bind("FPDFText_CountChars", c.c_int, handle)
    code = bind("FPDFText_GetUnicode", c.c_uint, handle, c.c_int)
    close_text = bind("FPDFText_ClosePage", None, handle)
    close_page = bind("FPDF_ClosePage", None, handle)
    close = bind("FPDF_CloseDocument", None, handle)
    lib.FPDF_InitLibrary()
    doc = load(os.fsencode(path), None)
    assert doc, "PDFium could not open the print job"
    result = []
    try:
        for i in range(count(doc)):
            p = page(doc, i)
            assert p
            text = text_page(p)
            assert text
            result.append("".join(chr(code(text, j)) for j in range(chars(text))))
            close_text(text)
            close_page(p)
    finally:
        close(doc)
        lib.FPDF_DestroyLibrary()
    return result
