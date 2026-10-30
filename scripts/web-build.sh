#!/usr/bin/env bash
# Run from the app's Cargo workspace. Produces a static, HTTPS-hostable folder.
set -euo pipefail
package=${1:?usage: web-build.sh PACKAGE [--example NAME] [--out DIR] [--opt] [--title TITLE] [--description DESCRIPTION]}
shift
example=
out=target/web
opt=false
title=Rune
description=
while (($#)); do
    case "$1" in
        --example) example=${2:?--example needs a name}; shift 2 ;;
        --out) out=${2:?--out needs a directory}; shift 2 ;;
        --opt) opt=true; shift ;;
        --title) title=${2:?--title needs a title}; shift 2 ;;
        --description) description=${2:?--description needs a description}; shift 2 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done
meta=$(cargo metadata --format-version 1 --no-deps)
selection=$(python3 -c '
import json,sys
m=json.load(sys.stdin); p=next((p for p in m["packages"] if p["name"]==sys.argv[1]),None)
if p is None: sys.exit("package is not a workspace member")
targets=[t for t in p["targets"] if "example" in t["kind"] and t["name"]==sys.argv[2]] if sys.argv[2] else [t for t in p["targets"] if "cdylib" in t["crate_types"]]
if not targets and not sys.argv[2]: targets=[t for t in p["targets"] if "bin" in t["kind"]]
if len(targets)!=1: sys.exit("select one cdylib, binary, or --example target")
t=targets[0]
print(m["workspace_root"]); print(m["target_directory"]); print(t["name"])
print("example" if sys.argv[2] else "lib" if "cdylib" in t["crate_types"] else "bin")
' "$package" "$example" <<< "$meta")
root=$(sed -n '1p' <<< "$selection")
target=$(sed -n '2p' <<< "$selection")
name=$(sed -n '3p' <<< "$selection")
kind=$(sed -n '4p' <<< "$selection")
args=(--lib)
artifact="$target/wasm32-unknown-unknown/release/${name//-/_}.wasm"
if [[ $kind == example ]]; then args=(--example "$name"); artifact="$target/wasm32-unknown-unknown/release/examples/$name.wasm"; fi
if [[ $kind == bin ]]; then args=(--bin "$name"); artifact="$target/wasm32-unknown-unknown/release/$name.wasm"; fi
CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=debuginfo \
    cargo build -p "$package" --target wasm32-unknown-unknown --release "${args[@]}"
version=$(python3 -c 'import sys,tomllib; print(next(p["version"] for p in tomllib.load(open(sys.argv[1],"rb"))["package"] if p["name"]=="wasm-bindgen"))' "$root/Cargo.lock")
if ! command -v wasm-bindgen >/dev/null || [[ $(wasm-bindgen --version) != "wasm-bindgen $version" ]]; then
    echo "Install the matching CLI: cargo install wasm-bindgen-cli --version $version --locked" >&2
    exit 2
fi
mkdir -p "$out"
wasm-bindgen --target web --out-dir "$out" --out-name app "$artifact"
if $opt; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-reference-types \
        "$out/app_bg.wasm" -o "$out/app_bg.opt.wasm"
    mv "$out/app_bg.opt.wasm" "$out/app_bg.wasm"
fi
python3 - "$out/index.html" "$title" "$description" <<'PY'
import html
import pathlib
import sys

path, title, description = sys.argv[1:]
metadata = f"<title>{html.escape(title, quote=True)}</title>\n"
if description:
    metadata += f'<meta name="description" content="{html.escape(description, quote=True)}">\n'
page = """<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<meta name="color-scheme" content="light dark">
<link rel="icon" href="data:,">
""" + metadata + """<link rel="preload" href="./app_bg.wasm" as="fetch" type="application/wasm" crossorigin="anonymous">
<style>html,body{margin:0;inline-size:100%;block-size:100%;overflow:clip;overscroll-behavior:none;touch-action:none}#status{font:16px system-ui;padding:24px}</style>
<div id="status">Loading…</div>
<script type="module">
import init from './app.js';
try {
    if (!navigator.gpu) throw new Error('WebGPU requires a supported browser and HTTPS.');
    await init();
    document.querySelector('#status').remove();
} catch (error) {
    document.querySelector('#status').textContent = String(error);
    console.error(error);
}
</script>
</html>
"""
pathlib.Path(path).write_text(page, encoding="utf-8")
PY
python3 - "$out" "$opt" <<'PY'
import gzip
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

root = pathlib.Path(sys.argv[1])
extensions = {".html", ".js", ".mjs", ".css", ".json", ".wasm", ".svg", ".txt", ".xml", ".map"}
brotli = shutil.which("brotli")
quality = "11" if sys.argv[2] == "true" else "9"

for variant in root.rglob("*"):
    if variant.is_file() and variant.suffix in {".gz", ".br"}:
        original = variant.with_suffix("")
        if original.suffix in extensions:
            variant.unlink()

for asset in sorted(root.rglob("*")):
    if not asset.is_file() or asset.suffix not in extensions:
        continue
    for encoding in (("gz", "br") if brotli else ("gz",)):
        destination = pathlib.Path(f"{asset}.{encoding}")
        fd, temporary = tempfile.mkstemp(prefix=f".{destination.name}.", dir=asset.parent)
        try:
            with os.fdopen(fd, "wb") as output:
                if encoding == "gz":
                    with asset.open("rb") as source, gzip.GzipFile(filename="", mode="wb", fileobj=output, compresslevel=9, mtime=0) as compressed:
                        shutil.copyfileobj(source, compressed)
                else:
                    subprocess.run([brotli, f"--quality={quality}", "--stdout", str(asset)], stdout=output, check=True)
            os.chmod(temporary, asset.stat().st_mode & 0o666)
            os.replace(temporary, destination)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)

wasm = root / "app_bg.wasm"
sizes = [f"{wasm.stat().st_size:,} bytes raw", f"{wasm.with_suffix('.wasm.gz').stat().st_size:,} bytes gzip"]
if brotli:
    sizes.append(f"{wasm.with_suffix('.wasm.br').stat().st_size:,} bytes Brotli")
print(f"{wasm}: {', '.join(sizes)}")
print(f"Static folder: {root}")
print("Configure the host to negotiate precompressed .br and .gz files with Content-Encoding and Vary: Accept-Encoding.")
print("Serve .wasm as application/wasm and retain the original asset MIME type for compressed responses.")
if not brotli:
    print("Brotli CLI not found; gzip files were generated. Install Brotli separately to add .br files.")
PY
