// The inventory and preview both identify a face by its file and Fontconfig
// index, including the named instance bits for variable OpenType fonts.
#include <fontconfig/fontconfig.h>
#include <pango/pangocairo.h>
#include <pango/pangofc-fontmap.h>
#include <pango/pangofc-font.h>
#include <string.h>

typedef void (*visit_face)(void *, const char *, const char *, const char *, int);

int mundane_font_list(const char *fixture, void *data, visit_face visit) {
    FcConfig *config = fixture ? FcConfigCreate() : FcInitLoadConfigAndFonts();
    if (!config) return 0;
    if (fixture && (!FcConfigParseAndLoadFromMemory(config, (const FcChar8 *)
        "<fontconfig><cachedir prefix='xdg'>fontconfig</cachedir></fontconfig>", FcTrue) ||
        !(g_file_test(fixture, G_FILE_TEST_IS_DIR) ? FcConfigAppFontAddDir(config, (const FcChar8 *)fixture) : FcConfigAppFontAddFile(config, (const FcChar8 *)fixture)))) {
        FcConfigDestroy(config);
        return 0;
    }
    FcPattern *all = FcPatternCreate();
    FcObjectSet *fields = FcObjectSetBuild(FC_FILE, FC_INDEX, FC_FAMILY, FC_STYLE, FC_VARIABLE, NULL);
    FcFontSet *fonts = FcFontList(config, all, fields);
    int ok = fonts != NULL;
    for (int i = 0; fonts && i < fonts->nfont; ++i) {
        FcPattern *p = fonts->fonts[i];
        FcChar8 *family, *style, *file;
        int index;
        FcBool variable = FcFalse;
        FcPatternGetBool(p, FC_VARIABLE, 0, &variable);
        // Fontconfig also lists a range pattern for variable fonts. Their
        // default and individual named faces have separate inventory entries.
        if (variable || FcPatternGetString(p, FC_FAMILY, 0, &family) != FcResultMatch ||
            FcPatternGetString(p, FC_FILE, 0, &file) != FcResultMatch ||
            FcPatternGetInteger(p, FC_INDEX, 0, &index) != FcResultMatch) continue;
        if (FcPatternGetString(p, FC_STYLE, 0, &style) != FcResultMatch)
            style = (FcChar8 *)"Regular";
        visit(data, (char *)family, (char *)style, (char *)file, index);
    }
    if (fonts) FcFontSetDestroy(fonts);
    FcObjectSetDestroy(fields);
    FcPatternDestroy(all);
    FcConfigDestroy(config);
    return ok;
}

typedef struct {
    PangoFontMap *map;
    PangoContext *context;
    PangoFontDescription *description;
    FcCharSet *coverage;
} Preview;

void mundane_font_close(Preview *preview) {
    if (!preview) return;
    if (preview->coverage) FcCharSetDestroy(preview->coverage);
    if (preview->description) pango_font_description_free(preview->description);
    if (preview->context) g_object_unref(preview->context);
    if (preview->map) g_object_unref(preview->map);
    g_free(preview);
}

