# Packages and dave

Packages uses joe’s installed `/usr/bin/dave` and `/usr/bin/pkexec`. Ship dave’s `io.joelinux.dave.change` polkit action, an administrator authentication agent and `dave-refresh.timer`. The timer is scheduled 15 minutes after boot and every 12 hours, with up to 30 minutes of randomized delay. Search and review show the index state that dave returned. All package and deployment queries run as the desktop user. [dave JSON contract](https://github.com/theJoeLinuxFoundation/joe-linux/blob/8d0ece7/dave/share/dave.8) · [Polkit policy](https://github.com/theJoeLinuxFoundation/joe-linux/blob/8d0ece7/dave/share/io.joelinux.dave.policy)

## Reviewing a change

The app calls `dave install|remove|upgrade --dry-run --json [NAMES]`. The review lists installations, version transitions and removals, including dependencies, download size and disk change when known. Cancel starts focused. A plan with a null digest has no Apply button.

Explicit confirmation consumes the review and invokes `pkexec /usr/bin/dave install|remove|upgrade -y --json --expect DIGEST [NAMES]` once. Names are validated against dave’s package-name syntax and passed as individual arguments. pkexec exit 126 reports dismissal and 127 reports denied or unavailable authorization. Success requires a matching operation and digest in the receipt. Packages then reloads the installed list or deployment status and displays the rollback point if dave returned one.

A structured refusal with `reason: "changed"` always opens a fresh unprivileged review with Cancel focused, even if the repositories have since returned to the original digest. Explicit `null`, `not_allowed`, `needs_root` and unrecognized reasons display dave’s message immediately. Older builds omit the field. For those replies Packages fetches the preview and asks for confirmation again only if the digest differs. A failed preview displays both errors. [dave error reasons at 676fd56](https://github.com/theJoeLinuxFoundation/joe-linux/commit/676fd5610c3700d2ae65a66f68bfc55b96e7737e)

Updates distinguishes the running deployment, the next boot and an upgrade prepared for shutdown. It displays partial state explicitly while waiting for dave’s boot service. The visible status page polls every 15 seconds and uses the revision to detect changes.

## Verification

The release workspace built offline. After the structured-reason update, all 40 workspace tests, strict Clippy and the Cargo licence check over 483 packages passed. Both the standard and high-contrast AT-SPI runs passed.

The decoder tests include eight documents captured from joe-linux’s dave-json build at `8d0ece7` using a private copy of its signed CLI test root. They cover list, info, search, install, remove, an empty upgrade, deployment status and a refusal. Additional fixtures exercise a future field, malformed JSON, unknown schemas, and an invalid digest. [Captured documents](../tests/packages/dave-v1.json)

The headless keyboard test searches for a package, inspects its dependencies, cancels, and verifies that authorization has never run. A recording substitute then refuses a stale plan and offers a different version. The recorded transaction file stays empty until the second explicit confirmation. The reason tests count preview and authorization invocations for old and new documents, including a changed refusal whose refreshed digest matches the original. Other cases cover denial, dismissal, removal, preparation for shutdown and empty plans. The real GUI and test launcher use the same service and argument builder. Only the test launcher substitutes the executable paths. [Recording fixture](../tests/packages/fixture.py)

Six pixel references show the dependency review in headless Sway and labwc with Pixman and Mesa lavapipe. The profiles cover 100%, 150%, and 200% with high contrast and larger type. Each run cancels one review, applies one fixture transaction and opens Updates. The private AT-SPI inspector checks the modal role, dependency and version labels, focused Cancel, and the Apply button. The [joe image report](vm/182cd7c/README.md) covers the packaged launcher and the installed dave version. Interactive polkit authentication remains pending.

```sh
cargo test --workspace --locked --offline
cargo build --release --locked --offline -p mundane-packages --bin mundane-packages --example packages-fixture
export MUNDANE_PACKAGES_BIN="$PWD/target/release/mundane-packages"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app packages --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app packages --at-spi
```

Discover reads AppStream desktop components from the XDG data directories (`metainfo`, `appdata`, `swcatalog/xml` and `app-info/xmls`) and the system catalog locations under `/var/cache` and `/var/lib`. It also reads visible launchers from GIO. joe can distribute a collection as XML or XML.gz with `pkgname`, desktop launchable, categories, description and screenshots. A screenshot can be local or HTTPS, and is read only on request. Launch uses GIO’s installed desktop ID. The local Wayland fixture exercises category navigation, a recorded launch and a screenshot without contacting a provider.
