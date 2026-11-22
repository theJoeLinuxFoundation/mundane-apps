#include <cairo-pdf.h>
#include <pango/pangocairo.h>
#include <errno.h>
#include <unistd.h>
#include <string.h>
#include <limits.h>

static cairo_status_t write_pdf(void *context, const unsigned char *data, unsigned int len) {
    while (len) {
        ssize_t n = write(*(int *)context, data, len);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) return CAIRO_STATUS_WRITE_ERROR;
        data += n;
        len -= (unsigned int)n;
    }
    return CAIRO_STATUS_SUCCESS;
}

// Points on A4 paper. The portal print dialog can scale to the selected paper.
int mundane_text_pdf(int fd, const char *text, size_t len) {
    const double width = 595, height = 842, margin = 40;
    cairo_surface_t *pdf = cairo_pdf_surface_create_for_stream(write_pdf, &fd, width, height);
    cairo_t *cr = cairo_create(pdf);
    PangoLayout *layout = pango_cairo_create_layout(cr);
    pango_cairo_context_set_resolution(pango_layout_get_context(layout), 72);
    PangoFontDescription *font = pango_font_description_from_string("monospace 10");
    pango_layout_set_font_description(layout, font);
    pango_font_description_free(font);
    pango_layout_set_width(layout, (int)((width - margin * 2) * PANGO_SCALE));
    pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
    double y = margin;
    const char *end = text + len;
    int result = 0;
    while (text <= end) {
        const char *newline = memchr(text, '\n', (size_t)(end - text));
        size_t bytes = newline ? (size_t)(newline - text) : (size_t)(end - text);
        if (bytes && text[bytes - 1] == '\r') --bytes;
        if (bytes > INT_MAX) { result = -1; break; }
        pango_layout_set_text(layout, text, (int)bytes);
        PangoLayoutIter *iter = pango_layout_get_iter(layout);
        do {
            PangoRectangle box;
            pango_layout_iter_get_line_extents(iter, NULL, &box);
            double line_height = (double)box.height / PANGO_SCALE;
            if (y + line_height > height - margin) {
                cairo_show_page(cr);
                y = margin;
            }
            double baseline = (double)(pango_layout_iter_get_baseline(iter) - box.y) / PANGO_SCALE;
            cairo_move_to(cr, margin + (double)box.x / PANGO_SCALE, y + baseline);
            pango_cairo_show_layout_line(cr, pango_layout_iter_get_line_readonly(iter));
            y += line_height;
        } while (pango_layout_iter_next_line(iter));
        pango_layout_iter_free(iter);
        if (!newline) break;
        text = newline + 1;
    }
    cairo_show_page(cr);
    g_object_unref(layout);
    cairo_surface_finish(pdf);
    if (cairo_status(cr) || cairo_surface_status(pdf)) result = -1;
    cairo_destroy(cr);
    cairo_surface_destroy(pdf);
    return result;
}
