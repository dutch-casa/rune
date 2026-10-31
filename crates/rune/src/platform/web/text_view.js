// Optional text view of the existing semantic DOM. One shell control per mount, outside the reconciled root.
import { setViewportInset } from './platform.js';
const flowProps = ['position', 'inset', 'insetInlineStart', 'insetBlockStart', 'insetInlineEnd', 'insetBlockEnd',
    'inlineSize', 'blockSize', 'width', 'height', 'margin', 'padding', 'border', 'background', 'backgroundColor',
    'color', 'pointerEvents', 'font', 'direction', 'writingMode', 'resize', 'whiteSpace', 'clipPath'];

let sheet = null;
let users = 0;

const css = `
/* Clip physical hits, not native actions: Safari removes AXPress for pointer-events:none. */
[data-rune-a11y][data-rune-text-view="off"] {
  clip-path: inset(50%) !important;
  overflow: clip !important;
  pointer-events: auto !important;
  color: transparent !important;
  background-color: transparent !important;
}
[data-rune-a11y][data-rune-text-view="off"] :is([data-rune-node], [data-rune-link]) {
  color: transparent !important;
  background-color: transparent !important;
  pointer-events: auto !important;
}
:where([data-rune-text-view="on"]) {
  box-sizing: border-box;
  font: 1rem/1.5 system-ui;
  color: CanvasText;
  background-color: Canvas;
  color-scheme: light dark;
  overflow-wrap: break-word;
  padding-inline: max(1rem, env(safe-area-inset-left, 0px)) max(1rem, env(safe-area-inset-right, 0px));
  padding-block: 0.75rem max(1rem, env(safe-area-inset-bottom, 0px));
  overscroll-behavior: contain;
}
:where([data-rune-text-view="on"] *) { box-sizing: border-box; }
:where([data-rune-text-view="on"] [role="listbox"]) {
  max-block-size: min(12lh, 50dvb);
  overflow-y: auto;
}
:where([data-rune-text-view="on"] [data-rune-node]) {
  max-inline-size: 100%;
  min-inline-size: 0;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}
:where([data-rune-text-view="on"] :is(input, textarea)) {
  font: inherit;
  inline-size: 100%;
  max-inline-size: 100%;
  min-block-size: 2.75rem;
}
:where([data-rune-text-view="on"] textarea) {
  field-sizing: content;
  min-block-size: 3lh;
  max-block-size: 12lh;
  resize: none;
}
:where([data-rune-text-view="on"] :is([aria-checked="true"], [aria-selected="true"])) {
  border-inline-start: 0.2rem solid CanvasText;
}
:where([data-rune-text-view="on"] :focus-visible) {
  outline: max(2px, 0.08em) solid CanvasText;
  outline-offset: 0.25em;
}
:where([data-rune-text-view="on"] :is(button, a, [role="button"])) {
  touch-action: manipulation;
}
:where([data-rune-text-view="on"] :is(button, [role="button"]):active) {
  background-color: color-mix(in oklch, CanvasText 16%, Canvas);
}
[data-rune-text-toggle] {
  position: fixed;
  display: block;
  z-index: 2;
  inset-block-start: 0;
  inset-inline-start: 0;
  box-sizing: border-box;
  margin: 0;
  min-block-size: 1.5rem;
  min-inline-size: 2.75rem;
  padding-block: 0.2rem;
  padding-inline: 0.55rem;
  padding-block-start: max(0.2rem, env(safe-area-inset-top, 0px));
  padding-inline-start: max(0.55rem, env(safe-area-inset-left, 0px));
  touch-action: manipulation;
}
[data-rune-text-toggle][data-rune-embedded] {
  position: relative;
  padding-block-start: 0.2rem;
  padding-inline-start: 0.55rem;
}
:where([data-rune-text-toggle]) {
  font: inherit;
  color: CanvasText;
  background-color: Canvas;
  color-scheme: light dark;
  border: 1px solid CanvasText;
  border-radius: 0.25rem;
  text-align: start;
}
[data-rune-text-toggle]:focus-visible {
  outline: max(2px, 0.08em) solid CanvasText;
  outline-offset: -2px;
}
[data-rune-text-toggle]:active { background-color: color-mix(in oklch, CanvasText 16%, Canvas); }
@media (hover: hover) and (pointer: fine) {
  :where([data-rune-text-toggle]:hover) {
    background-color: color-mix(in oklch, CanvasText 12%, Canvas);
  }
}
@media (pointer: coarse) {
  [data-rune-text-toggle] { min-block-size: 2.75rem; }
}
`;

