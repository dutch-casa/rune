import assert from 'node:assert/strict';

export async function accessibilityProof(page, url, axNode) {
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
    const circle = page.getByRole('progressbar', { name: 'Progress', exact: true });
    await circle.waitFor();
    if (axNode) {
        const node = await axNode(circle);
        assert.equal(node.name.value, 'Progress');
        assert.equal(node.value.value, 25);
    }
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

    console.log(JSON.stringify({ kitAccessibility: 'passed', checks: ['disclosure keyboard state', 'invalid field recovery',
        'native progress value', 'notification dismissal', 'search cursor ownership', 'grid cell structure and cursor', 'sort keyboard', 'tree depth', 'sidebar header focus'] }));
}

