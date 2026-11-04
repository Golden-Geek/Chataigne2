import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { selectMediaChannel } from './media-tree.mjs';

/** Parameter widgets author state; HTTP operations only read the committed projection. */
export async function verifyMediaParameters({
	page,
	inspectMedia,
	composition,
	rejected,
	wait,
	stage
}) {
	const widgets = page.locator('.parameter-inspector');
	const parameter = (label) => widgets.filter({ has: page.getByText(label, { exact: true }) });
	const choice = (label) => parameter(label).locator('select.dropdown-editor');
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	const channel = async (reference) => {
		const catalog = await inspectMedia();
		return catalog.media
			.find((media) => media.id === reference.media)
			?.channels.find((item) => item.reference.channel === reference.channel);
	};
	async function fields(labels) {
		await wait(
			() => widgets.count(),
			(count) => count === labels.length
		);
		for (const label of labels) assert.equal(await parameter(label).count(), 1, label);
		const ids = await widgets.evaluateAll((nodes) => nodes.map((node) => node.dataset.nodeId));
		assert.equal(new Set(ids).size, labels.length, 'Each widget binds a distinct Parameter node');
	}
	async function select(reference) {
		await selectMediaChannel(page, await inspectMedia(), reference);
		for (const label of ['Kind', 'Source kind', 'Sampling'])
			assert.equal(await parameter(label).count(), 0);
	}
	async function setText(input, value) {
		await input.fill(value);
		await input.press('Tab');
	}
	const catalog = await inspectMedia();
	const solid = catalog.media.find((media) => media.media_kind === 'SolidColor');
	const shared = catalog.media.find((media) => media.media_kind === 'SharedTexture');
	const picture = catalog.media.find((media) => media.media_kind === 'Picture');
	assert(solid && shared && picture);
	stage('generic compact Color editing and independent channel history');
	const original = solid.channels[0];
	const peer = solid.channels[1];
	await select(original.reference);
	await fields(['Color']);
	const color = parameter('Color');
	await color.locator('.cp-preview').click();
	await color.locator('.cp-input-hex').fill('ff0000');
	await color.locator('.cp-input-hex').press('Tab');
	const changed = await wait(
		() => channel(original.reference),
		(value) => JSON.stringify(value.definition.source.rgba.slice(0, 3)) === '[1,0,0]'
	);
	await undo();
	await wait(
		() => channel(original.reference),
		(value) => isDeepStrictEqual(value, original)
	);
	await redo();
	await wait(
		() => channel(original.reference),
		(value) => isDeepStrictEqual(value, changed)
	);
	assert.deepEqual(await channel(peer.reference), peer);

	stage('generic sender-only Shared Texture Parameters');
	const input = shared.channels[0],
		inputPeer = shared.channels[1];
	await select(input.reference);
	await fields(['Spout sender']);
	await setText(parameter('Spout sender').locator('input.string-editor'), 'Parameter Input');
	const spout = await wait(
		() => channel(input.reference),
		(value) => value.definition.source.sender === 'Parameter Input'
	);
	await undo();
	await wait(
		() => channel(input.reference),
		(value) => isDeepStrictEqual(value, input)
	);
	await redo();
	await wait(
		() => channel(input.reference),
		(value) => isDeepStrictEqual(value, spout)
	);
	assert.deepEqual(await channel(inputPeer.reference), inputPeer);
	await page.screenshot({ path: 'target/mapgyver-channel-parameters-ui.png' });

	stage('generic Picture reference and asset File Parameters');
	const linked = picture.channels[0];
	await select(linked.reference);
	await fields(['File']);
	assert.equal(await parameter('File').locator('input.path-input').count(), 1);
	await page.getByRole('button', { name: 'Inspect Picture asset', exact: true }).click();
	await fields(['File']);
	const asset = catalog.assets.find((item) => item.id === linked.definition.source.asset);
	assert(asset);
	const readAsset = async () => (await inspectMedia()).assets.find((item) => item.id === asset.id);
	const pathId = await parameter('File').getAttribute('data-node-id');
	await setText(parameter('File').locator('input.path-input'), 'media/parameter-ui.png');
	const replacement = await wait(
		readAsset,
		(value) => value.locator.path === 'media/parameter-ui.png'
	);
	await undo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, asset));
	await redo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, replacement));
	await setText(parameter('File').locator('input.path-input'), '');
	await wait(readAsset, (value) => value.locator === null);
	await fields(['File']);
	await undo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, replacement));
	await fields(['File']);
	assert.equal(await parameter('File').getAttribute('data-node-id'), pathId);
	const rejectionCount = rejected.length;
	stage('generic asset path rejects traversal without document or history mutation');
	await setText(parameter('File').locator('input.path-input'), '../outside.png');
	await wait(
		() => rejected.length,
		(count) => count === rejectionCount + 1
	);
	assert.deepEqual(await readAsset(), replacement);
	await parameter('File').locator('input.path-input').press('Escape');
	await undo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, asset));
	await redo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, replacement));
	const currentPicture = (await inspectMedia()).media.find((media) => media.id === picture.id);
	assert(
		currentPicture.channels.every((item) => item.definition.source.asset === asset.id),
		'Shared Picture references retain the same asset identity'
	);
	await page.screenshot({ path: 'target/mapgyver-asset-parameters-ui.png' });

	async function canvas(reference, width, height, readCanvas) {
		const media = (await inspectMedia()).media.find((item) => item.id === reference.media);
		await page.getByRole('button', { name: media.label, exact: true }).click();
		const resolution = parameter('Resolution');
		const toggle = resolution.getByRole('switch', { name: 'Toggle node enabled' });
		if ((await toggle.getAttribute('aria-checked')) === 'false') await toggle.click();
		const inputs = resolution.locator('input.number-field');
		await inputs.nth(0).fill(String(width));
		await inputs.nth(0).press('Enter');
		await wait(readCanvas, (value) => value.width === width);
		const before = await readCanvas();
		await inputs.nth(1).fill(String(height));
		await inputs.nth(1).press('Enter');
		const after = await wait(readCanvas, (value) => value.height === height);
		await undo();
		await wait(readCanvas, (value) => isDeepStrictEqual(value, before));
		await redo();
		await wait(readCanvas, (value) => isDeepStrictEqual(value, after));
		for (const label of ['Kind', 'Source kind', 'Sampling', 'Canvas width', 'Canvas height'])
			assert.equal(await parameter(label).count(), 0);
	}
	stage('generic media-owned Composition Resolution');
	await canvas(
		composition,
		1440,
		900,
		async () => (await channel(composition)).definition.source.canvas
	);

	stage('generic media-owned Matrix Resolution; transitions remain Layer-owned clocks');
	const beforeMatrix = await inspectMedia();
	await page.getByRole('button', { name: 'Add child item', exact: true }).first().click();
	await page.getByRole('menuitem', { name: 'Matrix', exact: true }).click();
	const withMatrix = await wait(
		inspectMedia,
		(value) => value.media.length === beforeMatrix.media.length + 1
	);
	const matrix = withMatrix.media.find(
		(item) => !beforeMatrix.media.some((old) => old.id === item.id)
	);
	assert.equal(matrix.media_kind, 'Matrix');
	const output = matrix.channels[0].reference;
	await canvas(
		output,
		1280,
		720,
		async () => (await channel(output)).definition.source.output.definition.canvas
	);
	assert.equal(await parameter('Transition duration').count(), 0);
	await page.screenshot({ path: 'target/mapgyver-matrix-channel-parameters-ui.png' });
	assert.equal(rejected.length, rejectionCount + 1);
}
