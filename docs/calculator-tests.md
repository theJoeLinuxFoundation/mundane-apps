# Testing Calculator

The evaluator uses integer ratios throughout. Its tests cover precedence, unary signs, parentheses, exact decimals, left-associative division and reusing a fraction as an operand. They check division by zero, missing operands, malformed decimals and trailing input. Expressions accept up to 1,024 UTF-8 bytes and 32 nested pairs of parentheses. When an exact decimal would exceed the input limit, the evaluator tries its reduced fraction so the result can still be reused.

The GPUI headless test enters `0.1 + 0.2`, reuses the result in a multiplication, and checks that an error leaves the two successful history entries available. It selects the older entry, activates Clear history by keyboard, and checks that focus returns to the expression.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --release --locked
export MUNDANE_CALCULATOR_BIN="$PWD/target/release/mundane-calculator"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app calculator --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app calculator --at-spi
```

Six pixel-exact references show a 372-pixel-wide window with two calculations. The first adds 18.75 and 4.50. The second reuses 23.25 and multiplies it by four. The test then recalls the earlier answer and checks its value on the private clipboard. Sway and labwc use isolated headless outputs with Pixman, and GPUI uses Mesa lavapipe. The profiles cover 100%, 150%, and 200% with high contrast. [Wayland test dependencies](terminal-tests.md#software-rendered-wayland-tests)

The private-bus AT-SPI test reads the focused Expression field and Result status, activates an earlier history entry, and presses Add, 2 and Equals through their accessible Click actions. It checks the resulting 25.25 and clears the history while preserving the result. The three open shared accessibility checks are tracked in the [suite report](accessibility/report.md). The packaged joe Linux VM smoke test is pending on the image.
