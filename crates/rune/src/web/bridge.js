// One browser shell owns semantic DOM, editing services and their event registrations.
import { mountAccessibility, destroyAccessibility } from './accessibility.js';

export function mount(canvas, callback) {
    const h = { canvas, key: '', listeners: [], syncing: false, disposed: false };
    const browserKeys = new Set(['l', 'r', 'f', 't', 'w', 'n', '+', '=', '-', '0']);
    h.passThrough = (event, editor = false) => (event.ctrlKey && event.altKey)
        || (!editor && event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight'))
        || ((event.metaKey || event.ctrlKey) && browserKeys.has(event.key.toLowerCase()));
    h.send = event => {
        const message = JSON.stringify(event);
        if (h.syncing) queueMicrotask(() => { if (!h.disposed) callback(message); });
        else if (!h.disposed) callback(message);
    };
    h.listen = (target, name, handler, options) => {
        target.addEventListener(name, handler, options);
        h.listeners.push(() => target.removeEventListener(name, handler, options));
    };
    mountAccessibility(h);
    canvas.tabIndex = -1;
    canvas.setAttribute('aria-hidden', 'true');
    // Keep winit from taking focus away from an editor during a pointer/touch event.
    h.listen(canvas, 'pointerdown', event => event.preventDefault(), { capture: true });
    h.listen(canvas, 'keydown', event => event.preventDefault());
    h.listen(canvas, 'wheel', event => event.preventDefault(), { passive: false });
    h.listen(document, 'gesturestart', event => event.preventDefault(), { passive: false });
    queueMicrotask(() => {
        if (!h.disposed && document.activeElement === document.body) h.root.focus({ preventScroll: true });
    });
    return h;
}


// The PNG the last paste carried, bounded by the caller and consumed once.
export async function pastedImage(h, max) {
    const file = h.image;
    h.image = undefined;
    return file && file.size <= max ? new Uint8Array(await file.arrayBuffer()) : undefined;
}

export function destroy(h) {
    h.disposed = true;
    h.image = undefined;
    for (const remove of h.listeners) remove();
    destroyAccessibility(h);
}
