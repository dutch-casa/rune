// The browser owns accessible-name calculation and native control behavior.
import { createEditor } from './text_input.js';

const roles = new Set(('alert alertdialog application article banner blockquote button caption cell checkbox code columnheader combobox complementary contentinfo definition deletion dialog directory document emphasis feed figure form grid gridcell group heading img insertion link list listbox listitem log main marquee math menu menubar menuitem menuitemcheckbox menuitemradio navigation none note option paragraph presentation progressbar radio radiogroup region row rowgroup rowheader scrollbar search searchbox separator slider spinbutton status strong subscript suggestion superscript switch tab table tablist tabpanel term textbox time timer toolbar tooltip tree treegrid treeitem').split(' '));
const selected = new Set(['option', 'tab', 'treeitem', 'row', 'gridcell']);
const unnamed = new Set(['none', 'presentation', 'paragraph', 'code', 'emphasis', 'strong', 'subscript', 'superscript', 'term', 'caption', 'deletion', 'insertion']);
let nextRoot = 0;

function attribute(element, name, value) {
    if (value === null || value === undefined) {
        if (element.hasAttribute(name)) element.removeAttribute(name);
    } else if (element.getAttribute(name) !== String(value)) element.setAttribute(name, String(value));
}

export function mountAccessibility(h) {
    const root = document.createElement('div');
    root.dataset.runeAccessibility = '';
    root.tabIndex = -1;
    root.style.cssText = 'position:fixed;inset:0;opacity:0;pointer-events:none;overflow:clip;direction:ltr;writing-mode:horizontal-tb';
    document.body.append(root);
    h.root = root;
    h.nodes = new Map();
    h.editors = new Map();
    h.parked = null;
    h.context = null;
    h.keys = new Map();
    h.prefix = `rune-semantics-${++nextRoot}`;
    h.nextId = 0;
    h.modal = false;
    h.focused = null;
    h.forwardKeyDown = event => {
        const key = event.code || event.key;
        const data = { kind: 'key', key: event.key, code: event.code, shift: event.shiftKey, ctrl: event.ctrlKey,
            alt: event.altKey, meta: event.metaKey, repeat: event.repeat };
        h.keys.set(key, data);
        h.send(data);
    };
    const target = event => {
        if (event.target === h.parked?.element) return h.parked;
        const element = event.target.closest?.('[data-rune-node]');
        return element && root.contains(element) ? h.nodes.get(element.dataset.runeNode) : undefined;
    };
    h.listen(root, 'focusin', event => {
        const entry = target(event);
        if (!entry) return;
        if (entry.editor) entry.element.inputMode = 'text';
        if (!h.syncing && entry !== h.parked && entry.data.id !== h.focused) h.send({ kind: 'accessibility', action: 'focus', id: entry.data.id });
    });
    h.listen(root, 'focusout', event => {
        if (!h.syncing && !root.contains(event.relatedTarget) && event.relatedTarget !== h.canvas) {
            h.send({ kind: 'accessibility', action: 'blur' });
        }
    });
    h.listen(root, 'click', event => {
        const entry = target(event);
        if (!entry?.data.press || entry.data.disabled || entry.editor) return;
        event.preventDefault();
        h.send({ kind: 'accessibility', action: 'activate', id: entry.data.id });
    });
    h.listen(root, 'keydown', event => {
        if (h.passThrough(event)) return;
        if (event.defaultPrevented || event.isComposing || event.keyCode === 229) return;
        const entry = target(event);
        if (event.key === 'Tab') {
            if (!h.modal) return;
        } else if (entry?.editor) return;
        event.preventDefault();
        h.forwardKeyDown(event);
    });
    h.listen(document, 'keyup', event => {
        const key = event.code || event.key;
        const down = h.keys.get(key);
        if (!down) return;
        h.keys.delete(key);
        h.send({ ...down, up: true, shift: event.shiftKey, ctrl: event.ctrlKey, alt: event.altKey, meta: event.metaKey, repeat: false });
    });
    h.listen(window, 'blur', () => {
        for (const [key, down] of h.keys) {
            h.keys.delete(key);
            h.send({ ...down, up: true, shift: false, ctrl: false, alt: false, meta: false, repeat: false });
        }
    });
    return root;
}

