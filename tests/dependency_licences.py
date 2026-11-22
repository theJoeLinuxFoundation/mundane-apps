"""Check Cargo's resolved, target-specific graph for every suite binary."""
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
# Positive list so missing or newly encountered terms require reading the licence.
# MPL is file-level copyleft. option-ext is already used in the desktop stack.
REVIEWED = set("""0BSD Apache-2.0 BSD-2-Clause BSD-3-Clause BSL-1.0 CC0-1.0 ISC
MIT MPL-2.0 Unicode-3.0 Unlicense Zlib""".split())
# Cargo 'links' names in the target graph. FreeType uses its FTL option.
LINKS = {"curl", "z", "freetype", "fontconfig", "rayon-core", "wasm_bindgen"}


def allowed(expression):
    # Old Cargo manifests use slash for a choice of licences. Evaluate every
    # operand, including a disallowed alternative, to consume the full grammar.
    tokens = iter(re.findall(r"[()]|[^\s()]+", (expression or "").replace("/", " OR ")))
    token = next(tokens, None)

    def shift():
        nonlocal token
        old, token = token, next(tokens, None)
        return old

    def atom():
        if token == "(":
            shift()
            value = choice()
            if shift() != ")":
                raise ValueError("missing closing parenthesis")
            return value
        identifier = shift()
        if identifier in (None, "OR", "AND", "WITH", ")"):
            raise ValueError("expected a licence identifier")
        if token == "WITH":
            shift()
            return (identifier, shift()) == ("Apache-2.0", "LLVM-exception")
        return identifier in REVIEWED

    def conjunction():
        value = atom()
        while token == "AND":
            shift()
            value = atom() and value
        return value

    def choice():
        value = conjunction()
        while token == "OR":
            shift()
            value = conjunction() or value
        return value

    try:
        return choice() and token is None
    except ValueError:
        return False


def main():
    assert allowed("(MIT OR GPL-3.0-only) AND Apache-2.0")
    assert allowed("Apache-2.0 WITH LLVM-exception OR MIT/Apache-2.0")
    for expr in (None, "GPL-2.0-only", "MIT AND AGPL-3.0-or-later", "LGPL-2.1-only",
                 "GPL-3.0-only WITH Classpath-exception-2.0", "(MIT", "MIT OR"):
        assert not allowed(expr), expr
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--all-features", "--filter-platform",
         "x86_64-unknown-linux-gnu", "--format-version", "1"], cwd=ROOT))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    todo = [i for i in metadata["workspace_members"]
            if any("bin" in t["kind"] for t in packages[i]["targets"])]
    seen = set()
    failures = []
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        p = packages[i]
        if not allowed(p["license"]) or (p["links"] and p["links"] not in LINKS):
            failures.append(f"{p['name']} {p['version']}  licence={p['license']}  links={p['links']}")
        for dep in nodes[i]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]) and not any(
                    "proc-macro" in t["kind"] for t in packages[dep["pkg"]]["targets"]):
                todo.append(dep["pkg"])
    if failures:
        raise SystemExit("Review linked dependency licences:\n" + "\n".join(sorted(failures)))
    print(f"Checked {len(seen)} packages reachable through normal x86-64 Linux binary dependencies.")


if __name__ == "__main__":
    main()
