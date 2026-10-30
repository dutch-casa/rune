import assert from 'node:assert/strict';

export async function accessibilityProof(page, url, axNode) {
    await textViewProof(page, url);
    const open = async widget => {
        await page.goto(new URL(`?accessibility-proof&widget=${widget}`, url).href);
        await page.getByRole('button', { name: 'After controls (0)', exact: true }).waitFor();
    };
    const attribute = async (locator, name, value) => page.waitForFunction(
        ({ element, name, value }) => element.getAttribute(name) === value,
        { element: await locator.elementHandle(), name, value });
    const active = async owner => {
        const id = await owner.getAttribute('aria-activedescendant');
        assert.ok(id, 'the keyboard cursor must have a mounted accessibility target');
        return page.locator(`[id="${id}"]`);
    };

    await open('controls');
    await page.getByRole('button', { name: 'After controls (0)', exact: true }).focus();
    await page.keyboard.press('Space');
    const changedAction = page.getByRole('button', { name: 'After controls (1)', exact: true });
    await changedAction.waitFor();
    if (axNode) assert.equal((await axNode(changedAction)).name.value, 'After controls (1)');
    const disclosure = page.getByRole('button', { name: 'Details', exact: true });
    await disclosure.focus();
    await page.keyboard.press('Space');
    await attribute(disclosure, 'aria-expanded', 'true');
    assert.equal(await page.getByText('Disclosure content', { exact: true }).count(), 1);
    const field = page.getByRole('textbox', { name: 'Account name', exact: true });
    await attribute(field, 'aria-invalid', 'true');
    await field.focus();
    await page.keyboard.press('Meta+A');
    await page.keyboard.insertText('ready');
    await attribute(field, 'aria-invalid', null);
    const progress = page.getByRole('progressbar', { name: 'Upload', exact: true });
    if (axNode) {
        const node = await axNode(progress);
        assert.equal(node.value.value, 40);
        assert.equal(node.properties.find(p => p.name === 'valuemax').value.value, 100);
    }
    await page.getByRole('button', { name: 'Text view', exact: true }).click();
    await page.locator('[data-rune-text-view="on"]').waitFor();
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
    await attribute(disclosure, 'aria-expanded', 'false');
    assert.equal(await page.evaluate(() => getSelection().toString()), '25', 'an unrelated update must preserve selected progress text');
    await page.getByRole('button', { name: 'Text view', exact: true }).click();
    await page.locator('[data-rune-text-view="off"]').waitFor();
    const toast = page.getByRole('status', { name: 'Saved. Draft stored', exact: true });
    await toast.getByRole('button', { name: 'Close', exact: true }).focus();
    await page.keyboard.press('Space');
    await toast.waitFor({ state: 'detached' });

    await open('combobox');
    const trigger = page.getByRole('combobox', { name: 'Model', exact: true });
    await trigger.focus();
    await page.keyboard.press('Enter');
    const search = page.getByRole('combobox', { name: 'Search', exact: true });
    await search.waitFor();
    await attribute(trigger, 'aria-expanded', 'true');
    await page.keyboard.press('ArrowDown');
    await page.waitForFunction(() => {
        const field = document.activeElement;
        return document.getElementById(field?.getAttribute('aria-activedescendant'))?.textContent.trim() === 'Beta';
    });
    const controlled = await search.getAttribute('aria-controls');
    assert.ok(controlled, 'the search field must control its option list');
    assert.equal(await trigger.getAttribute('aria-controls'), controlled);
    assert.equal(await page.locator(`[id="${controlled}"]`).getAttribute('role'), 'listbox');
    assert.equal(await (await active(search)).textContent(), 'Beta');
    await page.keyboard.press('Enter');
    await trigger.waitFor();
    assert.equal(await trigger.textContent(), 'Beta');
    await attribute(trigger, 'aria-expanded', 'false');

    await open('table');
    const grid = page.getByRole('grid', { name: 'Records', exact: true });
    assert.equal(await grid.getByRole('gridcell', { name: 'Alpha', exact: true }).count(), 1);
    await grid.focus();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.waitForFunction(() => {
        const grid = document.querySelector('[role="grid"]');
        return document.getElementById(grid?.getAttribute('aria-activedescendant'))?.textContent.trim() === 'Beta';
    });
    assert.equal(await (await active(grid)).getAttribute('aria-selected'), 'true');
    const header = page.getByRole('columnheader', { name: 'Name', exact: true });
    await header.focus();
    await page.keyboard.press('Space');
    await page.getByRole('columnheader', { name: 'Name, sorted ascending', exact: true }).waitFor();

    await open('tree');
    const tree = page.getByRole('tree', { name: 'Files', exact: true });
    await tree.focus();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    await page.waitForFunction(() => {
        const tree = document.querySelector('[role="tree"]');
        return document.getElementById(tree?.getAttribute('aria-activedescendant'))?.getAttribute('aria-level') === '2';
    });
    const cursor = await active(tree);
    assert.equal(await cursor.getAttribute('aria-level'), '2');
    assert.equal(await cursor.getAttribute('aria-selected'), 'true');
    await open('sidebar');
    await page.getByRole('button', { name: 'Text view', exact: true }).focus();
    await page.keyboard.press('Tab');
    const headerToggle = page.getByRole('button', { name: 'Today', exact: true });
    assert.equal(await headerToggle.evaluate(element => element === document.activeElement), true);
    await attribute(headerToggle, 'aria-expanded', 'true');
    await page.keyboard.press('ArrowLeft');
    await attribute(headerToggle, 'aria-expanded', 'false');
    assert.equal(await page.getByRole('listitem', { name: 'Alpha', exact: true }).count(), 0);
    await page.keyboard.press('Space');
    await attribute(headerToggle, 'aria-expanded', 'true');
    await page.getByRole('listitem', { name: 'Alpha', exact: true }).waitFor();


    await page.setViewportSize({ width: 1040, height: 760 });
    await open('split');
    const grip = page.getByRole('separator', { name: 'Resize sidebar', exact: true });
    const widthButton = page.getByRole('button', { name: 'Sidebar width', exact: true });
    await grip.waitFor();
    assert.equal(await grip.getAttribute('aria-orientation'), 'vertical');
    const width = Number(await grip.getAttribute('aria-valuenow'));
    const tap = async control => {
        const box = await control.boundingBox();
        assert.ok(box, 'the canvas control needs presented bounds');
        await page.touchscreen.tap(box.x + box.width / 2, box.y + box.height / 2);
    };
    const buttonBox = await widthButton.boundingBox();
    const contentBox = await page.getByText('Sidebar content', { exact: true }).boundingBox();
    assert.ok(buttonBox.y + buttonBox.height <= contentBox.y + 0.5, 'shell chrome must not cover pane content');
    await tap(widthButton);
    const narrow = page.getByRole('menuitem', { name: 'Narrow', exact: true });
    await narrow.waitFor();
    await tap(narrow);
    await attribute(grip, 'aria-valuenow', String(width - 8));
    await grip.focus();
    await page.keyboard.press('ArrowRight');
    await attribute(grip, 'aria-valuenow', String(width));
    await page.keyboard.press('Home');
    await attribute(grip, 'aria-valuenow', await grip.getAttribute('aria-valuemin'));
    await page.keyboard.press('End');
    await attribute(grip, 'aria-valuenow', await grip.getAttribute('aria-valuemax'));
    console.log(JSON.stringify({ kitAccessibility: 'passed', checks: ['disclosure keyboard state', 'invalid field recovery',
        'native progress value', 'notification dismissal', 'search cursor ownership', 'grid cell structure and cursor', 'sort keyboard', 'tree depth', 'pointer-only pane resize', 'pane keyboard bounds'] }));
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
