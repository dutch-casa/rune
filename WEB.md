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
The script prints the raw and gzip wasm sizes. Configure gzip or Brotli on the static host
to get the corresponding transfer size.

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

Rust supplies each retained native editor with up to 512 UTF-16 units on each side of the
selection, expanding for the selected range. Browser input events supply replacements;
Rune validates the context and converts window-relative UTF-16 positions to UTF-8 boundaries.
Composition holds the mirror stable until commit and replaces the composing word's range.

Mobile `beforeinput` line breaks use Rune's Enter rules: submit, or Shift/Alt+Enter newline
in a multiline field. A completed touch tap focuses the editor synchronously;
a pan scrolls instead, with momentum and a damped edge return.

Secure fields use an `<input type="password">` with autocomplete, autocorrect and spellcheck
disabled. It receives only new keystrokes and is emptied after every input; the model's
secret is never mirrored into the DOM. Native focus follows Rune's controls. Tab can leave
a non-modal app; modal traversal stays inside the active modal. Continuous pointer movement
and wheel events wait for rAF.

`Tx::copy` and `Tx::request_paste` use the browser clipboard. Call them from a user input
handler: permission and transient-activation rules still apply. Native Paste events work
without an asynchronous clipboard read. Secret-copy requests report failure because the
browser cannot promise native clipboard-history exclusion or conditional timed clearing.

`rune::web::open_url` accepts HTTP(S) and mail links and opens a tab with `noopener,noreferrer`.
Call it from a user input handler. `Cx::safe_area()` returns top/right/bottom/left insets in
points (zero on native). Apply them inside a full-canvas background. The canvas follows
`visualViewport` when a software keyboard reduces the visible height. Keep the generated
page's `viewport-fit=cover` meta tag and the textarea's 16 px font size.

## Accessibility

The browser shell extends the retained scene into a hidden semantic DOM mirror. It exposes
headings, text, links, controls and native editors without replacing canvas rendering.
Geometry follows presented layout. Unchanged frames emit no mirror update.

Rune Kit supplies names and checked, expanded and disabled state from its existing component
values. A `Field` caption names its sole editor; an explicit control label takes precedence.
Groups with several editors need a name for each editor. Icon-only controls need a label
or help text that names their action.

For custom controls, use `El::role`, `label`, `checked(bool)`, `indeterminate(bool)` and
`disabled(bool)` as appropriate. Roles do not implement a custom control's keyboard behavior.
Read-only editors support native selection and copying. Disabled controls remain readable
but cannot receive input. Browser actions are checked against the current scene.

Inline links follow source order while retaining their action identities. Ordinary web URLs
remain native anchors. Modal focus returns to the previous control or link on dismissal.
Focused virtual editors retain their native element across unmounts. Browsers without
`Element.moveBefore` use focus and selection restoration; reparenting can still end native
composition in those browsers.

`Runtime::a11y_json()` returns the existing JSON array with optional control and editor fields.
This target remains experimental. These semantics do not certify WCAG conformance or replace
screen-reader and device checks for an application's custom interactions.

## Scope and verification

This target needs WebGPU. It has no WebGL fallback, overlays, extra windows, native menus,
drag-out, native audio/video, networking layer or hot patching.
Native configuration types remain source-compatible, but their desktop host paths are not
used on wasm. Unsupported window requests warn in the console; Show/Open are ignored.
The document title follows `AppSpec::title`, and hidden tabs report `is_visible() == false`.

The functional proof uses headless Chromium and Playwright:

```sh
node scripts/web-smoke.mjs target/web target/web-proof
```

Install Playwright with its Chromium browser first. `PLAYWRIGHT_MODULE` and `CHROMIUM_PATH`
can select an existing installation. `WEB_ENGINE=webkit` tries an installed Playwright WebKit
and reports whether it has a WebGPU adapter. The proof checks GPU errors, idle rAF requests
and submissions, touch
focus and momentum, Unicode edits, a long sliding context, predictive replacement,
composition, selection, paste, copied model text, link isolation, theme and viewport resize.
The `?input-proof` fixture adds password input, Tab/Escape focus, mobile Enter, recomposition,
same-field focus stability, continuous-input coalescing and visibility regression checks.
Its mobile DPR check exercises the Safari-style CSS-size fallback because Chromium device
emulation does not scale `devicePixelContentBoxSize`. This is not a real iPhone keyboard,
dictation, autocorrect or Safari WebGPU test; those still need device verification.

The shell follows PLAN's winit-first approach, retaining winit 0.30's rAF and ResizeObserver
implementation. Unlike research/11's proposed custom shell, it adds a document-aware
textarea beside winit rather than replacing winit. This also defers PLAN's proposed switch
to an owned shell when text input lands. WebGL fallback and EditContext remain later work.
