# Run a Rune app in the browser

Rune's experimental web target renders one canvas through WebGPU. It uses the same Rust
views, layout, glyph atlas and shaders as native. GPU initialization is asynchronous; the
runtime stays on the browser's main thread and requests frames only while work is pending.

## Build the example

Install the wasm target, Python 3.11 or newer, and the wasm-bindgen CLI version in Cargo.lock:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
scripts/web-build.sh web-hello --out target/web
```

Serve the entire output folder over HTTPS, including `snippets/`. A local HTTP server on
`localhost` also works for development. The host must serve `.wasm` as `application/wasm`.
No application server, SharedArrayBuffer, COOP or COEP headers are required.
`--opt` additionally runs an installed `wasm-opt -Oz`; the default build does not need it.

For production, install Binaryen (`wasm-opt`) and the Brotli CLI, then export:

```sh
scripts/web-build.sh web-hello --out target/web --opt \
  --title "Messages" --description "A Rune message list with an editable composer."
```

The title and description are HTML-escaped. The runtime then sets the document title
from `AppSpec::title`. Supply a description that describes your application.
The generated page preloads its wasm without waiting for the JavaScript import chain.
It declares an empty favicon; replace that link when you supply an application icon.

Every export regenerates deterministic `.gz` files for HTML, JavaScript, wasm and other
text assets, including `snippets/`. If the Brotli CLI is available, it also generates
`.br` files at quality 9, or quality 11 with `--opt`. The extra compression runs at
build time, not per request. Without Brotli, gzip remains available. The script prints
the actual encoded wasm sizes. Old compressed variants are removed before regeneration.

Configure the host to negotiate these files from `Accept-Encoding`, set the matching
`Content-Encoding`, and return `Vary: Accept-Encoding`. Preserve the original asset's
MIME type, especially `application/wasm`. Keep the uncompressed files for clients that
do not accept a supported encoding. A plain development file server does not perform
this negotiation. Deploy the complete export together so assets and variants match.

Lighthouse scores depend on the page, browser and throttling settings. Its paint metrics
do not reliably represent WebGPU canvas presentation. Check the rendered application
and interaction latency separately.

The example is a scrollable message list with a composer, Send and Copy last. Its title
opens an external link. Refreshing resets the in-memory messages.

## Give your app a wasm entrypoint

Add a `cdylib` target and a wasm-only wasm-bindgen dependency:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

Export a start function that calls the usual Rune entrypoint:

```rust,ignore
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn start() {
    rune::run(AppSpec::new("My app", (390.0, 760.0), State::default, view));
}
```

From your app's workspace, run `path/to/rune/scripts/web-build.sh your-package`. The script
selects one cdylib, or one binary if there is no cdylib. `--example NAME` selects a Cargo
example. Disable Rune's `dev`, `hot` and `native-file-dialog` features on wasm. Move native
filesystem, process and thread dependencies behind target conditions in your app too.
Use `rune::time::Instant` for deadlines passed to Rune; it is `std::time::Instant` on native.

## Fonts and browser input

Web includes Inter Regular and IBM Plex Mono Regular. There is no system-font discovery.
Set `AppSpec::fonts` to static font bytes and `AppSpec::families` to your sans/mono family
names for branding, additional weights, emoji and scripts outside the default fonts.
For example, extend a static slice of `rune_theme_opencode::FONTS` with
`include_bytes!("assets/NotoColorEmoji.ttf")` and the CJK fonts your app needs, then assign
the combined slice to `spec.fonts`. Keep the theme's sans/mono family names in `spec.families`.
Missing emoji and CJK glyphs display fallback boxes; they do not trigger font downloads.

Rust supplies the focused native editor with up to 512 UTF-16 units on each side of its
selection, including the selected range. This is not a maximum document or selection size.
Unchanged contexts are skipped. Browser input events supply replacements; Rune validates the mirrored
context and converts window-relative UTF-16 positions to UTF-8 boundaries before editing.

Composition holds the mirror stable until commit and replaces the composing word's range.
Mobile `beforeinput` line breaks use Rune's Enter rules: submit, or Shift/Alt+Enter newline
in a multiline field. A completed touch tap focuses the editor synchronously;
a pan scrolls instead, with momentum and a damped edge return.

Secure fields use an `<input type="password">` with autocomplete, autocorrect and spellcheck
disabled. It receives only new keystrokes and is emptied after every input; the model's
secret is never mirrored into the DOM. Native semantic controls retain keyboard focus;
the canvas is excluded from the accessibility tree. Continuous pointer movement and wheel events wait for rAF.

`Tx::copy` and `Tx::request_paste` use the browser clipboard. Call them from a user input
handler: permission and transient-activation rules still apply. Ordinary text paste uses
native browser insertion, then validates the captured selection and context before one
Rust update. Rich HTML, image paste and custom hooks retain Rune's clipboard handling.
During IME composition, Rune can cancel native paste to protect the active preedit.
Secret-copy requests report failure because the browser cannot promise native
clipboard-history exclusion or conditional timed clearing.

`rune::web::open_url` accepts HTTP(S) and mail links and opens a tab with `noopener,noreferrer`.
Call it from a user input handler. `Cx::safe_area()` returns top/right/bottom/left insets in
points (zero on native). Apply them inside a full-canvas background. The canvas follows
`visualViewport` when a software keyboard reduces the visible height. Keep the generated
page's `viewport-fit=cover` meta tag and the textarea's 16 px font size.

## Supply accessible content

The web shell exposes mounted Rune content through persistent, transparent browser elements.
Rune still owns painting, focus policy and accepted state. Browser elements supply text
services and accessibility. Applications do not manage a second DOM tree.

Rune Kit supplies control roles, checked state, declared disabled state and expansion state.
`Field::label` names its direct control or a sole editor inside layout wrappers.
Explicit control names take precedence over composed captions, then fallback names.
`TextField::new` supplies the editor's name. A container with multiple editors keeps its
group name; each editor needs its own name. Textbox shells retain padding-click behavior
without exposing another editable textbox.

A select keeps its field name when its selected value changes; the browser exposes the
selected text separately as its value.
An editor without a nonempty name or placeholder uses the generic name "Text input".
Supply a meaningful label when its purpose is not clear from the surrounding content.

For icon actions, set `IconButton::label` or meaningful `help`. A glyph cannot identify an
action's purpose. For custom controls, use the existing `El::role`, `label`, `help`,
`level` and `expanded` methods. `checked(value)` takes a boolean. Use
`indeterminate(true)` for a mixed checkbox state; `indeterminate(false)` restores its
declared checked value. A control with neither method has no checked state.
`disabled(true)` disables the subtree without hiding its content and releases its
editor focus. Missing handlers alone do not declare a disabled control.

Inline `El::link` spans expose their existing names and actions in source order, without
repeating the surrounding prose. Keyboard traversal uses that same order even when links
were declared in a different order. Empty, invalid and fully elided ranges are not focus
targets.

Browser focus uses Rune's current modal scope and focus groups. Tab can leave a
nonmodal app. A modal contains Tab and restores its opener, including its focused link, on
close. Read-only editors participate in Tab navigation and retain native selection and
copy without granting edit access.

An unrelated commit preserves the native editor and composition. When a focused virtual
row unmounts, the shell retains its existing editor until Rune releases editable focus.
Browsers with `moveBefore` preserve that element's state during reparenting. The
`insertBefore` fallback restores focus and selection but cannot restore an IME composition
that the browser cancels.

The projection covers mounted content, plus the retained focused virtual editor. It does
not expose all unmounted rows of a virtual document. Range-widget values and every
composite-widget relationship are not supplied. An ARIA role alone does not implement a
widget's interaction. Supply a meaningful name and the matching Rune behavior.

## Scope and verification

This target needs WebGPU. It has no WebGL fallback, overlays,
extra windows, native menus, drag-out, native audio/video, networking layer or hot patching.
Native configuration types remain source-compatible, but their desktop host paths are not
used on wasm. Unsupported window requests warn in the console; Show/Open are ignored.
The document title follows `AppSpec::title`, and hidden tabs report `is_visible() == false`.

The functional proof uses headless Chromium and Playwright:

```sh
node scripts/web-smoke.mjs target/web target/web-proof
```

Install Playwright with its Chromium browser first. `PLAYWRIGHT_MODULE` and `CHROMIUM_PATH`
can select an existing installation. `WEB_ENGINE=webkit` tries an installed Playwright WebKit
and reports whether it has a WebGPU adapter.

The proof checks GPU errors, idle rAF requests and submissions.
Input cases cover touch focus and momentum, Unicode edits, sliding context and predictive replacement.
It also checks composition, selection, paste, copied model text, link isolation, theme and viewport resize.

The `?input-proof` fixture adds password input, Tab/Escape focus, mobile Enter, recomposition,
same-field focus stability, continuous-input coalescing and visibility regression checks.

The mobile DPR check exercises the Safari-style CSS-size fallback because Chromium device
emulation does not scale `devicePixelContentBoxSize`. It does not test Safari's WebGPU
implementation or a real iPhone keyboard, dictation or autocorrect.

The shell follows PLAN's winit-first approach, retaining winit 0.30's rAF and ResizeObserver
implementation. Unlike research/11's proposed custom shell, it adds a document-aware
textarea beside winit rather than replacing winit. This also defers PLAN's proposed switch
to an owned shell when text input lands. WebGL fallback and EditContext remain later work.
