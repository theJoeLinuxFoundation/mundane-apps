# Fonts

Fonts enumerates installed families through Fontconfig. A face is identified by its file and index, which also distinguishes faces in a collection. For the preview, a private Fontconfig configuration assigns the selected index its own family name. The app checks Pango’s resolved file and index and disables fallback for the sample. This lets the user see which characters that face can actually draw. Pango handles bidirectional text, shaping and wrapping. The status lists missing code points and indicates when a long sample has been shortened to fit. [Fontconfig developer reference](https://fontconfig.pages.freedesktop.org/fontconfig/fontconfig-devel/)

Ship the system Fontconfig (HPND and Unicode data notices), Pango with its Fontconfig and Cairo backends (dynamic LGPL), Cairo (dynamic LGPL option), HarfBuzz (MIT) and FreeType (FTL option). Retain these packages’ notices. The build uses the system headers and pkg-config files for `fontconfig`, `pangofc` and `pangocairo`. GPUI draws the interface. A dedicated worker draws the preview at the window’s current text and display scale.

## Verification

The repository contains an original two-face TrueType collection. The model test enumerates its Regular and Bold faces, compares their actual preview pixels and finds the missing U+661F in a mixed sample. The GPUI test uses the same collection, switches to Bold and replaces the sample through the keyboard. The checked-in collection suffices for both tests. [Fixture source](../tests/fonts/make_fixture.py)

The six software-rendered Sway and labwc references show DejaVu Sans Bold with Latin, Greek and Hebrew text. On the private accessibility bus the probe checks the selected family, face, editable sample and preview description. It replaces the sample with 字, verifies the missing U+5B57 in DejaVu and selects Noto Sans CJK SC to verify coverage.

Run `cargo test -p mundane-fonts`, `cargo build -p mundane-fonts` and `tests/wayland/run --app fonts`. The harness accepts `--compositor labwc`, `--profile fractional|contrast` and, on Sway, `--at-spi`.
