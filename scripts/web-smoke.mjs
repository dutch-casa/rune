// Functional browser proof for web-hello. Requires Playwright and its Chromium browser.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, mkdir } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
import { inputSmoke } from './web-input-smoke.mjs';
const { chromium, webkit } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const engine = process.env.WEB_ENGINE || 'chromium';
const root = resolve(process.argv[2] || 'target/web');
const output = resolve(process.argv[3] || 'target/web-proof');
await mkdir(output, { recursive: true });
const server = createServer(async (request, response) => {
    const path = resolve(root, '.' + new URL(request.url, 'http://localhost').pathname.replace(/\/$/, '/index.html'));
    if (!path.startsWith(root + sep)) { response.writeHead(403).end(); return; }
    try {
        const bytes = await readFile(path);
        response.setHeader('Content-Type', ({ '.wasm': 'application/wasm', '.js': 'text/javascript', '.html': 'text/html' })[extname(path)] || 'application/octet-stream');
        response.end(bytes);
    } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await (engine === 'webkit' ? webkit.launch({ headless: true }) :
    chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_PATH,
        args: ['--enable-unsafe-webgpu', ...(process.platform === 'darwin' ? ['--use-angle=metal'] : [])] }));
const errors = [];
try {
    const context = await browser.newContext({ viewport: { width: 390, height: 760 }, deviceScaleFactor: 3,
        hasTouch: true, isMobile: true,
        permissions: engine === 'chromium' ? ['clipboard-read', 'clipboard-write'] : [], colorScheme: 'dark' });
    const page = await context.newPage();
    const canvas = page.locator('canvas');
    page.on('pageerror', error => { errors.push(error.message); console.log(error.message); });
    page.on('console', message => { if (message.type() === 'error') { errors.push(message.text()); console.log(message.text()); } });
    const ax = engine === 'chromium' ? await context.newCDPSession(page) : null;
    const axNode = async locator => {
        await locator.evaluate(element => element.setAttribute('data-proof-ax', ''));
        try {
            const { root } = await ax.send('DOM.getDocument');
            const { nodeId } = await ax.send('DOM.querySelector', { nodeId: root.nodeId, selector: '[data-proof-ax]' });
            const { node } = await ax.send('DOM.describeNode', { nodeId });
            const { nodes } = await ax.send('Accessibility.getPartialAXTree', { nodeId, fetchRelatives: false });
            return nodes.find(item => item.backendDOMNodeId === node.backendNodeId);
        } finally {
            await locator.evaluate(element => element.removeAttribute('data-proof-ax'));
        }
    };
    await page.addInitScript(() => {
        // Chromium's device emulation leaves devicePixelContentBoxSize at the host DPR.
        // Exercise winit's Safari fallback (CSS content box × emulated DPR) instead.
        delete ResizeObserverEntry.prototype.devicePixelContentBoxSize;
        window.__proof = { submits: 0, rafs: 0, gpuErrors: [] };
        const raf = window.requestAnimationFrame;
        window.requestAnimationFrame = callback => { window.__proof.rafs++; return raf.call(window, callback); };
        if (!globalThis.GPUQueue || !globalThis.GPUAdapter) return;
        const submit = GPUQueue.prototype.submit;
        GPUQueue.prototype.submit = function (...args) { window.__proof.submits++; return submit.apply(this, args); };
        const request = GPUAdapter.prototype.requestDevice;
        GPUAdapter.prototype.requestDevice = async function (...args) {
            const device = await request.apply(this, args);
            device.addEventListener('uncapturederror', e => window.__proof.gpuErrors.push(e.error.message));
            return device;
        };
    });
    const url = `http://127.0.0.1:${server.address().port}`;
    await page.goto(url);
    if (engine === 'webkit' && !await page.evaluate(async () => !!(await navigator.gpu?.requestAdapter()))) {
        console.log(JSON.stringify({ engine, available: false, reason: 'Headless WebKit has no WebGPU adapter' }));
    } else {
    if (!process.env.WEB_INPUT_ONLY) {
    await page.waitForFunction(() => window.__proof?.submits > 0, { timeout: 30000 });
    const mirror = page.locator('[data-rune-a11y]');
    await mirror.waitFor();
    assert.equal(await canvas.getAttribute('aria-hidden'), 'true');
    assert.notEqual(await mirror.evaluate(element => getComputedStyle(element).opacity), '0');
    const send = page.getByRole('button', { name: 'Send', exact: true });
    await send.waitFor();
    assert.equal(await send.count(), 1);
    if (ax) {
        assert.equal((await axNode(mirror)).role.value, 'document');
        assert.equal((await axNode(send)).name.value, 'Send');
        assert.equal((await axNode(canvas)).ignored, true);
    }
    await page.waitForTimeout(1000); // Let the initial appearance finish before measuring idle.
    assert.deepEqual(await page.locator('canvas').evaluate(c => [c.width, c.height]), [1170, 2280]);
    const beforeIdle = await page.evaluate(() => window.__proof.submits);
    const beforeIdleRaf = await page.evaluate(() => window.__proof.rafs);
    await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => window.__proof.submits), beforeIdle, 'idle must not submit frames');
    assert.equal(await page.evaluate(() => window.__proof.rafs), beforeIdleRaf, 'idle must not request animation frames');
    await page.screenshot({ path: `${output}/web-dark.png` });
    const beforePan = await page.screenshot();
    const cdp = engine === 'chromium' ? await context.newCDPSession(page) : null;
    const touch = async (type, y) => cdp ? cdp.send('Input.dispatchTouchEvent', {
        type, touchPoints: type === 'touchEnd' ? [] : [{ x: 180, y }],
    }) : page.locator('canvas').dispatchEvent(({ touchStart: 'pointerdown', touchMove: 'pointermove', touchEnd: 'pointerup' })[type],
        { pointerType: 'touch', pointerId: 1, isPrimary: true, clientX: 180, clientY: y, buttons: type === 'touchEnd' ? 0 : 1 });
    await touch('touchStart', 180);
    for (let i = 1; i <= 10; i++) {
        await touch('touchMove', 180 + 30 * i);
        await page.waitForTimeout(16);
    }
    await touch('touchEnd', 480);
    const releaseSubmits = await page.evaluate(() => window.__proof.submits);
    await page.waitForTimeout(250);
    assert.ok(await page.evaluate(() => window.__proof.submits) > releaseSubmits, 'touch release continues momentum');
    assert.ok(!beforePan.equals(await page.screenshot()), 'touch pan moves rendered content');
    assert.notEqual(await page.evaluate(() => document.activeElement?.tagName), 'TEXTAREA', 'a pan must not focus an editor');
    await page.touchscreen.tap(100, 720);
    await page.waitForFunction(() => document.activeElement?.tagName === 'TEXTAREA');
    await page.keyboard.insertText('Bonjour 🦀');
    assert.equal(await page.locator('textarea').inputValue(), 'Bonjour 🦀');
    await page.keyboard.press('Enter');
    assert.equal(await page.locator('textarea').inputValue(), '');
    await page.touchscreen.tap(330, 36);
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'Bonjour 🦀', 'send updates the actual model');
    await page.touchscreen.tap(100, 720);
    await page.keyboard.insertText('x'.repeat(1200) + ' tail');
    assert.ok((await page.locator('textarea').inputValue()).length <= 1024, 'mirror is a sliding context');
    await page.locator('textarea').evaluate(field => {
        field.setSelectionRange(field.value.length - 4, field.value.length);
        field.setRangeText('corrected', field.selectionStart, field.selectionEnd, 'end');
        field.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText', data: 'corrected' }));
    });
    await page.keyboard.press('Enter');
    await page.touchscreen.tap(330, 36);
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'x'.repeat(1200) + ' corrected');
    await page.touchscreen.tap(100, 720);
    const beforeMissingGlyph = await page.evaluate(() => window.__proof.submits);
    await page.locator('textarea').evaluate(field => {
        field.dispatchEvent(new CompositionEvent('compositionstart', { data: '' }));
        field.value = '日本語'; field.setSelectionRange(3, 3);
        field.dispatchEvent(new CompositionEvent('compositionupdate', { data: '日本語' }));
        field.dispatchEvent(new InputEvent('input', { data: '日本語', isComposing: true }));
        field.dispatchEvent(new CompositionEvent('compositionend', { data: '日本語' }));
    });
    assert.equal(await page.locator('textarea').inputValue(), '日本語');
    await page.waitForFunction(previous => window.__proof.submits > previous, beforeMissingGlyph);
    await page.keyboard.press('Meta+A');
    await page.keyboard.insertText('replacement');
    assert.equal(await page.locator('textarea').inputValue(), 'replacement');
    // Browser paste is independent of async clipboard-read permission.
    await page.locator('textarea').evaluate(field => {
        const clipboardData = new DataTransfer(); clipboardData.setData('text/plain', ' pasted');
        field.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true }));
    });
    assert.equal(await page.locator('textarea').inputValue(), 'replacement pasted');
    const popup = context.waitForEvent('page');
    await page.touchscreen.tap(90, 36);
    const linked = await popup;
    await linked.waitForLoadState('domcontentloaded');
    assert.ok(linked.url().includes('rust-lang.org'));
    assert.equal(await linked.evaluate(() => window.opener), null);
    await linked.close();
    await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
    await page.setViewportSize({ width: 430, height: 540 });
    await page.waitForFunction(() => document.querySelector('canvas')?.height === 1620);
    await page.waitForTimeout(400);
    await page.screenshot({ path: `${output}/web-light-keyboard-size.png` });
    const gpuErrors = await page.evaluate(() => window.__proof.gpuErrors);
    assert.deepEqual(gpuErrors, []);
    assert.deepEqual(errors.filter(e => !e.includes('404')), []);
    console.log(JSON.stringify({ ok: true, engine, idleSubmits: 0, idleRafs: 0, dpr: 3, sizingPath: 'Safari fallback', checks: ['WebGPU render', 'idle', 'touch focus',
        'touch momentum', 'Unicode input', 'send', 'clipboard copy', 'sliding context', 'predictive replacement',
        'composition', 'selection replacement', 'paste', 'noopener link', 'theme', 'resize'], gpuErrors }));
    }
    await page.setViewportSize({ width: 390, height: 760 });
    await inputSmoke(page, url);
    const mirror = page.locator('[data-rune-a11y]');
    await page.getByRole('combobox', { name: 'Fruit', exact: true }).waitFor();
    const select = page.getByRole('combobox', { name: 'Fruit', exact: true });
    const editor = page.getByRole('textbox', { name: 'Captioned editor', exact: true });
    const visibleCheck = page.getByRole('checkbox', { name: 'Visible checkbox', exact: true });
    const hiddenCheck = page.getByRole('checkbox', { name: 'Hidden checkbox', exact: true });
    const button = page.getByRole('button', { name: 'Button', exact: true });
    assert.equal(await canvas.getAttribute('aria-hidden'), 'true');
    assert.notEqual(await mirror.evaluate(element => getComputedStyle(element).opacity), '0');
    assert.equal(await visibleCheck.getAttribute('aria-label'), null);
    assert.equal(await visibleCheck.textContent(), 'Visible checkbox');
    assert.equal(await hiddenCheck.getAttribute('aria-label'), 'Hidden checkbox');
    assert.equal(await page.getByRole('group', { name: 'Captioned editor', exact: true }).count(), 0);
    assert.equal(await mirror.getByRole('group', { name: '', exact: true }).count(), 0);
    const captionFor = async (control, name) => {
        const relation = await control.evaluate((element, name) => {
            const id = element.getAttribute('aria-labelledby');
            const caption = id && document.getElementById(id);
            const root = element.closest('[data-rune-a11y]');
            const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
            let copies = 0;
            while (walker.nextNode()) if (walker.currentNode.textContent.trim() === name) copies++;
            return { id, text: caption?.textContent.trim(), copies, override: element.getAttribute('aria-label') };
        }, name);
        assert.ok(relation.id, `${name} must reference its caption`);
        assert.equal(relation.text, name);
        assert.equal(relation.copies, 1, `${name} must occur once in the semantic DOM`);
        assert.equal(relation.override, null);
        return relation.id;
    };
    await captionFor(select, 'Fruit');
    const captionId = await captionFor(editor, 'Captioned editor');
    const editorId = await editor.getAttribute('id');
    await editor.evaluate(element => { window.__captionEditor = element; });
    const namedEditor = page.getByRole('textbox', { name: 'Nickname', exact: true });
    assert.equal(await namedEditor.getAttribute('aria-labelledby'), null);
    await captionFor(page.getByRole('group', { name: 'Travel dates', exact: true }), 'Travel dates');
    assert.equal(await page.getByRole('textbox', { name: 'Start date', exact: true }).count(), 1);
    assert.equal(await page.getByRole('textbox', { name: 'End date', exact: true }).count(), 1);
    const helpId = await button.getAttribute('aria-describedby');
    assert.ok(helpId);
    assert.equal(await page.locator(`[id="${helpId}"]`).textContent(), 'Button help');
    if (ax) {
        assert.equal((await axNode(mirror)).role.value, 'document');
        assert.equal((await axNode(canvas)).ignored, true);
    }
    if (ax) {
        const selectAx = await axNode(select);
        assert.equal(selectAx.name.value, 'Fruit');
        assert.equal(selectAx.value.value, 'Banana');
        assert.equal((await axNode(editor)).name.value, 'Captioned editor');
        assert.equal((await axNode(visibleCheck)).name.value, 'Visible checkbox');
        assert.equal((await axNode(hiddenCheck)).name.value, 'Hidden checkbox');
        assert.equal((await axNode(button)).description.value, 'Button help');
        await visibleCheck.focus();
        await page.keyboard.press('Space');
        await page.waitForFunction(() => document.querySelector('[role="checkbox"]')?.getAttribute('aria-checked') === 'true');
        assert.equal((await axNode(visibleCheck)).properties.find(property => property.name === 'checked').value.value, 'true');
    }
    await button.evaluate(element => element.click());
    await page.waitForFunction(id => !document.getElementById(id), helpId);
    assert.equal(await button.getAttribute('aria-describedby'), null);
    if (ax) assert.equal((await axNode(button)).description?.value, undefined);
    assert.equal(await page.locator(`[id="${helpId}"]`).count(), 0);
    const retainedEditor = page.locator(`[id="${editorId}"]`);
    assert.equal(await page.locator(`[id="${captionId}"]`).count(), 0);
    assert.equal(await retainedEditor.getAttribute('aria-labelledby'), null);
    assert.equal(await retainedEditor.evaluate(element => element === window.__captionEditor), true);
    await button.evaluate(element => element.click());
    await editor.waitFor();
    await captionFor(editor, 'Captioned editor');
    assert.equal(await editor.evaluate(element => element === window.__captionEditor), true);

    await page.goto(new URL('?input-proof&disputed-review', url).href);
    await page.getByRole('button', { name: 'Before links', exact: true }).waitFor();
    const expectFocus = async (role, name) => {
        await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        assert.equal(await page.getByRole(role, { name, exact: true }).evaluate(element => element === document.activeElement),
            true, `native Tab should reach ${name}`);
    };
    await page.getByRole('button', { name: 'Before links', exact: true }).focus();
    for (const [key, role, name] of [
        ['Tab', 'link', 'Alpha'], ['Tab', 'link', 'Beta'], ['Tab', 'button', 'After links'],
        ['Shift+Tab', 'link', 'Beta'], ['Shift+Tab', 'link', 'Alpha'], ['Shift+Tab', 'button', 'Before links'],
    ]) {
        await page.keyboard.press(key);
        await expectFocus(role, name);
    }
    await page.keyboard.press('Tab');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__disputedActions?.first === 1);
    await page.keyboard.press('Tab');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__disputedActions?.second === 1);
    assert.equal(await page.evaluate(() => window.__disputedActions.first), 1);
    const launch = page.getByRole('button', { name: 'Launch', exact: true });
    await launch.focus();
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__disputedActions?.launch === 1);
    if (ax) {
        assert.equal((await axNode(launch)).name.value, 'Launch');
        for (const [role, name] of [['option', 'Option label Option support'], ['tab', 'Tab label Tab support'],
            ['option', 'Distinct option name'], ['tab', 'Distinct tab name']]) {
            assert.equal((await axNode(page.getByRole(role, { name, exact: true }))).name.value, name);
        }
    }
    if (engine === 'chromium') {
        const mobile = await browser.newContext({ viewport: { width: 390, height: 1000 }, hasTouch: true, isMobile: true });
        try {
            const touch = await mobile.newPage();
            await touch.goto(new URL('?input-proof&disputed-review', url).href);
            await touch.getByRole('button', { name: 'Before links', exact: true }).waitFor();
            const session = await mobile.newCDPSession(touch);
            const before = await touch.evaluate(() => visualViewport.scale);
            await session.send('Input.synthesizePinchGesture', { x: 195, y: 380, scaleFactor: 1.8,
                relativeSpeed: 400, gestureSourceType: 'touch' });
            await touch.waitForFunction(before => visualViewport.scale > before + 0.05, before);
            assert.ok(await touch.evaluate(() => visualViewport.scale) > before + 0.05, 'native touch pinch must zoom');
        } finally { await mobile.close(); }
    }
    await page.evaluate(async () => {
        const source = performance.getEntriesByType('resource').map(entry => new URL(entry.name))
            .find(url => url.pathname.endsWith('/src/web/bridge.js'));
        const bridge = await import(source.href);
        const input = await import(new URL('text_input.js', source).href);
        const canvas = document.createElement('canvas');
        document.body.append(canvas);
        const events = [];
        const handle = bridge.mount(canvas, event => events.push(JSON.parse(event)), true, false);
        const edit = { text: 'abc', base: 0, anchor: 0, focus: 0, secure: false, readonly: false, single: true, placeholder: '' };
        const packet = { items: [
            { k: 'label', p: 'caption', r: [0, 0, 100, 20], t: 'Parked field' },
            { k: 'textbox', p: 'field', r: [0, 20, 100, 30], t: '', name: 'Parked field', labelled_by: 'caption', tab: 0, edit },
        ], bounds: [], focus: { path: 'field', link: null }, modal: false };
        bridge.syncA11y(handle, JSON.stringify(packet));
        input.sync(handle, 'field', 'abc', 0, 0, 0, false);
        const field = handle.editors.get('field').element;
        field.focus();
        window.__parkedProbe = { bridge, handle, canvas, field, packet, events };
    });
    const parked = page.getByRole('textbox', { name: 'Parked field', exact: true });
    assert.equal(await parked.count(), 1);
    await page.evaluate(() => {
        const { bridge, handle } = window.__parkedProbe;
        bridge.syncA11y(handle, JSON.stringify({ items: [], bounds: [], focus: { path: 'field', link: null }, modal: false }));
    });
    assert.equal(await parked.evaluate(element => element === window.__parkedProbe.field && document.activeElement === element), true);
    assert.equal(await parked.getAttribute('aria-labelledby'), null);
    if (ax) assert.equal((await axNode(parked)).name.value, 'Parked field');
    await page.evaluate(() => {
        const { bridge, handle, packet } = window.__parkedProbe;
        bridge.syncA11y(handle, JSON.stringify(packet));
    });
    await captionFor(parked, 'Parked field');
    assert.equal(await parked.evaluate(element => element === window.__parkedProbe.field), true);
    await page.evaluate(() => {
        const { bridge, handle, canvas } = window.__parkedProbe;
        bridge.destroy(handle);
        canvas.remove();
        delete window.__parkedProbe;
    });
    console.log(JSON.stringify({ accessibility: 'passed', checks: ['caption references and lifecycle', 'explicit names',
        'multi-editor groups', 'native link Tab order', 'option and tab content names', 'named pressable', 'native pinch zoom'] }));
    assert.deepEqual(await page.evaluate(() => window.__proof.gpuErrors), []);
    assert.deepEqual(errors.filter(e => !e.includes('404')), []);
    }
} finally {
    await browser.close();
    await new Promise(done => server.close(done));
}
