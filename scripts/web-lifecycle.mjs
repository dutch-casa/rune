import assert from 'node:assert/strict';

export async function lifecycleProof(page) {
    const setup = await page.evaluate(async () => {
        const source = performance.getEntriesByType('resource').map(entry => new URL(entry.name))
            .find(url => url.pathname.endsWith('/src/web/bridge.js'));
        const bridge = await import(source.href);
        const platform = await import(new URL('platform.js', source).href);
        const host = document.createElement('section');
        host.style.cssText = 'margin-inline-start:40px;margin-block-start:30px;inline-size:360px;block-size:180px';
        const text = document.createElement('p');
        text.textContent = 'Host text outside Rune';
        const canvas = document.createElement('canvas');
        canvas.setAttribute('aria-hidden', 'false');
        canvas.tabIndex = 3;
        canvas.style.cssText = 'inline-size:280px;block-size:80px';
        host.append(text, canvas);
        document.body.append(host);
        const events = [];
        const handle = bridge.mount(canvas, event => events.push(JSON.parse(event)), true, false);
        bridge.syncCopy(handle, 'Canvas selection');
        const current = document.activeElement;
        if (current instanceof HTMLElement) current.blur();
        const range = document.createRange();
        range.selectNodeContents(text);
        getSelection().removeAllRanges();
        getSelection().addRange(range);
        window.__lifecycle = { bridge, platform, host, text, canvas, handle, events };
        return { activeBody: document.activeElement === document.body };
    });
    try {
        assert.equal(setup.activeBody, true);
        await page.keyboard.press('Meta+C');
        assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'Host text outside Rune', 'a host selection must not become stale canvas text');
        const box = await page.evaluate(() => window.__lifecycle.canvas.getBoundingClientRect().toJSON());
        await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
        assert.equal(await page.evaluate(() => getSelection().isCollapsed), true, 'a primary canvas press clears the older DOM range');
        await page.evaluate(() => window.__lifecycle.bridge.activate(window.__lifecycle.handle));
        await page.keyboard.press('Meta+C');
        assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'Canvas selection', 'a new canvas selection owns copying');
        const cleanup = await page.evaluate(async () => {
            const p = window.__lifecycle;
            const pending = [];
            const dispose = p.platform.viewport(p.canvas, message => pending.push(message), true);
            dispose();
            await Promise.resolve();
            p.bridge.destroy(p.handle);
            const count = p.events.length;
            window.dispatchEvent(new Event('resize'));
            await Promise.resolve();
            return { pending: pending.length, late: p.events.length - count, connected: p.handle.a11y.isConnected,
                position: p.host.style.position, aria: p.canvas.getAttribute('aria-hidden'), tab: p.canvas.tabIndex };
        });
        assert.deepEqual(cleanup, { pending: 0, late: 0, connected: false, position: '', aria: 'false', tab: 3 });
        const geometry = await page.evaluate(async () => {
            const p = window.__lifecycle;
            p.host.style.direction = 'rtl';
            p.host.style.writingMode = 'vertical-rl';
            p.canvas.style.cssText = 'position:fixed;inline-size:10px;block-size:10px';
            const dispose = p.platform.viewport(p.canvas, message => p.events.push(JSON.parse(message)), false);
            await Promise.resolve();
            const rect = p.canvas.getBoundingClientRect();
            const viewport = visualViewport;
            const result = { actual: [rect.x, rect.y, rect.width, rect.height],
                expected: [viewport.offsetLeft, viewport.offsetTop, viewport.width, viewport.height] };
            dispose();
            return result;
        });
        for (let i = 0; i < 4; i++) assert.ok(Math.abs(geometry.actual[i] - geometry.expected[i]) <= 1, 'viewport geometry uses physical browser coordinates');
    } finally {
        await page.evaluate(() => {
            const p = window.__lifecycle;
            p.bridge.destroy(p.handle);
            p.host.remove();
            delete window.__lifecycle;
        });
    }
    console.log(JSON.stringify({ browserLifecycle: 'passed', checks: ['host and canvas copy ownership', 'deferred disposal', 'embedded restoration', 'RTL vertical viewport geometry'] }));
}
