// A retained semantic editor also owns browser text services. Rust remains the value owner.
const high = n => n >= 0xd800 && n <= 0xdbff;
const low = n => n >= 0xdc00 && n <= 0xdfff;
const fromPaste = event => event.inputType === 'insertFromPaste' || event.inputType === 'insertFromPasteAsQuotation';
function boundary(text, at, forward) {
    if (at > 0 && at < text.length && high(text.charCodeAt(at - 1)) && low(text.charCodeAt(at))) at += forward ? 1 : -1;
    return at;
}

function replacement(old, next, composition, caret) {
    let start = 0;
    while (start < old.length && start < next.length && old[start] === next[start]) start++;
    start = boundary(old, start, false);
    let end = old.length, nextEnd = next.length;
    while (end > start && nextEnd > start && old[end - 1] === next[nextEnd - 1]) { end--; nextEnd--; }
    const adjusted = boundary(old, end, true) - end;
    end += adjusted; nextEnd += adjusted;
    if (composition && caret >= composition.length && next.slice(caret - composition.length, caret) === composition) {
        const from = caret - composition.length, to = caret + old.length - next.length;
        if (from <= to && to <= old.length) {
            if (old === next) { start = from; end = to; nextEnd = caret; }
            else { start = Math.min(start, from); end = Math.max(end, to); nextEnd = end + next.length - old.length; }
        }
    }
    return { start, end, text: next.slice(start, nextEnd) };
}

