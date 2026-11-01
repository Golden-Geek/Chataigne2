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
	const number = (label) => parameter(label).locator('input.number-field');
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
	async function select(reference, kind) {
		await selectMediaChannel(page, await inspectMedia(), reference);
		await wait(
			() => choice('Source kind').inputValue(),
			(value) => value === kind
		);
		assert.match(await choice('Source kind').getAttribute('class'), /readonly/);
	}
	async function setNumber(label, value) {
		await number(label).fill(String(value));
		await number(label).press('Enter');
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
	const automatic = solid.channels.find((item) => item.definition.size.kind === 'Automatic');
	const solidPeer = solid.channels.find((item) => item !== automatic);
	assert(automatic && solidPeer);

	stage('generic channel size, sampling, visibility and history');
	await select(automatic.reference, 'SolidColor');
	await fields(['Source kind', 'Size', 'Sampling', 'Color']);
	await choice('Size').selectOption('Fixed');
	await wait(
		() => channel(automatic.reference),
		(value) => value.definition.size.kind === 'Fixed'
	);
	await fields(['Source kind', 'Size', 'Output width', 'Output height', 'Sampling', 'Color']);
	const widthId = await parameter('Output width').getAttribute('data-node-id');
	await setNumber('Output width', 640);
	await wait(
		() => channel(automatic.reference),
		(value) => value.definition.size.extent.width === 640
	);
	await setNumber('Output height', 360);
	await wait(
		() => channel(automatic.reference),
		(value) => value.definition.size.extent.height === 360
	);
	await choice('Sampling').selectOption('Linear');
	const fixed = await wait(
		() => channel(automatic.reference),
		(value) => value.definition.filter === 'Linear'
	);
	await undo();
	await wait(
		() => channel(automatic.reference),
		(value) => value.definition.filter === automatic.definition.filter
	);
	await redo();
	await wait(
		() => channel(automatic.reference),
		(value) => isDeepStrictEqual(value, fixed)
	);
	await choice('Size').selectOption('Automatic');
	await wait(
		() => channel(automatic.reference),
		(value) => value.definition.size.kind === 'Automatic'
	);
	await fields(['Source kind', 'Size', 'Sampling', 'Color']);
	await undo();
	await wait(
		() => channel(automatic.reference),
		(value) => isDeepStrictEqual(value, fixed)
	);
	await fields(['Source kind', 'Size', 'Output width', 'Output height', 'Sampling', 'Color']);
	assert.equal(
		await parameter('Output width').getAttribute('data-node-id'),
		widthId,
		'Hiding a field retains its actual Parameter identity'
	);
	assert.deepEqual(await channel(solidPeer.reference), solidPeer, 'A peer channel is unchanged');
	const manager = page.locator('section[aria-label="Media Manager"]');
	await manager
		.getByRole('treeitem', { name: solid.label, exact: true })
		.getByRole('button', { name: solid.label, exact: true })
		.click();
	await wait(
		() => choice('Kind').inputValue(),
		(value) => value === 'SolidColor'
	);
	await fields(['Kind']);
	assert.match(await choice('Kind').getAttribute('class'), /readonly/);

	stage('generic Spout descriptor and sender Parameters');
	const input = shared.channels.find((item) => item.definition.size.kind === 'Automatic');
	const inputPeer = shared.channels.find((item) => item !== input);
	assert(input && inputPeer);
	await select(input.reference, 'SharedTexture');
	await fields([
		'Source kind',
		'Size',
		'Sampling',
		'Spout sender',
		'Maximum width',
		'Maximum height',
		'Pixel format',
		'Color encoding',
		'Alpha mode'
	]);
	await setText(parameter('Spout sender').locator('input.string-editor'), 'Parameter Input');
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.sender === 'Parameter Input'
	);
	await setNumber('Maximum width', 1024);
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.extent.width === 1024
	);
	await setNumber('Maximum height', 768);
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.extent.height === 768
	);
	await choice('Pixel format').selectOption('Rgba8Unorm');
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.format === 'Rgba8Unorm'
	);
	await choice('Color encoding').selectOption('Linear');
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.color_encoding === 'Linear'
	);
	await choice('Alpha mode').selectOption('Opaque');
	const spout = await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.alpha_mode === 'Opaque'
	);
	await undo();
	await wait(
		() => channel(input.reference),
		(value) => value.definition.source.maximum.alpha_mode === 'Straight'
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
	await select(linked.reference, 'Picture');
	const pictureFields = ['Source kind', 'Size', 'Sampling', 'Picture asset'];
	if (linked.definition.size.kind === 'Fixed') pictureFields.push('Output width', 'Output height');
	await fields(pictureFields);
	assert.equal(await parameter('Picture asset').locator('.reference-editor').count(), 1);
	await page.getByRole('button', { name: 'Inspect Picture asset', exact: true }).click();
	await wait(
		() => choice('Location').inputValue(),
		(value) => value === 'ProjectRelative'
	);
	await fields(['Location', 'Project path']);
	const asset = catalog.assets.find((item) => item.id === linked.definition.source.asset);
	assert(asset);
	const readAsset = async () => (await inspectMedia()).assets.find((item) => item.id === asset.id);
	const pathId = await parameter('Project path').getAttribute('data-node-id');
	await setText(parameter('Project path').locator('input.path-input'), 'media/parameter-ui.png');
	const replacement = await wait(
		readAsset,
		(value) => value.locator.path === 'media/parameter-ui.png'
	);
	await undo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, asset));
	await redo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, replacement));
	await choice('Location').selectOption('Unconfigured');
	await wait(readAsset, (value) => value.locator === null);
	await fields(['Location']);
	await undo();
	await wait(readAsset, (value) => isDeepStrictEqual(value, replacement));
	await fields(['Location', 'Project path']);
	assert.equal(await parameter('Project path').getAttribute('data-node-id'), pathId);
	const rejectionCount = rejected.length;
	stage('generic asset path rejects traversal without document or history mutation');
	await setText(parameter('Project path').locator('input.path-input'), '../outside.png');
	await wait(
		() => rejected.length,
		(count) => count === rejectionCount + 1
	);
	assert.deepEqual(await readAsset(), replacement);
	await parameter('Project path').locator('input.path-input').press('Escape');
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

	stage('generic Composition canvas Parameters');
	await select(composition, 'Composition');
	await fields(['Source kind', 'Size', 'Sampling', 'Canvas width', 'Canvas height']);
	await setNumber('Canvas width', 1440);
	await wait(
		() => channel(composition),
		(value) => value.definition.source.canvas.width === 1440
	);
	await setNumber('Canvas height', 900);
	const canvas = await wait(
		() => channel(composition),
		(value) => value.definition.source.canvas.height === 900
	);
	await undo();
	await wait(
		() => channel(composition),
		(value) => value.definition.source.canvas.height === 1080
	);
	await redo();
	await wait(
		() => channel(composition),
		(value) => isDeepStrictEqual(value, canvas)
	);

	stage('generic Matrix final canvas and transition Parameters');
	const beforeMatrix = await inspectMedia();
	await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
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
	await select(output, 'Matrix');
	await fields([
		'Source kind',
		'Size',
		'Sampling',
		'Canvas width',
		'Canvas height',
		'Matrix output',
		'Empty cell',
		'Transition duration',
		'Transition curve'
	]);
	await setNumber('Canvas width', 1280);
	await wait(
		() => channel(output),
		(value) => value.definition.source.output.definition.canvas.width === 1280
	);
	await setNumber('Canvas height', 720);
	await wait(
		() => channel(output),
		(value) => value.definition.source.output.definition.canvas.height === 720
	);
	await setNumber('Transition duration', 1234);
	await wait(
		() => channel(output),
		(value) => value.definition.source.output.definition.transition.duration_ms === 1234
	);
	await choice('Transition curve').selectOption('SmoothStep');
	const transition = await wait(
		() => channel(output),
		(value) => value.definition.source.output.definition.transition.curve === 'SmoothStep'
	);
	await undo();
	await wait(
		() => channel(output),
		(value) => value.definition.source.output.definition.transition.curve === 'Linear'
	);
	await redo();
	await wait(
		() => channel(output),
		(value) => isDeepStrictEqual(value, transition)
	);
	await page.screenshot({ path: 'target/mapgyver-matrix-channel-parameters-ui.png' });
	assert.equal(rejected.length, rejectionCount + 1);
}