function installSheet() {
    if (!sheet) {
        sheet = document.createElement('style');
        sheet.textContent = css;
        (document.head || document.documentElement).append(sheet);
    }
    users++;
}

function uninstallSheet() {
    if (!sheet || --users > 0) return;
    sheet.remove();
    sheet = null;
}

function clearInline(element) {
    const style = element.style;
    for (const name of flowProps) if (style[name]) style[name] = '';
}

function releaseEntry(entry) {
    clearInline(entry.element);
    for (const link of entry.content?.querySelectorAll('[data-rune-link]') ?? []) clearInline(link);
}

function eachEntry(h, fn) {
    for (const entry of h.a11yNodes.values()) fn(entry);
    if (h.parked) fn(h.parked);
}

export function showing(h) {
    return !!h.textView?.on;
}

export function releaseAll(h) {
    if (!showing(h)) return;
    eachEntry(h, releaseEntry);
}

function selectionInside(h) {
    const sel = document.getSelection?.();
    if (!sel || sel.isCollapsed || !sel.anchorNode || !h.a11y.contains(sel.anchorNode)) return false;
    const node = sel.anchorNode.nodeType === 1 ? sel.anchorNode : sel.anchorNode.parentElement;
    return !node?.closest?.('input, textarea');
}

export function selection(h) {
    if (!showing(h) || !selectionInside(h)) return null;
    const current = document.getSelection();
    return { anchor: current.anchorNode, start: current.anchorOffset, focus: current.focusNode, end: current.focusOffset };
}

export function restoreSelection(h, saved) {
    if (!saved || !h.a11y.contains(saved.anchor) || !h.a11y.contains(saved.focus)) return;
    const a = saved.anchor, f = saved.focus;
    if (saved.start > (a.nodeType === 3 ? a.length : a.childNodes.length)
        || saved.end > (f.nodeType === 3 ? f.length : f.childNodes.length)) return;
    const current = document.getSelection();
    if (current.anchorNode !== a || current.anchorOffset !== saved.start || current.focusNode !== f || current.focusOffset !== saved.end) {
        current.setBaseAndExtent(a, saved.start, f, saved.end);
    }
}

export function keepFocus(h, active, modelChanged) {
    if (active && active === h.textView?.control) return true;
    if (!showing(h)) return false;
    if (selectionInside(h)) {
        if (modelChanged) h.textView.deferred = true;
        return true;
    }
    if (h.textView.deferred) {
        h.textView.deferred = false;
        return false;
    }
    return !modelChanged && !!active && h.a11y.contains(active);
}

export function blockActivate(h) {
    const active = document.activeElement;
    return active === h.textView?.control || (showing(h) && selectionInside(h));
}

export function skipCopy(h, active) {
    if (!showing(h)) return false;
    return active === h.textView.control || h.a11y.contains(active) || selectionInside(h);
}

function placeControl(h) {
    if (!h.textView.fixed) return;
    const view = window.visualViewport;
    if (!view) return;
    h.textView.control.style.insetBlockStart = `${view.offsetTop}px`;
    h.textView.control.style.insetInlineStart = `${view.offsetLeft}px`;
}

function reserve(h) {
    if (!showing(h) || h.disposed) return;
    const root = h.a11y;
    const canvas = h.canvas.getBoundingClientRect();
    root.style.clipPath = 'none';
    root.style.pointerEvents = 'auto';
    root.style.overflowX = 'clip';
    root.style.overflowY = 'auto';
    root.style.zIndex = '1';
    root.style.direction = '';
    root.style.writingMode = '';
    root.style.position = h.textView.fixed ? 'fixed' : 'absolute';
    root.style.insetInlineStart = `${h.textView.fixed ? canvas.left : h.canvas.offsetLeft}px`;
    root.style.insetBlockStart = `${h.textView.fixed ? canvas.top : h.canvas.offsetTop}px`;
    root.style.insetInlineEnd = 'auto';
    root.style.insetBlockEnd = 'auto';
    root.style.inlineSize = `${canvas.width}px`;
    root.style.blockSize = `${canvas.height}px`;
}

function apply(h) {
    const on = h.textView.on;
    const inputMode = on || !matchMedia('(pointer: coarse)').matches ? 'text' : 'none';
    for (const entry of h.editors.values()) entry.element.inputMode = inputMode;
    h.a11y.setAttribute('data-rune-text-view', on ? 'on' : 'off');
    h.textView.control.setAttribute('aria-pressed', on ? 'true' : 'false');
    h.canvas.style.visibility = on ? 'hidden' : h.textView.canvasVisibility;
    if (!on) {
        h.textView.deferred = false;
        h.textView.pageMarker = null;
        h.a11y.style.cssText = h.textView.rootCss;
        eachEntry(h, entry => h.restoreOverlay(entry));
        h.refreshEntries();
        h.placeBounds();
        return;
    }
    h.refreshEntries();
    releaseAll(h);
    reserve(h);
}

