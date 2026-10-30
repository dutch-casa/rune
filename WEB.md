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

Full-canvas pages allow native pinch zoom while Rune owns one-finger scrolling and dragging.
Fit-content canvases also allow native vertical page scrolling. Ctrl+wheel zoom is not cancelled.

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

The browser shell projects the retained scene into a semantic DOM mirror beside the canvas.
It exposes headings, text, links, controls and native editors. Geometry follows presented
layout. Unchanged frames emit no mirror update. The inspector reads the same retained facts.
The mirror is clipped in canvas view. Its native controls keep pointer events enabled so
Safari exposes activation; the clip leaves physical pointer hits with the canvas.

Rune Kit supplies names, checked and mixed state, expansion, disabled and invalid state,
and progress values from its existing component values. A `Field` renders one caption node;
its control references that node with `aria-labelledby`. An explicit different control name
takes precedence. Groups with several editors need a name for each editor. Icon-only controls
need a label or help text that names their action.

`El::fallback_label` replaces an existing name; it does not mean “set only if missing.”
Use `label_if_missing` to supply a composed caption without replacing an explicit control name.

For custom controls, use `El::role`, `label`, `checked(bool)`, `indeterminate(bool)`,
`disabled(bool)` and `invalid(bool)` as appropriate. `value_range(value, min..=max)` requires
finite, ordered bounds and an in-range value; it panics rather than clamping invalid input.
`controls(key)` resolves one current target. A missing or ambiguous target leaves the
relationship unset. `orientation(Axis)` describes the control's orientation.

Roles do not implement keyboard behavior. Kit supplies that behavior for its controls.
Its tree and table cursors are separate from selection. `focus_cursor(bool)` exposes that
cursor; a focused search editor references the cursor inside its declared controlled list.
Read-only editors support selection and copying without changing the model or enabling IME.
Return can activate the window's default button. Disabled controls remain readable but cannot
receive input. Browser actions are checked against the current scene.

Inline links follow source order while retaining their action identities. Ordinary web URLs
remain native anchors. Modal focus returns to the previous control or link on dismissal.
Focused virtual editors retain their native element across unmounts. Browsers without
`Element.moveBefore` use focus and selection restoration; reparenting can still end native
composition in those browsers.

The shell's **Text view** button displays the same semantic DOM in normal text flow and hides
the canvas. It keeps the native editor, draft, selection and application handlers. Browser
zoom, user fonts, text spacing and foreground/background overrides can then apply to real
text. The button remains outside the reconciled scene and returns to canvas rendering.
This is an alternate presentation, not another UI tree or application state model.

`Runtime::a11y_json()` retains its JSON array contract with optional control and editor fields.
Virtual lists expose mounted rows only. Custom drawings need meaningful text alternatives.
Authors still own content, theme contrast, custom keyboard interactions and the `open` state
of controlled overlays such as `Tooltip` and `HoverCard`. Use `help` for a control's accessible
description. This experimental target does not make an arbitrary application WCAG-conformant;
check complete pages and processes with their target browsers, screen readers and devices.

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
and reports whether it has a WebGPU adapter.

The check reports GPU errors and measures idle rAF requests and GPU submissions. It exercises
touch focus, momentum, Unicode edits, sliding editor contexts, composition, paste, clipboard copy,
themes and resizing.

The `?input-proof` fixture adds password input, Tab/Escape focus and mobile Enter behavior.
Its Kit controls check native names and values, caption ownership, checkable labels and help
descriptions through Chromium's accessibility tree. `?accessibility-proof` exercises disclosure,
invalid input recovery, progress and notification dismissal. Its `widget=combobox`, `table`
and `tree` variants exercise keyboard cursors and native relationships. `widget=sidebar`
checks independent header focus, and `widget=split` checks pointer-only resizing and grip keys.

Its mobile DPR check exercises the Safari-style CSS-size fallback because Chromium device
emulation does not scale `devicePixelContentBoxSize`. This is not a real iPhone keyboard,
dictation, autocorrect or Safari WebGPU test; those still need device verification.

For a manual Safari and VoiceOver check, open `?accessibility-proof`. Activate
**After controls (0)** with VoiceOver; its name must change to **After controls (1)**.
Expand **Details** and check the reported state. Change **Account name** from `bad` to `ready`
and check that VoiceOver no longer reports invalid data. On `?accessibility-proof&widget=combobox`,
open **Model**, use Down and Return to choose **Beta**, and check both the name and value.

The shell retains winit 0.30's rAF and ResizeObserver integration. Rune owns the browser text
services and semantic DOM bridge beside the canvas. This does not replace the native shells.
WebGL fallback and EditContext remain later work.
