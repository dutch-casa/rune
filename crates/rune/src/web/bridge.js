// Browser text services and semantic DOM accessibility mirror.
import { createEditor } from './text_input.js';

const roles = new Set(('alert alertdialog application article banner blockquote button caption cell checkbox code columnheader combobox complementary contentinfo definition deletion dialog directory document emphasis feed figure form grid gridcell group heading img insertion link list listbox listitem log main marquee math menu menubar menuitem menuitemcheckbox menuitemradio navigation none note option paragraph presentation progressbar radio radiogroup region row rowgroup rowheader scrollbar search searchbox separator slider spinbutton status strong subscript suggestion superscript switch tab table tablist tabpanel term textbox time timer toolbar tooltip tree treegrid treeitem').split(' '));
const unnamed = new Set(['none', 'presentation', 'paragraph', 'code', 'emphasis', 'strong', 'subscript', 'superscript', 'term', 'caption', 'deletion', 'insertion']);
const selectedRoles = new Set(['option', 'tab', 'treeitem', 'row', 'gridcell']);
const buttonRoles = new Set(['button', 'combobox']);
let nextRoot = 0;

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
        canvasAriaHidden: canvas.getAttribute('aria-hidden'), mirrorActive: false };
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
        if (!h.syncing && !a11y.contains(event.relatedTarget) && event.relatedTarget !== canvas) h.send({ kind: 'a11y_blur' });
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
        if (!entry || entry.editor || entry.data.disabled || (entry.data.tab == null && !buttonRoles.has(entry.data.k))) return;
        event.preventDefault();
        h.send({ kind: 'a11y_press', path: node.path });
    });
    h.listen(a11y, 'keydown', event => {
        if (h.passThrough(event) || event.defaultPrevented || event.isComposing || event.keyCode === 229) return;
        const node = target(h, event.target);
        if (event.key === 'Tab') {
            if (!h.modal) return;
        } else {
            if (node?.element.dataset.runeUrl === 'true' && event.key === 'Enter') return;
            if (node?.link === null && h.a11yNodes.get(node?.path)?.data.k === 'button' && event.key === 'Enter') return;
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
        for (const [key, down] of h.keys) {
            h.keys.delete(key);
            h.send({ ...down, kind: 'key_up', shift: false, ctrl: false, alt: false, meta: false, repeat: false });
        }
    });
    h.listen(document, 'copy', event => {
        const active = document.activeElement;
        if (!h.editors.has(target(h, active)?.path) && h.copyText && event.clipboardData
            && (active === canvas || active === document.body || a11y.contains(active))) {
            event.preventDefault();
            event.clipboardData.setData('text/plain', h.copyText);
        }
    });
    h.listen(canvas, 'pointerdown', event => event.preventDefault(), { capture: true });
    h.listen(canvas, 'keydown', event => { if (!browserShortcut(event)) event.preventDefault(); });
    if (!fitHeight) {
        h.listen(canvas, 'wheel', event => event.preventDefault(), { passive: false });
        h.listen(document, 'gesturestart', event => event.preventDefault(), { passive: false });
    }
    queueMicrotask(() => {
        if (!h.disposed && !embedded && document.activeElement === document.body) focusElement(h).focus({ preventScroll: true });
    });
    return h;
}

function tagFor(item) {
    if (item.edit) return item.edit.secure ? 'input' : 'textarea';
    if (buttonRoles.has(item.k)) return 'button';
    if (item.k === 'heading') return `h${item.l || 2}`;
    return item.k === 'text' ? 'p' : 'div';
}

function compatible(entry, item) {
    return !!item && entry.tag === tagFor(item) && !!entry.editor === !!item.edit
        && (!entry.editor || entry.editor.secure === item.edit.secure);
}

function create(h, item) {
    const tag = tagFor(item);
    const editor = item.edit ? createEditor(h, item.p, item.edit.secure) : null;
    const element = editor?.field ?? document.createElement(tag);
    if (tag === 'button') element.type = 'button';
    element.id = `${h.prefix}-${++h.nextId}`;
    element.dataset.runeNode = item.p;
    element.style.cssText = 'position:absolute;box-sizing:border-box;margin:0;padding:0;border:0;background:transparent;color:transparent;pointer-events:none;resize:none;direction:ltr;writing-mode:horizontal-tb;';
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
    attribute(element, 'role', editor || item.k === 'button' || item.k === 'heading' || item.k === 'text' ? null : role);
    attribute(element, 'aria-label', editor ? item.name || item.edit.placeholder || (item.edit.secure ? 'Password input' : 'Text input')
        : role && !unnamed.has(role) ? item.name || null : null);
    attribute(element, 'tabindex', item.tab ?? (editor || buttonRoles.has(item.k) ? -1 : null));
    attribute(element, 'aria-expanded', item.x);
    attribute(element, 'aria-checked', item.checked);
    attribute(element, 'aria-selected', selectedRoles.has(item.k) ? !!item.selected : null);
    attribute(element, 'aria-modal', item.modal ? true : null);
    attribute(element, 'aria-disabled', item.disabled ? true : null);
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
    element.style.font = `${editor ? 16 : item.s || 14}px/1.25 sans-serif`;
    element.style.whiteSpace = 'pre-wrap';
    if (editor) {
        element.readOnly = !!item.edit.readonly;
        attribute(element, 'aria-multiline', !item.edit.single);
        attribute(element, 'placeholder', item.edit.placeholder || null);
        if (item.edit.text != null) editor.sync(item.edit.text, item.edit.base, item.edit.anchor, item.edit.focus);
        else if (h.context?.key === item.p && h.context.secure === item.edit.secure) {
            const context = h.context;
            editor.sync(context.text, context.base, context.anchor, context.focus);
        }
    } else {
        const signature = JSON.stringify([item.t, item.u]);
        if (entry.signature !== signature) {
            entry.signature = signature;
            content.replaceChildren();
            if (item.u?.length) {
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
                        a.style.cssText = 'color:transparent;text-decoration:none;pointer-events:none;';
                        content.append(a);
                    } else content.append(document.createTextNode(run.t));
                }
            } else content.textContent = item.t ?? '';
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

export function syncA11y(h, json) {
    if (h.disposed) return;
    const packet = JSON.parse(json);
    const active = document.activeElement;
    const owned = active === h.canvas || h.a11y.contains(active);
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
            if (h.parked) h.editors.set(h.parked.editor.key, h.parked);
        }
        for (const [path, rect] of packet.bounds ?? []) h.bounds.set(path, rect);
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
        h.focused = packet.focus ?? null;
        if (owned) {
            const element = focusElement(h);
            if (document.activeElement !== element) element.focus({ preventScroll: true });
        }
    } finally { h.syncing = false; }
}

export function activate(h) {
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
    h.a11y.remove();
}
