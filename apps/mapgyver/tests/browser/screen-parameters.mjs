import assert from 'node:assert/strict';

/** Real Golden widgets and node picker shared by the Screen/Source acceptance flows. */
export function screenParameterControls(page) {
	const parameters = page.locator('.parameter-inspector');
	const parameter = (label) => parameters.filter({ has: page.getByText(label, { exact: true }) });
	const number = (label, component = 0) =>
		parameter(label).locator('input.number-field').nth(component);
	const choice = (label) => parameter(label).locator('select.dropdown-editor');
	async function setNumber(label, value, component = 0) {
		await number(label, component).fill(String(value));
		await number(label, component).press('Enter');
	}
	async function setText(label, value) {
		const input = parameter(label).locator('input.string-editor');
		await input.fill(value);
		await input.press('Tab');
	}
	async function rename(label, value) {
		const title = page
			.locator('.inspector-header .title-text')
			.filter({ hasText: new RegExp(`^${RegExp.escape(label)}$`) });
		await title.dblclick();
		const input = page.locator('.inspector-header .title-input');
		await input.fill(value);
		await input.press('Enter');
	}
	async function reference(label, search, target) {
		await parameter(label).locator('button.picker-trigger').click();
		const picker = page.getByRole('dialog', { name: 'Select reference target', exact: true });
		await picker.locator('input.picker-search').fill(search);
		await picker
			.getByRole('treeitem', { name: target, exact: true })
			.getByRole('button', { name: target, exact: true })
			.click();
		await picker.getByRole('button', { name: 'Select', exact: true }).click();
		await picker.waitFor({ state: 'detached' });
	}
	async function fields(labels, wait) {
		await wait(
			() => parameters.count(),
			(count) => count === labels.length
		);
		for (const label of labels) assert.equal(await parameter(label).count(), 1, label);
		const ids = await parameters.evaluateAll((nodes) => nodes.map((node) => node.dataset.nodeId));
		assert.equal(
			new Set(ids).size,
			labels.length,
			'Each ordinary widget owns a distinct real Parameter'
		);
	}
	return { parameters, parameter, number, choice, setNumber, setText, rename, reference, fields };
}

export const surfaceParameterLabels = [
	'Source media',
	'Source channel',
	'Crop origin',
	'Crop size',
	'Top left',
	'Top right',
	'Bottom right',
	'Bottom left',
	'Opacity',
	'Calibration',
	'Enabled'
];
