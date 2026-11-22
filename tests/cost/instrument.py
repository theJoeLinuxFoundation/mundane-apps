"""Instrument a disposable copy of the pinned toolkit for untimed producer diagnostics."""
import shutil
import sys
from pathlib import Path

source, work = map(Path, sys.argv[1:])
target = work / "instrumented-zed"
shutil.copytree(source, target, ignore=shutil.ignore_patterns(".git", "target"))
p = target / "crates/gpui/Cargo.toml"
p.write_text(p.read_text().replace("accesskit.workspace = true", 'accesskit = { workspace = true, features = ["serde"] }'))
p = target / "crates/gpui/src/window.rs"
s = p.read_text()
# Monotonic time from activation callback to first completed full snapshot.
s = s.replace('log::info!("Accessibility activated");', 'unsafe { mundane_cost_activation() };\n                        log::info!("Accessibility activated");')
needle = "                self.platform_window.a11y_tree_update(tree_update);"
assert s.count(needle) == 1
s = s.replace(needle, '''
                let payload = serde_json::to_vec(&tree_update).unwrap();
                static SIZES: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());
                let mut sizes = SIZES.lock().unwrap();
                if !sizes.contains(&tree_update.nodes.len()) {
                    sizes.push(tree_update.nodes.len());
                    let path = std::env::var("COST_TRACE").unwrap();
                    std::fs::write(format!("{path}.nodes-{}.json",tree_update.nodes.len()), &payload).unwrap();
                }
                drop(sizes);
                unsafe { mundane_cost_update(tree_update.nodes.len(), payload.len()) };
''' + needle)
p.write_text(s + '\nunsafe extern "C" { fn mundane_cost_activation(); fn mundane_cost_update(nodes: usize, json_bytes: usize); }\n')
p = work / "trace"
shutil.copytree(work / "pin", p)
import re
manifest = p / "Cargo.toml"
s = manifest.read_text()
for name in ("gpui", "gpui_linux"):
    s = re.sub(rf'^{name} = \{{ git = "[^"]+", rev = "[^"]+",', f'{name} = {{ path = "{target}/crates/{name}",', s, flags=re.M)
manifest.write_text(s)

# Record the CPU clock of the released Unix adapter's actual event-loop thread.
import subprocess, json
metadata = json.loads(subprocess.check_output(["cargo","metadata","--format-version","1","--locked","--manifest-path",str(work/"pin/Cargo.toml")]))
adapter = next(Path(pkg["manifest_path"]).parent for pkg in metadata["packages"] if pkg["name"]=="accesskit_unix")
local = work/"instrumented-adapter"
shutil.copytree(adapter, local)
with manifest.open("a") as stream:
    print(f'\n[patch.crates-io]\naccesskit_unix = {{ path = "{local}" }}', file=stream)
context = local/"src/context.rs"
s = context.read_text()
needle = "thread::spawn(|| {"
assert s.count(needle)==1
context.write_text(s.replace(needle, needle+"\n                unsafe { mundane_cost_adapter_thread() };") +
                   '\nunsafe extern "C" { fn mundane_cost_adapter_thread(); }\n')
