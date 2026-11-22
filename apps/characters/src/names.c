// ICU's public symbols carry its major version. Resolve them with the installed headers.
#include <unicode/uchar.h>

typedef void (*add_name)(void *, int32_t, const char *, int32_t);
struct collector { void *data; add_name add; };

static UBool U_CALLCONV collect(void *opaque, UChar32 code, UCharNameChoice choice,
                               const char *name, int32_t length) {
    (void)choice;
    struct collector *out = opaque;
    out->add(out->data, code, name, length);
    return 1;
}

int mundane_names(void *data, add_name add) {
    struct collector out = {data, add};
    UErrorCode error = U_ZERO_ERROR;
    u_enumCharNames(0, 0x110000, collect, &out, U_UNICODE_CHAR_NAME, &error);
    return U_SUCCESS(error);
}

int mundane_extended_name(int32_t code, char *name, int32_t capacity) {
    UErrorCode error = U_ZERO_ERROR;
    int32_t length = u_charName(code, U_EXTENDED_CHAR_NAME, name, capacity, &error);
    return U_SUCCESS(error) ? length : 0;
}
