# Characters

Characters builds a searchable index from the distribution’s ICU common library and Unicode data. Multiword queries match all the words, in any order. A single character, U+ notation or a hexadecimal code with at least four digits including a decimal digit selects a scalar directly. U+ also accepts all-letter hexadecimal values. ICU provides extended names for scalars without a formal name. [ICU character properties](https://unicode-org.github.io/icu/userguide/strings/properties.html)

Ship the matching ICU common library and data (Unicode licence). Building needs its headers and the `icu-uc` pkg-config file. The C adapter uses the symbol names for the installed ICU version. The application indexes and searches names off the UI thread.

The unit test resolves U+1D11E and MUSICAL SYMBOL G CLEF and rejects surrogate and out-of-range values. The GPUI test copies the supplementary-plane character. The release binary copies both 𝄞 and ★ to a private Wayland clipboard. Six pixel references show BLACK STAR selected on Sway and labwc at standard, fractional and high-contrast scales. The private AT-SPI probe checks the selected option, code point, Copy action and clipboard status.

Run `cargo test -p mundane-characters`, then `cargo build -p mundane-characters` and `tests/wayland/run --app characters`. The harness accepts `--compositor labwc`, `--profile fractional|contrast` and, on Sway, `--at-spi`.
