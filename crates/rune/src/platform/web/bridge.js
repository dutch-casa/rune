// Browser text services, semantic DOM mirror, and an optional text view of that mirror.
import { createEditor } from './text_input.js';
import * as textView from './text_view.js';

const roles = new Set(('alert alertdialog application article banner blockquote button caption cell checkbox code columnheader combobox complementary contentinfo definition deletion dialog directory document emphasis feed figure form grid gridcell group heading img insertion link list listbox listitem log main marquee math menu menubar menuitem menuitemcheckbox menuitemradio navigation none note option paragraph presentation progressbar radio radiogroup region row rowgroup rowheader scrollbar search searchbox separator slider spinbutton status strong subscript suggestion superscript switch tab table tablist tabpanel term textbox time timer toolbar tooltip tree treegrid treeitem').split(' '));
const unnamed = new Set(['none', 'presentation', 'paragraph', 'code', 'emphasis', 'strong', 'subscript', 'superscript', 'term', 'caption', 'deletion', 'insertion']);
const selectedRoles = new Set(['option', 'tab', 'treeitem', 'row', 'gridcell']);
const buttonRoles = new Set(['button', 'combobox']);
let nextRoot = 0;
const nodeChrome = 'position:absolute;box-sizing:border-box;margin:0;padding:0;border:0;background:transparent;color:transparent;pointer-events:none;resize:none;direction:ltr;writing-mode:horizontal-tb;';
const linkChrome = 'color:transparent;text-decoration:none;pointer-events:none;';

function browserShortcut(event) {
    const chord = event.metaKey || event.ctrlKey;
    if (event.key === 'F5' || event.key === 'F12') return true;
    if (!chord) return false;
    const k = event.key.toLowerCase();
    // Copy stays native so the synchronous copy event can supply selected canvas text.
    return ['c', 'r', 'l', 't', 'w', 'n', 'f', 'g', '=', '+', '-', '0'].includes(k)
        || (event.altKey && ['i', 'j', 'u'].includes(k));
}

function attribute(element, name, value) {
    if (value === null || value === undefined) {
        if (element.hasAttribute(name)) element.removeAttribute(name);
    } else if (element.getAttribute(name) !== String(value)) element.setAttribute(name, String(value));
}

function webLink(url) {
    return url.startsWith('https://') || url.startsWith('http://') || url.startsWith('mailto:');
}

function target(h, element) {
    if (!element || !h.a11y.contains(element)) return null;
    const link = element.closest?.('[data-rune-link]');
    if (link && h.a11y.contains(link)) return { path: link.dataset.runePath, link: Number(link.dataset.runeLink), element: link };
    const node = element.closest?.('[data-rune-node]');
    if (!node || !h.a11y.contains(node)) return null;
    return { path: node.dataset.runeNode, link: null, element: node };
}

function sameFocus(a, b) {
    return a?.path === b?.path && (a?.link ?? null) === (b?.link ?? null);
}
function focusElement(h) {
    const focus = h.focused;
    if (focus?.link != null) {
        for (const entry of h.a11yNodes.values()) {
            for (const link of entry.content?.querySelectorAll('[data-rune-link]') ?? []) {
                if (link.dataset.runePath === focus.path && Number(link.dataset.runeLink) === focus.link) return link;
            }
        }
    } else if (focus) {
        const element = h.a11yNodes.get(focus.path)?.element;
        if (element) return element;
    }
    if (h.parked && (!focus || focus.path === h.parked.editor.key)) return h.parked.element;
    return h.mirrorActive ? h.a11y : h.canvas;
}