function toggle(h) {
    if (h.disposed) return;
    h.textView.on = !h.textView.on;
    apply(h);
}

function trackViewport(h) {
    if (h.disposed || !h.textView) return;
    if (h.textView.fixed) setViewportInset(h.canvas, h.textView.control.offsetHeight);
    placeControl(h);
    if (showing(h)) reserve(h);
}

// The text view shows the mounted window. Reaching either edge asks the runtime to mount
// the next page of a virtual list. A sync's own scroll does not ask again.
export function holdScroll(h) {
    const view = h.textView;
    if (!view) return;
    view.suppressScroll = true;
    requestAnimationFrame(() => requestAnimationFrame(() => {
        if (h.textView === view) view.suppressScroll = false;
    }));
}

function requestPage(h, edge) {
    if (!showing(h) || h.disposed || h.textView.suppressScroll || !edge) return;
    const nodes = h.a11y.querySelectorAll('[data-rune-node]');
    const marker = `${edge}:${nodes[0]?.dataset.runeNode ?? ''}:${nodes[nodes.length - 1]?.dataset.runeNode ?? ''}`;
    if (h.textView.pageMarker === marker) return;
    h.textView.pageMarker = marker;
    h.send({ kind: 'text_view_page', edge });
}

function atEdge(h) {
    const root = h.a11y;
    if (root.scrollHeight <= root.clientHeight + 2) return null;
    if (root.scrollTop + root.clientHeight >= root.scrollHeight - 2) return 'end';
    if (root.scrollTop <= 2) return 'start';
    return null;
}

export function attach(h) {
    installSheet();
    const control = document.createElement('button');
    control.type = 'button';
    control.textContent = 'Text view';
    control.dataset.runeTextToggle = '';
    control.setAttribute('aria-pressed', 'false');
    if (h.embedded) control.dataset.runeEmbedded = '';
    const canvasVisibility = h.canvas.style.visibility;
    h.a11y.setAttribute('data-rune-text-view', 'off');
    h.canvas.before(control);
    h.textView = { on: false, control, rootCss: h.a11y.style.cssText, canvasVisibility, fixed: !h.embedded, deferred: false, pageMarker: null };
    h.textView.resize = new ResizeObserver(() => trackViewport(h));
    h.textView.resize.observe(control);
    h.textView.resize.observe(h.canvas);
    h.listen(control, 'click', () => toggle(h));
    h.listen(h.a11y, 'scroll', () => { if (showing(h)) requestPage(h, atEdge(h)); });
    h.listen(h.a11y, 'wheel', event => {
        if (!showing(h) || h.textView.suppressScroll) return;
        const root = h.a11y;
        const atEnd = root.scrollTop + root.clientHeight >= root.scrollHeight - 2;
        const atStart = root.scrollTop <= 2;
        if (event.deltaY > 0 && atEnd) requestPage(h, 'end');
        else if (event.deltaY < 0 && atStart) requestPage(h, 'start');
    }, { passive: true });
    h.listen(h.a11y, 'focusin', event => {
        if (!showing(h) || h.textView.suppressScroll) return;
        const node = event.target?.closest?.('[data-rune-node], [data-rune-link]');
        if (!node || !h.a11y.contains(node)) return;
        const nodes = h.a11y.querySelectorAll('[data-rune-node]');
        const first = nodes[0];
        const last = nodes[nodes.length - 1];
        const box = node.getBoundingClientRect();
        const root = h.a11y.getBoundingClientRect();
        const atLast = last && (node === last || last.contains(node));
        const atFirst = first && (node === first || first.contains(node));
        if (atLast || box.bottom >= root.bottom - 4) requestPage(h, 'end');
        else if (atFirst || box.top <= root.top + 4) requestPage(h, 'start');
    });
    h.listen(window, 'resize', () => trackViewport(h));
    if (window.visualViewport) {
        h.listen(window.visualViewport, 'resize', () => trackViewport(h));
        h.listen(window.visualViewport, 'scroll', () => trackViewport(h));
    }
    trackViewport(h);
}

export function detach(h) {
    const view = h.textView;
    if (!view) return;
    view.resize.disconnect();
    h.canvas.style.visibility = view.canvasVisibility;
    if (view.fixed) setViewportInset(h.canvas, 0);
    h.textView = null;
    view.control.remove();
    h.a11y?.removeAttribute('data-rune-text-view');
    uninstallSheet();
}

export function linked() {}
