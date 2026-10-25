import assert from 'node:assert/strict';

export async function qualifyLogs(page, emit) {
	const publication = await page.evaluate(() => window.collectionFixture.logStorePublication());
	assert(Object.values(publication).every((passed) => passed === true));
	emit({ kind: 'log-store-publication', ...publication });
	const list = page.locator('.logger-list');
	const record = (id) => list.locator(`[data-record-key="r:${id}"]`);
	const counts = () =>
		list.locator('.virtual-rows').evaluate((node) => ({
			rows: Number(node.dataset.rowCount),
			mounted: Number(node.dataset.mountedRowCount)
		}));
	const visible = (id) =>
		record(id).evaluate((row) => {
			const rect = row.getBoundingClientRect(),
				bounds = row.closest('.logger-list').getBoundingClientRect();
			return rect.width > 0 && rect.bottom > bounds.top && rect.top < bounds.bottom;
		});
	const settleSelection = () =>
		page.evaluate(async () => {
			await new Promise(requestAnimationFrame);
			await new Promise(requestAnimationFrame);
		});
	for (const count of [1000, 10000, 20000, 50000, 100000]) {
		const setup = await page.evaluate(
			(count) => window.collectionFixture.setupLogger(count),
			count
		);
		assert.equal((await counts()).rows, count);
		assert((await counts()).mounted > 0 && (await counts()).mounted <= 42);
		assert.equal(await visible(count), true);
		assert.equal(await page.evaluate(() => window.collectionFixture.logBrowse(1)), true);
		assert.equal(await visible(1), true);
		assert.equal(
			await record(1).evaluate((row) => {
				const content = row.getBoundingClientRect(),
					wrapper = row.closest('.virtual-row').getBoundingClientRect();
				return (
					content.height > 2 * parseFloat(getComputedStyle(document.documentElement).fontSize) &&
					wrapper.height >= content.height
				);
			}),
			true
		);
		if (count === 1000) {
			const text = await page.evaluate(() => window.collectionFixture.logCopyText());
			assert(text.includes('Log 000001') && text.includes('Log 001000'));
		}
		const middle = Math.floor(count / 2);
		await page.evaluate((id) => window.collectionFixture.logFocus(id), middle);
		assert.equal(
			await record(middle).evaluate((row) => row.classList.contains('is-focused')),
			true
		);
		const focused = await record(middle).elementHandle();
		await focused.evaluate((node) => node.focus());
		await page.evaluate((id) => window.collectionFixture.logBrowse(id), count);
		assert.equal(await focused.evaluate((node) => node === document.activeElement), true);
		const appended = await page.evaluate(() => window.collectionFixture.logAppend());
		assert.equal(
			await focused.evaluate((node) => node.isConnected && node === document.activeElement),
			true
		);
		assert.equal((await counts()).rows, count);
		await focused.dispose();
		const selectedId = Math.floor(count / 26) * 13 + 1;
		await page.evaluate((id) => window.collectionFixture.logBrowse(id), selectedId);
		const selected = await record(selectedId).locator('.message').elementHandle();
		const text = await selected.evaluate((node) => {
			const range = document.createRange();
			range.selectNodeContents(node);
			const selection = getSelection();
			selection.removeAllRanges();
			selection.addRange(range);
			return selection.toString();
		});
		await settleSelection();
		await page.evaluate((id) => window.collectionFixture.logBrowse(id), count);
		await page.evaluate(() => window.collectionFixture.logAppend());
		assert.equal(await selected.evaluate((node) => node.isConnected), true);
		assert.equal(await page.evaluate(() => getSelection().toString()), text);
		assert((await counts()).mounted <= 112);
		await selected.dispose();
		await page.evaluate(() => getSelection().removeAllRanges());
		await settleSelection();
		await page.evaluate(() => window.collectionFixture.logBrowse(10));
		const rangeText = await list.evaluate((node) => {
			const first = node.querySelector('[data-record-key="r:10"] .message').firstChild;
			const last = node.querySelector('[data-record-key="r:13"] .message').firstChild;
			const range = document.createRange();
			range.setStart(first, 0);
			range.setEnd(last, last.textContent.length);
			getSelection().removeAllRanges();
			getSelection().addRange(range);
			return getSelection().toString();
		});
		await settleSelection();
		await page.evaluate((id) => window.collectionFixture.logBrowse(id), count);
		assert.equal(await page.evaluate(() => getSelection().toString()), rangeText);
		assert((await counts()).mounted <= 112);
		await page.evaluate(() => getSelection().removeAllRanges());
		await settleSelection();
		await page.evaluate(() => window.collectionFixture.logSearch('Log 000005', 'engine', 'stream'));
		assert.equal((await counts()).rows, 1);
		await record(5).evaluate((node) => node.focus());
		await page.evaluate(() => window.collectionFixture.logSearch('no-match'));
		assert.equal((await counts()).rows, 0);
		assert.equal(await list.locator('.record').count(), 0);
		assert.equal(await list.evaluate((node) => node === document.activeElement), true);
		emit({
			kind: 'log-tier',
			size: count,
			setup,
			selection: 'single multiline and four adjacent records retained',
			appended: appended.message,
			passed: true
		});
		await page.evaluate((count) => window.collectionFixture.setupLogger(count), count);
		const measured = await page.evaluate(() => window.collectionFixture.measureLogs());
		emit({ kind: 'measurement', ...measured });
		assert(measured.passed, `Log feedback gate failed: ${JSON.stringify(measured.summary)}`);
		const appendedBatch = await page.evaluate(() => window.collectionFixture.measureLogAppend());
		emit({ kind: 'measurement', ...appendedBatch });
		assert(
			appendedBatch.passed,
			`Log append gate failed: ${JSON.stringify(appendedBatch.summary)}`
		);
	}
	await page.evaluate(() => window.collectionFixture.setupLogger(1000, false, true));
	assert.equal((await counts()).rows, 1000);
	assert.equal(
		await list
			.locator('.repeat-count')
			.first()
			.textContent()
			.then((text) => text.trim()),
		'x2'
	);
	await page.evaluate(() => window.collectionFixture.logCollapse());
	assert.equal((await counts()).rows, 1000);
	await list
		.locator('.record')
		.first()
		.evaluate((node) => node.focus());
	await page.evaluate(() => window.collectionFixture.logCollapse());
	assert.equal(await list.evaluate((node) => node === document.activeElement), true);
	await page.evaluate(() => window.collectionFixture.logSuperCollapse());
	assert.equal((await counts()).rows, 10);
	assert.equal(
		await list
			.locator('.repeat-count')
			.first()
			.textContent()
			.then((text) => text.trim()),
		'x200'
	);
	await page.evaluate(() => window.collectionFixture.setupLogger(1000, true));
	assert.equal((await counts()).rows, 1000);
	await page.evaluate(() => window.collectionFixture.setupLogger(2));
	await page.evaluate(() => window.collectionFixture.logFocus(1));
	await record(1).evaluate((node) => node.focus());
	const latest = await page.evaluate(() => window.collectionFixture.logAppend());
	assert.equal(await record(1).count(), 0);
	assert.equal(await list.evaluate((node) => node === document.activeElement), true);
	assert.equal(await visible(latest.id), true);
	await page.evaluate(() => window.collectionFixture.setupLogger(1000));
	await page.evaluate(() => window.collectionFixture.logBrowse(50));
	await record(50).evaluate((node) => node.focus());
	await page.evaluate(() => window.collectionFixture.logClear());
	assert.equal((await counts()).rows, 0);
	assert.equal(await list.evaluate((node) => node === document.activeElement), true);
	assert.equal(await page.evaluate(() => window.collectionFixture.logCopyText()), '');
	emit({ kind: 'log-result', passed: true });
}