export function mount(canvas, callback, embedded, fitHeight) {
    const a11y = document.createElement('div');
    a11y.setAttribute('data-rune-a11y', '');
    a11y.setAttribute('role', 'document');
    a11y.tabIndex = -1;
    if (embedded && canvas.parentElement) {
        if (getComputedStyle(canvas.parentElement).position === 'static') canvas.parentElement.style.position = 'relative';
        a11y.style.cssText = 'position:absolute;inset:0;overflow:clip;clip-path:inset(50%);pointer-events:none;z-index:1;direction:ltr;writing-mode:horizontal-tb;';
        canvas.after(a11y);
    } else {
        a11y.style.cssText = 'position:fixed;inset:0;overflow:clip;clip-path:inset(50%);pointer-events:none;z-index:1;direction:ltr;writing-mode:horizontal-tb;';
        document.body.append(a11y);
    }
    const h = { canvas, a11y, a11yNodes: new Map(), editors: new Map(), parked: null, context: null,
        bounds: new Map(), focused: null, modal: false, keys: new Map(), prefix: `rune-a11y-${++nextRoot}`, nextId: 0,
        copyText: '', listeners: [], syncing: false, disposed: false, embedded, fitHeight,
        canvasAriaHidden: canvas.getAttribute('aria-hidden'), canvasTabIndex: canvas.getAttribute('tabindex'), mirrorActive: false };
    h.send = event => {
        const message = JSON.stringify(event);
        if (h.syncing) queueMicrotask(() => { if (!h.disposed) callback(message); });
        else if (!h.disposed) callback(message);
    };
    h.listen = (target, name, handler, options) => {
        target.addEventListener(name, handler, options);
        h.listeners.push(() => target.removeEventListener(name, handler, options));
    };
    h.passThrough = (event, editor = false) => browserShortcut(event) || (event.ctrlKey && event.altKey)
        || (!editor && event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight'));
    h.forwardKeyDown = event => {
        const data = { kind: 'key', key: event.key, code: event.code ?? '', shift: !!event.shiftKey,
            ctrl: !!event.ctrlKey, alt: !!event.altKey, meta: !!event.metaKey, repeat: !!event.repeat };
        h.keys.set(event.code || event.key, data);
        h.send(data);
    };
    h.listen(a11y, 'focusin', event => {
        const next = target(h, event.target);
        if (!next) return;
        const entry = h.a11yNodes.get(next.path);
        if (entry?.editor) entry.element.inputMode = 'text';
        if (!h.syncing && next.element !== h.parked?.element && !sameFocus(next, h.focused)) {
            h.send({ kind: 'a11y_focus', path: next.path, ...(next.link === null ? {} : { link: next.link }) });
        }
    });
    h.listen(a11y, 'focusout', event => {
        if (h.syncing) return;
        const next = event.relatedTarget;
        if (next && (a11y.contains(next) || next === canvas)) return;
        if (next) {
            h.send({ kind: 'a11y_blur' });
            return;
        }
        // A null relatedTarget is the window leaving, Safari reporting a move inside this
        // tree, or a press on the canvas. activeElement has settled by the next frame.
        const token = (h.focusLeave = (h.focusLeave || 0) + 1);
        requestAnimationFrame(() => {
            if (h.disposed || h.syncing || h.focusLeave !== token || h.canvasDown) return;
            const active = document.activeElement;
            if (active && (a11y.contains(active) || active === canvas || active === h.textView?.control)) return;
            if (h.windowBlurred || !document.hasFocus()) return;
            h.send({ kind: 'a11y_blur' });
        });
    });
    h.listen(a11y, 'click', event => {
        const node = target(h, event.target);
        if (!node) return;
        const entry = h.a11yNodes.get(node.path);
        if (entry?.data.disabled) { event.preventDefault(); return; }
        if (node.link !== null) {
            if (node.element.dataset.runeUrl === 'true') return;
            event.preventDefault();
            h.send({ kind: 'a11y_link', path: node.path, link: node.link });
            return;
        }
        if (!entry || entry.editor || entry.data.disabled) return;
        if (!entry.data.pressable && entry.data.tab == null && !buttonRoles.has(entry.data.k)) return;
        event.preventDefault();
        h.send({ kind: 'a11y_press', path: node.path });
    });
    h.listen(a11y, 'keydown', event => {
        if (h.passThrough(event) || event.defaultPrevented || event.isComposing || event.keyCode === 229) return;
        const node = target(h, event.target);
        if (event.key === 'Tab') {
            if (!h.modal) return;
        } else if (textView.showing(h) && !node) {
            return;
        } else {
            if (node?.element.dataset.runeUrl === 'true' && (event.key === 'Enter' || event.key === ' ')) return;
            if (node?.link === null && h.a11yNodes.get(node?.path)?.data.k === 'button' && event.key === 'Enter') return;
            if (textView.showing(h) && node?.link === null && buttonRoles.has(h.a11yNodes.get(node?.path)?.data.k) && event.key === ' ') return;
            if (h.editors.has(node?.path)) return;
        }
        event.preventDefault();
        h.forwardKeyDown(event);
    });
    h.listen(document, 'keyup', event => {
        const key = event.code || event.key;
        const down = h.keys.get(key);
        if (!down) return;
        h.keys.delete(key);
        h.send({ ...down, kind: 'key_up', shift: !!event.shiftKey, ctrl: !!event.ctrlKey,
            alt: !!event.altKey, meta: !!event.metaKey, repeat: false });
    });
    h.listen(window, 'blur', () => {
        h.windowBlurred = true;
        for (const [key, down] of h.keys) {
            h.keys.delete(key);
            h.send({ ...down, kind: 'key_up', shift: false, ctrl: false, alt: false, meta: false, repeat: false });
        }
    });
    h.listen(window, 'focus', () => {
        h.windowBlurred = false;
        if (h.disposed || h.syncing || !document.hasFocus()) return;
        const active = document.activeElement;
        if (active && (a11y.contains(active) || active === canvas || active === h.textView?.control)) return;
        if (!h.focused && !h.parked) return;
        const element = focusElement(h);
        if (element && document.activeElement !== element) element.focus({ preventScroll: true });
    });
    h.listen(document, 'copy', event => {
        const active = document.activeElement;
        if (textView.skipCopy(h, active)) return;
        if (!h.editors.has(target(h, active)?.path) && h.copyText && event.clipboardData
            && (active === canvas || active === document.body || a11y.contains(active))) {
            event.preventDefault();
            event.clipboardData.setData('text/plain', h.copyText);
        }
    });
    h.listen(canvas, 'pointerdown', event => {
        // A canvas press can blur the mirror with a null relatedTarget. It is a hit on the
        // scene, so the field keeps its focus and composition.
        h.canvasDown = true;
        requestAnimationFrame(() => requestAnimationFrame(() => { h.canvasDown = false; }));
        event.preventDefault();
    }, { capture: true });
    h.listen(canvas, 'keydown', event => { if (!browserShortcut(event)) event.preventDefault(); });
    if (!fitHeight) {
        h.listen(canvas, 'wheel', event => { if (!event.ctrlKey) event.preventDefault(); }, { passive: false });
    }
    queueMicrotask(() => {
        if (!h.disposed && !embedded && document.activeElement === document.body) focusElement(h).focus({ preventScroll: true });
    });
    h.restoreOverlay = entry => {
        entry.element.style.cssText = nodeChrome;
        entry.element.style.font = `${entry.editor ? 16 : entry.data.s || 14}px/1.25 sans-serif`;
        entry.element.style.whiteSpace = 'pre-wrap';
        for (const link of entry.content?.querySelectorAll('[data-rune-link]') ?? []) link.style.cssText = linkChrome;
    };
    h.placeBounds = () => applyBounds(h);
    h.refreshEntries = () => refreshEntries(h);
    // Pinch-zoom pans the canvas through visualViewport. Geometry in Rust is unchanged, so
    // the mirror's hit rectangles are placed again from the canvas's new box.
    const placeAfterViewport = () => { if (!h.disposed && !textView.showing(h)) h.placeBounds(); };
    if (window.visualViewport) {
        h.listen(window.visualViewport, 'scroll', placeAfterViewport);
        h.listen(window.visualViewport, 'resize', placeAfterViewport);
    }
    h.listen(window, 'resize', placeAfterViewport);
    textView.attach(h);
    return h;
}

function comboboxEditor(item) {
    return !!(item?.edit && item.k === 'combobox');
}

function comboboxInput(item) {
    return comboboxEditor(item) && !!item.edit.single;
}

function tagFor(item) {
    if (item.edit) return item.edit.secure || comboboxInput(item) ? 'input' : 'textarea';
    if (buttonRoles.has(item.k)) return 'button';
    if (item.k === 'label') return 'label';
    if (item.k === 'heading') return `h${item.l || 2}`;
    return item.k === 'text' ? 'p' : 'div';
}

function compatible(entry, item) {
    return !!item && entry.tag === tagFor(item) && !!entry.editor === !!item.edit
        && (!entry.editor || (entry.editor.secure === item.edit.secure && !!entry.editor.combobox === comboboxInput(item)));
}

function create(h, item) {
    const tag = tagFor(item);
    const editor = item.edit ? createEditor(h, item.p, item.edit.secure, comboboxInput(item)) : null;
    const element = editor?.field ?? document.createElement(tag);
    if (tag === 'button') element.type = 'button';
    element.id = `${h.prefix}-${++h.nextId}`;
    element.dataset.runeNode = item.p;
    element.style.cssText = textView.showing(h) ? '' : nodeChrome;
    const content = editor ? null : document.createElement('span');
    if (content) element.append(content);
    return { element, editor, content, data: item, tag, help: null, signature: null };
}

function place(parent, element, before) {
    if (element.parentNode === parent && element === before) return;
    if (parent.moveBefore && element.isConnected && parent.isConnected) {
        parent.moveBefore(element, before);
        return;
    }
    const active = document.activeElement;
    const focused = active && (active === element || element.contains(active));
    const start = focused ? active.selectionStart : null;
    const end = focused ? active.selectionEnd : null;
    const direction = focused ? active.selectionDirection : null;
    parent.insertBefore(element, before);
    if (focused && active.isConnected) {
        if (document.activeElement !== active) active.focus({ preventScroll: true });
        if (start != null && end != null && (active.selectionStart !== start || active.selectionEnd !== end || active.selectionDirection !== direction)) {
            active.setSelectionRange(start, end, direction);
        }
    }
}

function update(h, entry, item) {
    const { element, editor, content } = entry;
    entry.data = item;
    const role = roles.has(item.k) ? item.k : null;
    const chromeRole = editor || item.k === 'button' || item.k === 'heading' || item.k === 'text';
    attribute(element, 'role', comboboxEditor(item) ? 'combobox' : chromeRole ? null : role);
    attribute(element, 'aria-label', editor ? item.name || item.edit.placeholder || (item.edit.secure ? 'Password input' : 'Text input')
        : role && !unnamed.has(role) ? item.name || null : null);
    attribute(element, 'tabindex', item.tab ?? (editor || buttonRoles.has(item.k) ? -1 : null));
    attribute(element, 'aria-expanded', item.x);
    attribute(element, 'aria-autocomplete', comboboxEditor(item) ? 'list' : null);
    attribute(element, 'aria-orientation', item.orientation === 'horizontal' || item.orientation === 'vertical' ? item.orientation : null);
    attribute(element, 'aria-checked', item.checked);
    attribute(element, 'aria-selected', selectedRoles.has(item.k) ? !!item.selected : null);
    attribute(element, 'aria-modal', item.modal ? true : null);
    attribute(element, 'aria-disabled', item.disabled ? true : null);
    attribute(element, 'aria-invalid', item.invalid ? true : null);
    attribute(element, 'aria-level', item.k === 'treeitem' ? item.l ?? null : null);
    attribute(element, 'aria-valuenow', item.range ? item.range.value : null);
    attribute(element, 'aria-valuemin', item.range ? item.range.min : null);
    attribute(element, 'aria-valuemax', item.range ? item.range.max : null);
    if (editor || element.tagName === 'BUTTON') element.disabled = !!item.disabled;
    if (item.help) {
        if (!entry.help) {
            entry.help = document.createElement('span');
            entry.help.id = `${element.id}-help`;
            entry.help.hidden = true;
            h.a11y.append(entry.help);
        }
        if (entry.help.textContent !== item.help) entry.help.textContent = item.help;
        attribute(element, 'aria-describedby', entry.help.id);
    } else {
        entry.help?.remove(); entry.help = null;
        attribute(element, 'aria-describedby', null);
    }
    if (!textView.showing(h)) {
        element.style.font = `${editor ? 16 : item.s || 14}px/1.25 sans-serif`;
        element.style.whiteSpace = 'pre-wrap';
    }
    if (editor) {
        element.readOnly = !!item.edit.readonly;
        attribute(element, 'aria-multiline', !item.edit.single);
        attribute(element, 'placeholder', item.edit.placeholder || null);
        if (item.edit.text != null) editor.sync(item.edit.text, item.edit.base, item.edit.anchor, item.edit.focus);
        else if (h.context?.key === item.p && h.context.secure === item.edit.secure) {
            const context = h.context;
            editor.sync(context.text, context.base, context.anchor, context.focus);
        }
    }
}

function disposeEntry(entry) {
    entry.editor?.destroy();
    entry.help?.remove();
    entry.element.remove();
}

export function syncCopy(h, text) {
    h.copyText = text;
}

function writeContent(h, entry, parents) {
    const { content, data: item } = entry;
    if (!content) return;
    const mode = textView.showing(h);
    const fallback = parents && mode ? leafFallback(item, parents) : '';
    const signature = JSON.stringify([mode, item.t, item.u, fallback]);
    if (entry.signature === signature) return;
    entry.signature = signature;
    if (item.u?.length) {
        content.replaceChildren();
        for (const run of item.u) {
            if (run.h && run.p != null && run.i != null) {
                const a = document.createElement('a');
                a.textContent = run.t;
                a.dataset.runePath = run.p;
                a.dataset.runeLink = String(run.i);
                const url = webLink(run.h);
                a.dataset.runeUrl = String(url);
                a.href = url ? run.h : '#';
                if (url) { a.target = '_blank'; a.rel = 'noopener noreferrer'; }
                attribute(a, 'role', run.role || null);
                attribute(a, 'aria-label', run.label || null);
                attribute(a, 'tabindex', run.tab ?? -1);
                a.style.cssText = mode ? '' : linkChrome;
                content.append(a);
            } else content.append(document.createTextNode(run.t));
        }
    } else {
        // Safari can cache a clipped control's name after replacing its text node.
        const text = item.t || fallback;
        const child = content.firstChild;
        if (child?.nodeType === 3 && content.childNodes.length === 1) child.data = text;
        else content.textContent = text;
    }
}

function leafFallback(item, parents) {
    if (item.t || item.u?.length || parents.has(item.p)) return '';
    const parts = [];
    if (item.name) parts.push(item.name);
    const range = item.range;
    if (range && Number.isFinite(range.value) && Number.isFinite(range.min) && Number.isFinite(range.max)) {
        parts.push(`${range.value} (${range.min}–${range.max})`);
    }
    return parts.join(' ');
}

function presentEntries(h) {
    let parents = null;
    if (textView.showing(h)) {
        parents = new Set();
        for (const { data: item } of h.a11yNodes.values()) {
            if (!item.t && !item.u?.length && !item.name && !item.edit) continue;
            let parent = item.parent;
            while (parent && !parents.has(parent)) {
                parents.add(parent);
                parent = h.a11yNodes.get(parent)?.data.parent;
            }
        }
        if (h.parked?.data.parent) parents.add(h.parked.data.parent);
    }
    for (const entry of h.a11yNodes.values()) writeContent(h, entry, parents);
    if (h.parked) writeContent(h, h.parked, parents);
}

function refreshEntries(h) {
    if (h.disposed) return;
    for (const entry of h.a11yNodes.values()) {
        entry.signature = null;
        update(h, entry, entry.data);
    }
    if (h.parked) {
        h.parked.signature = null;
        update(h, h.parked, h.parked.data);
    }
    presentEntries(h);
    applyBounds(h);
}

function revealActive(h, entry, activeDesc, activeId, focusPath) {
    if (!textView.showing(h) || textView.blockActivate(h) || entry.data.p !== focusPath) return;
    if (entry.data.k !== 'grid' && entry.data.k !== 'treegrid' && entry.data.k !== 'tree' && entry.data.k !== 'combobox') return;
    if (!activeId || !activeDesc || entry.element.getAttribute('aria-activedescendant') === activeId) return;
    activeDesc.element.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

function compositionHeld(h) {
    const entry = h.editors.get(h.context?.key);
    // moveBefore keeps a focused field's composition. Without it, reparenting ends the
    // composition, so the tree waits until the composition commits.
    return !!entry?.editor.composing && typeof h.a11y.moveBefore !== 'function';
}

function applyBounds(h) {
    if (h.disposed || textView.showing(h)) return;
    const canvasRect = h.canvas.getBoundingClientRect();
    const rootRect = h.a11y.getBoundingClientRect();
    for (const [path, entry] of h.a11yNodes) {
        const rect = h.bounds.get(path) ?? entry.data.r;
        if (!rect) continue;
        const parentRect = h.bounds.get(entry.data.parent) ?? h.a11yNodes.get(entry.data.parent)?.data.r;
        const x = parentRect ? rect[0] - parentRect[0] : canvasRect.left - rootRect.left + rect[0];
        const y = parentRect ? rect[1] - parentRect[1] : canvasRect.top - rootRect.top + rect[1];
        const values = { insetInlineStart: `${x}px`, insetBlockStart: `${y}px`, inlineSize: `${rect[2]}px`, blockSize: `${rect[3]}px` };
        for (const [name, value] of Object.entries(values)) if (entry.element.style[name] !== value) entry.element.style[name] = value;
    }
}

export function syncA11y(h, json) {
    if (h.disposed) return;
    const packet = JSON.parse(json);
    textView.holdScroll(h);
    if (packet.items && compositionHeld(h)) {
        h.imeHeld = true;
        h.modal = !!packet.modal;
        h.focused = packet.focus ?? null;
        for (const [path, rect] of packet.bounds ?? []) h.bounds.set(path, rect);
        applyBounds(h);
        return;
    }
    const active = document.activeElement;
    const owned = active === h.canvas || h.a11y.contains(active);
    const selection = textView.selection(h);
    h.syncing = true;
    try {
        h.modal = !!packet.modal;
        if (h.parked && (h.parked.editor.key !== h.context?.key || h.parked.editor.secure !== h.context?.secure)) {
            disposeEntry(h.parked);
            h.editors.delete(h.parked.editor.key);
            h.parked = null;
        }
        if (packet.items) {
            if (!h.mirrorActive) {
                h.canvas.setAttribute('aria-hidden', 'true');
                h.canvas.setAttribute('tabindex', '-1');
                h.mirrorActive = true;
            }
            const next = new Map(packet.items.map(item => [item.p, item]));
            const focused = h.editors.get(h.context?.key);
            if (!h.parked && focused && focused.editor.secure === h.context?.secure) {
                let park = !compatible(focused, next.get(focused.data.p));
                for (let parent = focused.element.parentElement; !park && parent && parent !== h.a11y; parent = parent.parentElement) {
                    const ancestor = h.a11yNodes.get(parent.dataset.runeNode);
                    park = !ancestor || !compatible(ancestor, next.get(ancestor.data.p));
                }
                if (park) {
                    place(h.a11y, focused.element, null);
                    focused.element.tabIndex = -1;
                    h.a11yNodes.delete(focused.data.p);
                    h.parked = focused;
                }
            }
            h.editors.clear();
            const previous = new Map();
            for (const item of packet.items) {
                let entry = h.a11yNodes.get(item.p);
                if (entry && !compatible(entry, item)) {
                    disposeEntry(entry);
                    h.a11yNodes.delete(item.p);
                    entry = null;
                }
                if (!entry && h.parked && h.parked.editor.key === item.p && item.edit && !item.edit.readonly
                    && h.parked.editor.secure === item.edit.secure) {
                    entry = h.parked;
                    h.parked = null;
                }
                if (!entry) entry = create(h, item);
                h.a11yNodes.set(item.p, entry);
                update(h, entry, item);
                if (entry.editor) h.editors.set(item.p, entry);
                const parentEntry = h.a11yNodes.get(item.parent);
                const parent = parentEntry?.element ?? h.a11y;
                const after = previous.get(parent);
                const before = after ? after.nextSibling : parentEntry?.content ? parentEntry.content.nextSibling : parent.firstChild;
                if (entry.element.parentElement !== parent || before !== entry.element) place(parent, entry.element, before ?? null);
                previous.set(parent, entry.element);
            }
            for (const [path, entry] of h.a11yNodes) {
                if (next.has(path)) continue;
                disposeEntry(entry);
                h.a11yNodes.delete(path);
                h.bounds.delete(path);
            }
            presentEntries(h);
            // Paths may refer to later siblings. Resolve them only after retained entries exist,
            // and clear a relation whose target was removed.
            const focusPath = packet.focus?.path ?? null;
            for (const entry of h.a11yNodes.values()) {
                const label = h.a11yNodes.get(entry.data.labelled_by);
                const id = label?.data.k === 'label' ? label.element.id : null;
                attribute(entry.element, 'aria-labelledby', id);
                if (id) attribute(entry.element, 'aria-label', null);
                const activeDesc = h.a11yNodes.get(entry.data.activedescendant);
                const activeId = activeDesc && activeDesc.element !== entry.element && activeDesc.element.isConnected ? activeDesc.element.id : null;
                revealActive(h, entry, activeDesc, activeId, focusPath);
                attribute(entry.element, 'aria-activedescendant', activeId);
                const controlled = h.a11yNodes.get(entry.data.controls);
                const controlsId = controlled && controlled.element !== entry.element && controlled.element.isConnected ? controlled.element.id : null;
                attribute(entry.element, 'aria-controls', controlsId);
            }
            if (h.parked) {
                const entry = h.parked;
                attribute(entry.element, 'aria-labelledby', null);
                attribute(entry.element, 'aria-activedescendant', null);
                attribute(entry.element, 'aria-controls', null);
                attribute(entry.element, 'aria-label', entry.data.name || entry.data.edit.placeholder
                    || (entry.editor.secure ? 'Password input' : 'Text input'));
                h.editors.set(entry.editor.key, entry);
            }
        }
        for (const [path, rect] of packet.bounds ?? []) h.bounds.set(path, rect);
        applyBounds(h);
        const modelChanged = !sameFocus(h.focused, packet.focus ?? null);
        h.focused = packet.focus ?? null;
        if (!textView.keepFocus(h, active, modelChanged) && owned) {
            const element = focusElement(h);
            if (document.activeElement !== element) {
                element.focus({ preventScroll: true });
                if (textView.showing(h) && modelChanged) element.scrollIntoView({ block: 'nearest', inline: 'nearest' });
            }
        }
    } finally {
        textView.restoreSelection(h, selection);
        h.syncing = false;
    }
}

export function activate(h) {
    if (textView.blockActivate(h)) return;
    const entry = h.editors.get(h.context?.key);
    const field = entry?.element ?? focusElement(h);
    if (entry) field.inputMode = 'text';
    if (document.activeElement !== field) field.focus({ preventScroll: true });
}

export async function pastedImage(h, max) {
    const file = h.image;
    h.image = undefined;
    return file && file.size <= max ? new Uint8Array(await file.arrayBuffer()) : undefined;
}

export function destroy(h) {
    h.disposed = true;
    textView.detach(h);
    h.image = undefined;
    for (const remove of h.listeners) remove();
    for (const entry of h.a11yNodes.values()) disposeEntry(entry);
    if (h.parked) disposeEntry(h.parked);
    h.parked = null;
    h.context = null;
    h.copyText = '';
    h.keys.clear();
    h.editors.clear();
    h.a11yNodes.clear();
    h.bounds.clear();
    if (h.mirrorActive && h.canvas.getAttribute('aria-hidden') === 'true') {
        attribute(h.canvas, 'aria-hidden', h.canvasAriaHidden);
    }
    if (h.mirrorActive && h.canvas.getAttribute('tabindex') === '-1') {
        attribute(h.canvas, 'tabindex', h.canvasTabIndex);
    }
    h.a11y.remove();
}
