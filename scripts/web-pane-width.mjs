import assert from 'node:assert/strict';

export async function paneWidthProof(page, url, axNode) {
    await page.setViewportSize({ width: 1040, height: 760 });
    await page.goto(new URL('?accessibility-proof&widget=split', url).href);
    await page.getByRole('button', { name: 'After controls (0)', exact: true }).waitFor();
    const grip = page.getByRole('separator', { name: 'Resize sidebar', exact: true });
    const widthButton = page.getByRole('button', { name: 'Sidebar width', exact: true });
    await grip.waitFor();
    const attribute = async (name, value) => page.waitForFunction(
        ({ element, name, value }) => element.getAttribute(name) === value,
        { element: await grip.elementHandle(), name, value });
    const tap = async control => {
        const box = await control.boundingBox();
        assert.ok(box, 'the rendered control needs bounds');
        await page.touchscreen.tap(box.x + box.width / 2, box.y + box.height / 2);
    };
    for (const pane of ['Sidebar', 'Inspector']) {
        const buttonBox = await page.getByRole('button', { name: `${pane} width`, exact: true }).boundingBox();
        const contentBox = await page.getByText(`${pane} content`, { exact: true }).boundingBox();
        assert.ok(buttonBox && contentBox, `${pane} chrome and content need bounds`);
        assert.ok(buttonBox.y + buttonBox.height <= contentBox.y + 0.5, `${pane} chrome must not cover authored content`);
    }
    assert.equal(await grip.getAttribute('tabindex'), '0');
    assert.equal(await grip.getAttribute('aria-orientation'), 'vertical');
    const width = Number(await grip.getAttribute('aria-valuenow'));
    const min = Number(await grip.getAttribute('aria-valuemin'));
    const max = Number(await grip.getAttribute('aria-valuemax'));
    assert.ok(min < width && width < max, 'fixture width must allow resizing in both directions');
    const controlled = await grip.getAttribute('aria-controls');
    assert.ok(controlled, 'the grip must control its pane');
    const pane = page.locator(`[id="${controlled}"]`);
    assert.equal(await pane.count(), 1);
    assert.equal(await pane.getByText('Sidebar content', { exact: true }).count(), 1);
    if (axNode) {
        const node = await axNode(grip);
        const property = name => node.properties.find(item => item.name === name)?.value;
        assert.equal(node.role.value, 'separator');
        assert.equal(node.value.value, width);
        assert.equal(property('valuemin').value, min);
        assert.equal(property('valuemax').value, max);
        assert.equal(property('orientation').value, 'vertical');
        assert.ok(property('controls').relatedNodes.some(item => item.idref === controlled), 'native AX must expose the controlled pane');
    }
    await tap(widthButton);
    const narrow = page.getByRole('menuitem', { name: 'Narrow', exact: true });
    await narrow.waitFor();
    await tap(narrow);
    await attribute('aria-valuenow', String(width - 8));
    await grip.focus();
    await page.keyboard.press('ArrowRight');
    await attribute('aria-valuenow', String(width));
    await page.keyboard.press('Home');
    await attribute('aria-valuenow', String(min));
    await page.keyboard.press('End');
    await attribute('aria-valuenow', String(max));
    if (axNode) assert.equal((await axNode(grip)).value.value, max);
    console.log(JSON.stringify({ paneWidth: 'passed', checks: ['touch width menu', 'chrome clearance', 'keyboard range', 'native grip semantics'] }));
}