Preview *mundane_font_open(const char *file, int index) {
    FcConfig *config = FcConfigCreate();
    Preview *preview = g_new0(Preview, 1);
    // Give just this face a private family name, including within TTC files.
    char *rule = g_strdup_printf("<fontconfig><match target='scan'>"
        "<test name='index' compare='eq'><int>%d</int></test>"
        "<edit name='family' mode='assign_replace'><string>Mundane Preview Face</string></edit>"
        "</match></fontconfig>", index);
    gboolean ok = config && FcConfigParseAndLoadFromMemory(config, (FcChar8 *)rule, FcTrue)
        && FcConfigAppFontAddFile(config, (const FcChar8 *)file);
    g_free(rule);
    if (!ok) goto failed;
    FcFontSet *faces = FcConfigGetFonts(config, FcSetApplication);
    for (int i = 0; faces && i < faces->nfont; ++i) {
        FcPattern *p = faces->fonts[i];
        int candidate;
        FcCharSet *coverage;
        FcBool variable = FcFalse;
        FcPatternGetBool(p, FC_VARIABLE, 0, &variable);
        if (variable || FcPatternGetInteger(p, FC_INDEX, 0, &candidate) != FcResultMatch ||
            candidate != index || FcPatternGetCharSet(p, FC_CHARSET, 0, &coverage) != FcResultMatch) continue;
        preview->description = pango_fc_font_description_from_pattern(p, FALSE);
        preview->coverage = FcCharSetCopy(coverage);
        break;
    }
    if (!preview->description || !preview->coverage) goto failed;
    preview->map = pango_cairo_font_map_new_for_font_type(CAIRO_FONT_TYPE_FT);
    if (!preview->map) goto failed;
    pango_fc_font_map_set_config(PANGO_FC_FONT_MAP(preview->map), config);
    preview->context = pango_font_map_create_context(preview->map);
    pango_font_description_set_family(preview->description, "Mundane Preview Face");
    PangoFont *font = pango_context_load_font(preview->context, preview->description);
    FcChar8 *resolved_file;
    int resolved_index;
    if (!font) goto failed;
    FcPattern *resolved = pango_fc_font_get_pattern(PANGO_FC_FONT(font));
    ok = FcPatternGetInteger(resolved, FC_INDEX, 0, &resolved_index) == FcResultMatch &&
         resolved_index == index && FcPatternGetString(resolved, FC_FILE, 0, &resolved_file) == FcResultMatch &&
         strcmp((char *)resolved_file, file) == 0;
    g_object_unref(font);
    if (!ok) goto failed;
    FcConfigDestroy(config);
    return preview;
failed:
    if (config) FcConfigDestroy(config);
    mundane_font_close(preview);
    return NULL;
}

int mundane_font_missing(Preview *preview, unsigned int code) {
    GUnicodeType category = g_unichar_type(code);
    // Selectors and shaping controls modify adjacent glyphs.
    if (category == G_UNICODE_CONTROL || category == G_UNICODE_FORMAT ||
        (code >= 0xfe00 && code <= 0xfe0f) || (code >= 0xe0100 && code <= 0xe01ef)) return 0;
    return !FcCharSetHasChar(preview->coverage, code);
}

static void color(cairo_t *cr, unsigned rgb) {
    cairo_set_source_rgb(cr, (rgb >> 16 & 255) / 255., (rgb >> 8 & 255) / 255., (rgb & 255) / 255.);
}

// Cairo ARGB32 yields the BGRA bytes GPUI expects on the x86 Linux target.
int mundane_font_paint(Preview *preview, const char *text, int length, int size,
                       int width, int height, unsigned fg, unsigned bg, unsigned char *pixels) {
    cairo_surface_t *surface = cairo_image_surface_create_for_data(pixels, CAIRO_FORMAT_ARGB32,
                                                                   width, height, width * 4);
    cairo_t *cr = cairo_create(surface);
    color(cr, bg);
    cairo_paint(cr);
    pango_font_description_set_absolute_size(preview->description, size * PANGO_SCALE);
    PangoLayout *layout = pango_layout_new(preview->context);
    pango_layout_set_font_description(layout, preview->description);
    PangoAttrList *attrs = pango_attr_list_new();
    pango_attr_list_insert(attrs, pango_attr_fallback_new(FALSE));
    pango_layout_set_attributes(layout, attrs);
    pango_attr_list_unref(attrs);
    pango_layout_set_width(layout, width * PANGO_SCALE);
    pango_layout_set_height(layout, height * PANGO_SCALE);
    pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
    pango_layout_set_ellipsize(layout, PANGO_ELLIPSIZE_END);
    pango_layout_set_text(layout, text, length);
    color(cr, fg);
    pango_cairo_show_layout(cr, layout);
    cairo_surface_flush(surface);
    int result = cairo_status(cr) == CAIRO_STATUS_SUCCESS ? pango_layout_is_ellipsized(layout) : -1;
    g_object_unref(layout);
    cairo_destroy(cr);
    cairo_surface_destroy(surface);
    return result;
}