function create(h, data) {
    const editor = data.edit ? createEditor(h, data.key, data.edit.secure) : null;
    const tag = data.role === 'button' ? 'button' : 'div';
    const element = editor?.field ?? document.createElement(tag);
    if (tag === 'button' && !editor) element.type = 'button';
    element.id = `${h.prefix}-${++h.nextId}`;
    element.dataset.runeNode = data.id;
    element.style.cssText += ';position:fixed;box-sizing:border-box;margin:0;padding:0;border:0;pointer-events:none;resize:none;overflow:clip;';
    const text = document.createTextNode('');
    if (!editor) element.append(text);
    return { element, editor, text, data, help: null, after: null };
}

function compatible(entry, data) {
    if (!data || !!entry.editor !== !!data.edit) return false;
    return entry.editor ? entry.editor.key === data.key && entry.editor.secure === data.edit.secure
        : (entry.data.role === 'button') === (data.role === 'button');
}

function place(parent, element, before) {
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
        if (start !== null && start !== undefined && end !== null && end !== undefined
            && (active.selectionStart !== start || active.selectionEnd !== end || active.selectionDirection !== direction)) {
            active.setSelectionRange(start, end, direction);
        }
    }
}

function update(h, entry, data) {
    const { element, editor, text } = entry;
    entry.data = data;
    const role = roles.has(data.role) ? data.role : null;
    attribute(element, 'role', editor || data.role === 'button' ? null : role);
    const label = data.label || (editor ? data.edit.placeholder || 'Text input' : null);
    attribute(element, 'aria-label', role && !unnamed.has(role) ? label : null);
    const content = data.text ?? (data.role === 'text' ? data.label : '') ?? '';
    if (!editor && text.data !== content) text.data = content;
    if (data.after !== null) {
        entry.after ??= document.createTextNode('');
        if (entry.after.data !== data.after) entry.after.data = data.after;
    } else {
        entry.after?.remove(); entry.after = null;
    }
    if (data.focusable || data.press || editor || data.role === 'button') {
        attribute(element, 'tabindex', data.tab);
    } else attribute(element, 'tabindex', null);
    attribute(element, 'aria-expanded', data.expanded);
    attribute(element, 'aria-checked', data.checked);
    attribute(element, 'aria-selected', selected.has(data.role) ? data.selected : null);
    attribute(element, 'aria-level', data.role === 'heading' ? data.level ?? 2 : data.level);
    attribute(element, 'aria-modal', data.modal ? true : null);
    attribute(element, 'aria-disabled', data.disabled ? true : null);
    if (editor || data.role === 'button') element.disabled = data.disabled;
    if (data.help) {
        if (!entry.help) {
            entry.help = document.createElement('span');
            entry.help.id = `${element.id}-help`;
            entry.help.hidden = true;
            h.root.append(entry.help);
        }
        if (entry.help.textContent !== data.help) entry.help.textContent = data.help;
        attribute(element, 'aria-describedby', entry.help.id);
    } else {
        entry.help?.remove(); entry.help = null;
        attribute(element, 'aria-describedby', null);
    }
    if (editor) {
        const edit = data.edit;
        element.readOnly = edit.readonly;
        attribute(element, 'aria-multiline', !edit.single);
        attribute(element, 'placeholder', edit.placeholder || null);
        if (edit.text !== null) editor.sync(edit.text, edit.base, edit.anchor, edit.focus);
        else if (h.context?.key === data.key && h.context.secure === edit.secure) {
            const context = h.context;
            editor.sync(context.text, context.base, context.anchor, context.focus);
        }
    }
}

