import assert from 'node:assert/strict';

export async function textPresentationProof(page, url, axNode) {
    await textViewProof(page, url);
    await page.goto(new URL('?accessibility-proof&widget=controls', url).href);
    const toggle = page.getByRole('button', { name: 'Text view', exact: true });
    await toggle.click();
    await page.locator('[data-rune-text-view="on"]').waitFor();
    const disclosure = page.getByRole('button', { name: 'Details', exact: true });
    await disclosure.click();
    await page.waitForFunction(() => document.querySelector('[aria-expanded]')?.getAttribute('aria-expanded') === 'true');
    const circle = page.getByRole('progressbar', { name: 'Progress', exact: true });
    await circle.waitFor();
    if (axNode) {
        const node = await axNode(circle);
        assert.equal(node.name.value, 'Progress');
        assert.equal(node.value.value, 25);
    }
    await circle.evaluate(element => {
        const text = element.querySelector('span').firstChild;
        const start = text.textContent.indexOf('25');
        const range = document.createRange();
        range.setStart(text, start);
        range.setEnd(text, start + 2);
        getSelection().removeAllRanges();
        getSelection().addRange(range);
    });
    await disclosure.evaluate(element => element.click());
    await page.waitForFunction(() => document.querySelector('[aria-expanded]')?.getAttribute('aria-expanded') === 'false');
    assert.equal(await page.evaluate(() => getSelection().toString()), '25', 'an unrelated update must preserve selected progress text');
    await toggle.click();
    await page.locator('[data-rune-text-view="off"]').waitFor();
}

async function textViewProof(page, url) {
    await page.goto(url);
    const composer = page.getByRole('textbox', { name: 'Message', exact: true });
    await composer.waitFor();
    await composer.focus();
    await page.keyboard.insertText('Retained draft');
    const editor = await composer.elementHandle();
    const toggle = page.getByRole('button', { name: 'Text view', exact: true });
    await toggle.focus();
    await page.keyboard.press('Space');
    const view = page.locator('[data-rune-text-view="on"]');
    await view.waitFor();
    assert.equal(await composer.evaluate((current, previous) => current === previous, editor), true);
    assert.equal(await composer.inputValue(), 'Retained draft');
    await page.setViewportSize({ width: 320, height: 568 });
    const style = await page.addStyleTag({ content: `
        [data-rune-text-view="on"], [data-rune-text-view="on"] * {
            font-family: monospace !important;
            font-size: 2rem !important;
            line-height: 1.5 !important;
            letter-spacing: .12em !important;
            word-spacing: .16em !important;
            color: oklch(0 0 none) !important;
            background-color: oklch(1 0 none) !important;
        }
        [data-rune-text-view="on"] p { margin-block-end: 2em !important; }
    ` });
    await composer.focus();
    await page.keyboard.press('Meta+A');
    const message = 'Text view message ' + 'longword'.repeat(100);
    await page.keyboard.insertText(message);
    await page.keyboard.press('Enter');
    const log = page.getByRole('log', { name: 'Messages', exact: true });
    await page.waitForFunction(message => [...document.querySelectorAll('[role="log"] p')].some(p => p.textContent === message), message);
    assert.equal(await composer.inputValue(), '');
    assert.equal(await composer.evaluate(element => element === document.activeElement), true);
    const paragraph = log.locator('p').last();
    const appearance = await paragraph.evaluate(element => {
        const style = getComputedStyle(element);
        return { family: style.fontFamily, size: style.fontSize, line: style.lineHeight,
            spacing: style.letterSpacing, width: element.scrollWidth, available: element.clientWidth };
    });
    assert.equal(appearance.family, 'monospace');
    const size = await page.evaluate(() => Number.parseFloat(getComputedStyle(document.documentElement).fontSize) * 2);
    assert.equal(Number.parseFloat(appearance.size), size);
    assert.equal(Number.parseFloat(appearance.line), size * 1.5);
    assert.ok(appearance.width <= appearance.available + 1, 'restyled text must wrap without losing content');
    assert.ok(await view.evaluate(element => element.scrollWidth <= element.clientWidth + 1), 'text view must reflow at 320px');
    await paragraph.click();
    await paragraph.evaluate(element => {
        const range = document.createRange();
        range.selectNodeContents(element);
        const selection = getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
    });
    await page.keyboard.press('Meta+C');
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), message);
    await style.evaluate(element => element.remove());
    await toggle.focus();
    await page.keyboard.press('Space');
    await view.waitFor({ state: 'detached' });
    assert.equal(await composer.evaluate((current, previous) => current === previous, editor), true);
    console.log(JSON.stringify({ textView: 'passed', checks: ['retained editor and draft', 'native text customization',
        'user text and spacing', '320px reflow', 'actual send', 'native selection and copy', 'canvas return'] }));
}
