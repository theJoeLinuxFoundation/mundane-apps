# mundane-apps

Desktop applications for mundaneDE on the x86 edition of joe Linux. The suite will use Rust and GPUI's Linux Wayland platform, with GPU rendering through wgpu.

## Status

The project is in planning. Terminal is the first application, followed by Files and Text Editor. The [development plan](docs/plan.md) covers the full suite, its shared interface and the acceptance tests for each application.

## Building

This planning revision contains documentation only. Once the Cargo workspace lands, the intended build command on x86 joe Linux is `cargo build --workspace --release --locked`. Running an application will require a mundaneDE Wayland session and a working Mesa graphics stack.

## Licence

The project's code and documentation are covered by the [BSD 3-Clause licence](LICENSE). GPUI is an Apache-2.0 dependency.