export function syncAccessibility(h, json) {
    if (h.disposed) return;
    const packet = JSON.parse(json);
    const active = document.activeElement;
    const owned = active === h.canvas || h.root.contains(active);
    h.syncing = true;
    try {
        h.modal = packet.modal;
        if (h.parked && (h.parked.editor.key !== h.key || h.parked.editor.secure !== h.context?.secure)) {
            const entry = h.parked;
            if (h.editors.get(entry.editor.key) === entry.editor) h.editors.delete(entry.editor.key);
            entry.editor.destroy(); entry.help?.remove();
            h.parked = null;
        }
        if (packet.nodes) {
            const next = new Map();
            for (const data of packet.nodes) next.set(data.id, data);
            const focused = h.editors.get(h.key);
            if (!h.parked && focused && focused.secure === h.context?.secure) {
                let entry = null;
                for (const node of h.nodes.values()) {
                    if (node.editor === focused) { entry = node; break; }
                }
                if (entry) {
                    let park = !compatible(entry, next.get(entry.data.id));
                    for (let parent = entry.element.parentElement; !park && parent && parent !== h.root; parent = parent.parentElement) {
                        const ancestor = h.nodes.get(parent.dataset.runeNode);
                        park = !ancestor || !compatible(ancestor, next.get(ancestor.data.id));
                    }
                    if (park) {
                        place(h.root, entry.element, null);
                        entry.element.tabIndex = -1;
                        h.nodes.delete(entry.data.id);
                        h.parked = entry;
                    }
                }
            }
            h.editors.clear();
            const previous = new Map();
            for (const data of packet.nodes) {
                let entry = h.nodes.get(data.id);
                if (entry && !compatible(entry, data)) {
                    entry.editor?.destroy(); entry.help?.remove(); entry.after?.remove(); entry.element.remove();
                    h.nodes.delete(data.id); entry = null;
                }
                if (!entry && h.parked && h.parked.editor.key === data.key && data.edit
                    && !data.edit.readonly && h.parked.editor.secure === data.edit.secure) {
                    entry = h.parked;
                    h.parked = null;
                    entry.element.dataset.runeNode = data.id;
                }
                if (!entry) entry = create(h, data);
                h.nodes.set(data.id, entry);
                update(h, entry, data);
                if (entry.editor) h.editors.set(data.key, entry.editor);
                const parent = h.nodes.get(data.parent)?.element ?? h.root;
                const after = previous.get(parent);
                const before = after ? after.nextSibling : parent === h.root ? parent.firstChild : parent.firstChild?.nextSibling;
                if (entry.element.parentElement !== parent || before !== entry.element) place(parent, entry.element, before ?? null);
                if (entry.after && entry.element.nextSibling !== entry.after) parent.insertBefore(entry.after, entry.element.nextSibling);
                previous.set(parent, entry.after ?? entry.element);
            }
            for (const [id, entry] of h.nodes) {
                if (next.has(id)) continue;
                entry.editor?.destroy(); entry.help?.remove(); entry.after?.remove(); entry.element.remove();
                h.nodes.delete(id);
            }
            if (h.parked) h.editors.set(h.parked.editor.key, h.parked.editor);
        }
        const origin = h.canvas.getBoundingClientRect();
        for (const [id, rect] of packet.bounds) {
            const element = h.nodes.get(id)?.element;
            if (!element) continue;
            const values = { insetInlineStart: `${origin.x + rect[0]}px`, insetBlockStart: `${origin.y + rect[1]}px`,
                inlineSize: `${rect[2]}px`, blockSize: `${rect[3]}px` };
            for (const [name, value] of Object.entries(values)) if (element.style[name] !== value) element.style[name] = value;
        }
        h.focused = packet.focus;
        if (owned) {
            const target = h.nodes.get(packet.focus)?.element
                ?? (packet.focus === null && h.parked ? h.parked.element : h.root);
            if (document.activeElement !== target) target.focus({ preventScroll: true });
        }
    } finally { h.syncing = false; }
}

export function destroyAccessibility(h) {
    for (const entry of h.nodes.values()) entry.editor?.destroy();
    h.parked?.editor.destroy();
    h.parked = null;
    h.context = null;
    h.keys.clear();
    h.editors.clear();
    h.nodes.clear();
    h.root.remove();
}