export function createEditor(h, key, secure) {
    const field = document.createElement(secure ? 'input' : 'textarea');
    if (secure) {
        field.type = 'password';
        field.autocomplete = 'off';
        field.setAttribute('autocorrect', 'off');
        field.setAttribute('autocapitalize', 'off');
        field.spellcheck = false;
    } else field.setAttribute('autocapitalize', 'sentences');
    field.style.fontSize = '16px';
    field.inputMode = matchMedia('(pointer: coarse)').matches ? 'none' : 'text';
    const e = { field, key, secure, value: '', base: 0, composing: false, composition: '', shift: false,
        handledInput: null, paste: null, syncing: false, disposed: false, listeners: [] };
    const listen = (target, name, handler) => {
        target.addEventListener(name, handler);
        e.listeners.push(() => target.removeEventListener(name, handler));
    };
    const send = event => { if (!e.disposed) h.send(event); };
    const keyEvent = (name, event = {}) => {
        if (e.disposed) return;
        const data = { kind: 'key', key: name, code: event.code ?? '', shift: !!event.shiftKey,
            ctrl: !!event.ctrlKey, alt: !!event.altKey, meta: !!event.metaKey, repeat: false };
        h.send(data);
        h.send({ ...data, up: true });
    };
    const changed = (preedit = false) => {
        const backward = field.selectionDirection === 'backward';
        send({ kind: preedit ? 'preedit' : 'edit', key, base: e.base, expected: e.value,
            ...replacement(e.value, field.value, preedit ? e.composition : '', field.selectionEnd),
            anchor: backward ? field.selectionEnd : field.selectionStart,
            focus: backward ? field.selectionStart : field.selectionEnd });
    };
    const commitPaste = paste => {
        if (paste.invalid) { send({ kind: 'sync' }); return; }
        if (paste.custom || secure) {
            h.image = paste.image;
            try { send({ kind: 'paste', key, text: paste.text, html: paste.html, image: !!paste.image,
                base: paste.base, expected: paste.expected, anchor: paste.anchor, focus: paste.focus, composing: paste.composing }); }
            finally {
                if (h.syncing) queueMicrotask(() => { h.image = undefined; });
                else h.image = undefined;
            }
        } else send({ kind: 'paste_edit', key, base: paste.base, expected: paste.expected,
            start: paste.start, end: paste.end, text: paste.text, hasHtml: !!paste.html });
    };
    listen(field, 'input', event => {
        if (e.paste && fromPaste(event)) {
            const paste = e.paste;
            e.paste = null;
            if (secure) field.value = '';
            commitPaste(paste);
            return;
        }
        e.paste = null;
        if (secure) {
            const text = field.value;
            field.value = '';
            if (text) send({ kind: 'secure', key, text });
        } else changed(e.composing || event.isComposing);
    });
    if (!secure) {
        listen(field, 'compositionstart', () => { e.composing = true; e.composition = ''; });
        listen(field, 'compositionupdate', event => {
            e.composition = event.data ?? '';
            queueMicrotask(() => { if (!e.disposed && e.composing) changed(true); });
        });
        listen(field, 'compositionend', () => {
            changed();
            e.composing = false; e.composition = '';
            send({ kind: 'sync' });
        });
        listen(field, 'select', () => {
            if (field.readOnly || h.syncing || e.syncing || e.composing || document.activeElement !== field) return;
            const backward = field.selectionDirection === 'backward';
            send({ kind: 'selection', key, base: e.base, expected: e.value,
                anchor: backward ? field.selectionEnd : field.selectionStart,
                focus: backward ? field.selectionStart : field.selectionEnd });
        });
    }
    listen(field, 'paste', event => {
        if (event.defaultPrevented || field.readOnly || field.disabled) return;
        e.paste = null;
        const context = h.context, data = event.clipboardData;
        if (!context || context.key !== key || !data) return;
        const text = data.getData('text/plain'), html = secure ? '' : data.getData('text/html');
        const invalid = !secure && !e.composing && field.value !== e.value;
        const custom = e.composing || invalid || !text || context.customPaste || (context.richPaste && !!html)
            || (secure && !text.split('\r').join('').split('\n').join(''));
        const backward = field.selectionDirection === 'backward';
        const paste = { text, html, invalid, custom, base: e.base, expected: e.value, composing: e.composing,
            start: field.selectionStart, end: field.selectionEnd,
            anchor: backward ? field.selectionEnd : field.selectionStart, focus: backward ? field.selectionStart : field.selectionEnd,
            image: !secure ? [...data.items].find(i => i.kind === 'file' && i.type === 'image/png')?.getAsFile() : undefined };
        if (e.composing && event.cancelable) {
            // During composition a non-cancellable browser insertion cannot be rolled back
            // without losing the preedit, so keep the existing model-owned paste behavior.
            event.preventDefault();
            commitPaste({ ...paste, custom: true });
            return;
        }
        e.paste = paste;
        // Image-only and empty clipboard contents may produce no beforeinput or input.
        setTimeout(() => {
            if (e.paste !== paste) return;
            e.paste = null;
            if (!event.defaultPrevented && !paste.beforeinput?.defaultPrevented
                && !e.disposed && !field.readOnly && !field.disabled) commitPaste(paste);
        }, 0);
    });
    listen(field, 'beforeinput', event => {
        if (field.readOnly) return;
        if (fromPaste(event) && e.paste) {
            e.paste.beforeinput = event;
            const paste = e.paste, context = h.context;
            if (!context || context.key !== key || context.secure !== secure
                || (!secure && (field.value !== paste.expected || e.base !== paste.base))) {
                paste.invalid = true;
                paste.custom = true;
            } else {
                if (context.customPaste || (context.richPaste && paste.html)) paste.custom = true;
                if (!secure) {
                    const backward = field.selectionDirection === 'backward';
                    paste.start = field.selectionStart; paste.end = field.selectionEnd;
                    paste.anchor = backward ? paste.end : paste.start; paste.focus = backward ? paste.start : paste.end;
                }
            }
            if (paste.custom && event.cancelable && !event.defaultPrevented) {
                event.preventDefault();
                e.paste = null;
                commitPaste(paste);
                return;
            }
        }
        if (event.inputType === 'insertLineBreak' || event.inputType === 'insertParagraph') {
            event.preventDefault();
            const handled = e.handledInput === 'Enter';
            if (handled) e.handledInput = null;
            if (!handled && !e.composing && !event.isComposing) keyEvent('Enter', { shiftKey: e.shift, code: 'Enter' });
        } else if (secure && event.inputType.startsWith('delete')) {
            event.preventDefault();
            const forward = event.inputType.endsWith('Forward');
            const name = forward ? 'Delete' : 'Backspace';
            const handled = e.handledInput === name;
            if (handled) e.handledInput = null;
            if (!handled) keyEvent(name, { altKey: event.inputType.includes('Word'), code: name });
        }
    });
    listen(field, 'keydown', event => {
        e.shift = event.shiftKey;
        if (event.keyCode === 229) { e.handledInput = null; return; }
        if (h.passThrough(event, true) || event.isComposing || e.composing || event.key === 'Tab') return;
        const chord = event.metaKey || event.ctrlKey;
        const navigation = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'];
        if (field.readOnly) {
            const native = navigation.includes(event.key)
                || (chord && ['a', 'c', 'x', 'v', 'insert'].includes(event.key.toLowerCase()));
            if (event.key !== 'Escape' && (!chord || native)) return;
        } else {
            const editing = event.key === 'Enter' || event.key === 'Escape' || navigation.includes(event.key)
                || (secure && (event.key === 'Backspace' || event.key === 'Delete'));
            if (!chord && !editing) return;
            if (chord && event.key.toLowerCase() === 'v') return;
        }
        event.preventDefault();
        if (event.key === 'Enter' || (secure && (event.key === 'Backspace' || event.key === 'Delete'))) {
            e.handledInput = event.key;
        }
        h.forwardKeyDown(event);
    });
    listen(field, 'keyup', event => {
        e.shift = event.shiftKey;
        if (e.handledInput === event.key) e.handledInput = null;
    });
    listen(field, 'focus', () => { if (!h.syncing) e.handledInput = null; });
    listen(field, 'blur', () => { if (!h.syncing) e.shift = false; });
    e.sync = (text, base, anchor, focus) => {
        if (e.composing) return;
        e.syncing = true;
        e.base = base; e.value = secure ? '' : text;
        if (secure) field.value = '';
        else {
            if (field.value !== text) field.value = text;
            if (field.readOnly) { e.syncing = false; return; }
            const start = Math.min(anchor, focus), end = Math.max(anchor, focus), direction = anchor > focus ? 'backward' : 'forward';
            if (field.selectionStart !== start || field.selectionEnd !== end || field.selectionDirection !== direction) field.setSelectionRange(start, end, direction);
        }
        e.syncing = false;
    };
    e.destroy = () => {
        e.disposed = true;
        for (const remove of e.listeners) remove();
        field.value = ''; e.value = ''; e.paste = null;
        field.remove();
    };
    return e;
}
export function sync(h, key, text, base, anchor, focus, secure, customPaste, richPaste) {
    h.key = key;
    h.context = key ? { key, text: secure ? '' : text, base, anchor, focus, secure, customPaste, richPaste } : null;
    const editor = h.editors.get(key);
    if (h.context && editor?.secure === secure) editor.sync(h.context.text, base, anchor, focus);
}

export function activate(h) {
    const editor = h.editors.get(h.key);
    const field = editor?.field ?? h.nodes.get(h.focused)?.element ?? h.root;
    if (editor) field.inputMode = 'text';
    if (document.activeElement !== field) field.focus({ preventScroll: true });
}
