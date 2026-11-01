// Focus, read-only Return, viewport bounds, text view paging, the text view button,
// composition hold, and whether a clipped mirror steals clicks.
// Serves the web shell modules and drives headless Chromium. No wasm app.
// PLAYWRIGHT_MODULE selects playwright or playwright-core. CHROMIUM_PATH selects the browser.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';

const root = resolve('crates/rune/src/platform/web');
const playwright = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const chromium = playwright.chromium ?? playwright.default?.chromium;
const server = createServer(async (request, response) => {
    const pathname = new URL(request.url, 'http://localhost').pathname;
    if (pathname === '/harness.html') {
        response.setHeader('Content-Type', 'text/html');
        response.end('<!doctype html><title>focus</title><body></body>');
        return;
    }
    const rel = pathname.startsWith('/src/platform/web/') ? pathname.slice('/src/platform/web'.length) : pathname;
    const path = resolve(root, '.' + (rel === '/' ? '/bridge.js' : rel));
    if (!path.startsWith(root + sep) && path !== root) { response.writeHead(403).end(); return; }
    try {
        const bytes = await readFile(path);
        response.setHeader('Content-Type', extname(path) === '.js' ? 'text/javascript' : 'text/html');
        response.end(bytes);
    } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const url = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROMIUM_PATH,
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
});
try {
    const page = await browser.newPage({ viewport: { width: 390, height: 760 } });
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(`${url}/harness.html`);
    const result = await page.evaluate(async (base) => {
        const frames = n => new Promise(resolve => {
            const step = left => left ? requestAnimationFrame(() => step(left - 1)) : resolve();
            step(n);
        });
        const bridge = await import(new URL('bridge.js', base).href);
        const input = await import(new URL('text_input.js', base).href);
        const platform = await import(new URL('platform.js', base).href);
        const same = await import(new URL('platform.js', new URL('text_view.js', base)).href);
        const out = { module: platform.setViewportInset === same.setViewportInset };
        const parent = document.createElement('div');
        parent.style.cssText = 'position:static !important;width:320px;height:400px';
        const canvas = document.createElement('canvas');
        canvas.width = 320; canvas.height = 400;
        canvas.style.cssText = 'display:block;width:100%;height:100%';
        parent.append(canvas);
        document.body.append(parent);
        const events = [];
        const stopViewport = platform.viewport(canvas, message => events.push(JSON.parse(message)), true);
        const handle = bridge.mount(canvas, message => events.push(JSON.parse(message)), true, false);
        await frames(2);
        const button = handle.textView.control;
        const canvasTop = canvas.getBoundingClientRect().top;
        const buttonBottom = button.getBoundingClientRect().bottom;
        out.buttonFollows = !!(button.compareDocumentPosition(handle.a11y) & Node.DOCUMENT_POSITION_PRECEDING);
        out.canvasClearsButton = canvasTop >= buttonBottom - 0.5;
        out.buttonHeight = buttonBottom - button.getBoundingClientRect().top;
        const edit = { text: 'read only', base: 0, anchor: 0, focus: 0, secure: false, readonly: true, single: true, placeholder: '' };
        const packet = { items: [
            { kind: 'group', path: 'group', rect: [0, 0, 200, 80] },
            { kind: 'textbox', path: 'field', rect: [8, 8, 180, 32], name: 'Note', tab: 0, parent: 'group', edit },
            { kind: 'button', path: 'go', rect: [8, 48, 80, 28], name: 'Go', text: 'Go', tab: 0, parent: 'group' },
        ], bounds: [['field', [8, 8, 180, 32]], ['go', [8, 48, 80, 28]]], focus: { path: 'field', link: null }, modal: false };
        bridge.syncA11y(handle, JSON.stringify(packet));
        input.sync(handle, 'field', 'read only', 0, 0, 0, false);
        const field = handle.editors.get('field').element;
        field.focus();
        const before = events.length;
        field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }));
        out.readonlyEnter = events.slice(before).some(event => event.kind === 'key' && event.key === 'Enter');
        field.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: null }));
        await frames(2);
        out.intraTreeBlur = events.slice(before).some(event => event.kind === 'a11y_blur');
        window.dispatchEvent(new Event('blur'));
        field.blur();
        await frames(2);
        out.windowBlur = events.some(event => event.kind === 'a11y_blur');
        window.dispatchEvent(new Event('focus'));
        await frames(1);
        out.restored = document.activeElement === field;
        canvas.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true }));
        field.blur();
        await frames(1);
        out.canvasBlur = events.some(event => event.kind === 'a11y_blur');
        await frames(3);
        const host = document.createElement('div');
        host.style.cssText = 'position:fixed;inset:0';
        const full = document.createElement('canvas');
        full.style.cssText = 'position:fixed;left:0;top:0;width:200px;height:200px';
        document.body.append(full);
        const fullEvents = [];
        const fullHandle = bridge.mount(full, message => fullEvents.push(JSON.parse(message)), false, false);
        bridge.syncA11y(fullHandle, JSON.stringify({ items: [
            { kind: 'button', path: 'go', rect: [20, 30, 80, 28], name: 'Go', text: 'Go', tab: 0 },
            { kind: 'text', path: 'links', rect: [0, 80, 200, 20], runs: [
                { text: 'URL', href: 'https://example.com/', path: 'links', index: 0, tab: 0 },
                { text: 'Unsafe', href: 'javascript:alert(1)', path: 'links', index: 1, tab: 0 },
            ] },
        ], bounds: [['go', [20, 30, 80, 28]]], focus: null, modal: false }));
        const links = fullHandle.a11yNodes.get('links').content.querySelectorAll('a');
        const click = new MouseEvent('click', { bubbles: true, cancelable: true });
        out.urlActionCancelled = !links[0].dispatchEvent(click);
        out.urlAction = fullEvents.some(event => event.kind === 'a11y_link' && event.path === 'links' && event.link === 0);
        out.unsafeUrl = links[1].getAttribute('href') === '#';
        const node = fullHandle.a11yNodes.get('go').element;
        const beforeMove = node.style.insetInlineStart;
        full.style.left = '48px';
        full.style.top = '36px';
        window.visualViewport.dispatchEvent(new Event('scroll'));
        out.boundsMoved = node.style.insetInlineStart !== beforeMove;
        out.boundsLeft = node.style.insetInlineStart;
        const hit = document.elementFromPoint(full.getBoundingClientRect().left + 40, full.getBoundingClientRect().top + 40);
        out.clipHit = hit === full || hit === document.body || hit === document.documentElement ? 'canvas' : (hit?.dataset?.runeNode || hit?.tagName || 'none');
        handle.a11y.moveBefore = undefined;
        const editor = handle.editors.get('field').editor;
        editor.composing = true;
        const parentBefore = field.parentElement;
        bridge.syncA11y(handle, JSON.stringify({ items: [
            { kind: 'button', path: 'group', rect: [0, 0, 200, 80], name: 'Group', text: 'Group', tab: 0 },
            { kind: 'textbox', path: 'field', rect: [8, 8, 180, 32], name: 'Note', tab: 0, parent: 'group', edit },
        ], bounds: [], focus: { path: 'field', link: null }, modal: false }));
        out.compositionStayed = field.parentElement === parentBefore && field.isConnected;
        field.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
        out.compositionRelease = events.some(event => event.kind === 'a11y_release');
        await frames(3);
        handle.textView.on = true;
        handle.a11y.setAttribute('data-rune-text-view', 'on');
        handle.a11y.style.overflowY = 'auto';
        handle.a11y.style.blockSize = '80px';
        const tall = document.createElement('div');
        tall.dataset.runeNode = 'tall';
        tall.style.blockSize = '400px';
        handle.a11y.append(tall);
        handle.a11y.scrollTop = handle.a11y.scrollHeight;
        handle.a11y.dispatchEvent(new Event('scroll'));
        out.textViewPage = events.some(event => event.kind === 'text_view_page' && event.edge === 'end');
        await frames(3);
        const pages = () => events.filter(event => event.kind === 'text_view_page' && event.edge === 'end').length;
        const beforePage = pages();
        handle.a11y.dispatchEvent(new WheelEvent('wheel', { deltaY: 10, bubbles: true }));
        out.repeatedTextViewPage = pages() === beforePage + 1;
        await frames(3);
        const touchEvent = (type, touches) => {
            const event = new Event(type, { bubbles: true });
            Object.defineProperty(event, 'touches', { value: touches });
            handle.a11y.dispatchEvent(event);
        };
        const beforeTouch = pages();
        touchEvent('touchstart', [{ identifier: 1, clientY: 120 }]);
        touchEvent('touchmove', [{ identifier: 1, clientY: 80 }]);
        out.touchTextViewPage = pages() === beforeTouch + 1;
        touchEvent('touchend', []);
        await frames(3);
        const beforePinch = pages();
        touchEvent('touchstart', [{ identifier: 1, clientY: 120 }, { identifier: 2, clientY: 140 }]);
        touchEvent('touchmove', [{ identifier: 1, clientY: 80 }, { identifier: 2, clientY: 170 }]);
        out.pinchDoesNotPage = pages() === beforePinch;
        touchEvent('touchend', []);
        const earlyCanvas = document.createElement('canvas');
        const lateEvents = [];
        const stopEarly = platform.viewport(earlyCanvas, message => lateEvents.push(message), false);
        stopEarly();
        await Promise.resolve();
        out.earlyViewportDisposed = lateEvents.length === 0 && !earlyCanvas.style.inlineSize;
        bridge.destroy(handle);
        stopViewport();
        out.parentRestored = parent.style.position === 'static' && parent.style.getPropertyPriority('position') === 'important';
        bridge.destroy(fullHandle);
        parent.remove();
        full.remove();
        host.remove();
        return out;
    }, `${url}/src/platform/web/`);
    assert.equal(result.module, true, 'platform.js is one module instance');
    assert.equal(result.buttonFollows, true, 'the text view button follows the mirror');
    assert.ok(result.buttonHeight > 0, 'the text view button has a height');
    assert.equal(result.canvasClearsButton, true, `embedded canvas clears the button (${result.buttonHeight})`);
    assert.equal(result.readonlyEnter, true, 'read-only Enter reaches the runtime');
    assert.equal(result.intraTreeBlur, false, 'a null relatedTarget inside the tree does not blur');
    assert.equal(result.windowBlur, false, 'leaving the window does not blur app focus');
    assert.equal(result.restored, true, 'returning to the window restores the field');
    assert.equal(result.canvasBlur, false, 'a canvas press does not blur app focus');
    assert.equal(result.boundsMoved, true, `a viewport pan refreshes bounds (${result.boundsLeft})`);
    assert.equal(result.compositionStayed, true, 'a structural update keeps a composing field put');
    assert.equal(result.compositionRelease, true, 'ending the composition releases the held tree');
    assert.equal(result.textViewPage, true, 'scrolling a text view to its end asks for the next page');
    assert.equal(result.repeatedTextViewPage, true, 'another edge gesture can page even when the first and last nodes do not change');
    assert.equal(result.touchTextViewPage, true, 'a touch swipe pages when the scrollable DOM is still at the edge');
    assert.equal(result.pinchDoesNotPage, true, 'a two-finger pinch does not page the text view');
    assert.equal(result.urlActionCancelled, true, 'a URL anchor must not bypass the application link handler');
    assert.equal(result.urlAction, true, 'a URL anchor dispatches its retained link action');
    assert.equal(result.unsafeUrl, true, 'unsafe link identifiers do not become navigable URLs');
    assert.equal(result.earlyViewportDisposed, true, 'a disposed viewport does not run its queued initialization');
    assert.equal(result.parentRestored, true, 'teardown restores the embedded parent position and priority');
    assert.equal(result.clipHit, 'canvas', `a clipped mirror must not take the click (hit ${result.clipHit})`);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ webFocus: 'passed', checks: Object.keys(result) }));
} finally {
    await browser.close();
    await new Promise(done => server.close(done));
}
