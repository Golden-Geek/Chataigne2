import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const requested = path.resolve(process.argv[2] ?? 'target/mapgyver-gradient');
await mkdir(requested, { recursive: true });
const directory = await mkdtemp(path.join(requested, 'run-'));
const native = await openNativeEditor();
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token;
let stage = 'create the Solid Color source';
let report;

async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}

async function media(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}

async function graph() {
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	return nodes;
}

async function wait(read, accepts) {
	const deadline = performance.now() + 5000;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(performance.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}

const label = 'Gradient output';
const editor = page.getByRole('application', { name: 'Gradient editor' });
async function openNode(id) {
	const header = page.locator(`.node-inspector[data-node-id="${id}"] > .node-header > .node-title`);
	await header.waitFor();
	if ((await header.getAttribute('aria-checked')) === 'false') await header.press('Enter');
}

async function showColor(reference) {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: label, exact: true })
		.click();
	const nodes = await graph();
	const channel = nodes.find((node) => node.uuid === reference.channel);
	await openNode(channel.node_id);
	const parameters = nodes.find(
		(node) => channel.children.includes(node.node_id) && node.decl_id === 'parameters'
	);
	return nodes.find(
		(node) => parameters.children.includes(node.node_id) && node.decl_id === 'parameters/rgba'
	).node_id;
}

const modeTrigger = (id) =>
	page.locator(`.parameter-inspector[data-node-id="${id}"] .control-mode-trigger`);
async function mode(id, name) {
	await modeTrigger(id).click();
	await page.getByRole('menu').getByText(name, { exact: true }).click();
	await wait(
		() => modeTrigger(id).getAttribute('class'),
		(classes) => classes.split(' ').includes(name.toLowerCase())
	);
}

async function paintStop(position, hex) {
	await editor.getByRole('button', { name: `Gradient stop at ${position} percent` }).dblclick();
	const popup = page.getByRole('dialog', { name: 'Stop color' });
	await popup.locator('.cp-input-hex').fill(hex);
	await popup.locator('.gradient-color-popup-close').click();
}

async function setNumber(node, value) {
	const input = page.locator(`.parameter-inspector[data-node-id="${node}"] input.number-field`);
	await input.fill(String(value));
	await input.press('Enter');
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
	await media({
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'SolidColor', rgba: [0, 1, 0, 1] }
	});
	const output = await createPreviewOutputObserver(post, directory, page);
	await output.demand(reference);
	let parameter = await showColor(reference);
	const read = async () =>
		(await graph()).find((node) => node.node_id === parameter)?.data.param.value.Color;
	const pixels = async (name, rgba) => {
		await wait(read, (value) => value?.every((channel, index) => channel === rgba[index] / 255));
		await delay(150);
		await output.observePixels(name, rgba);
	};
	const green = [0, 255, 0, 255],
		red = [255, 0, 0, 255],
		blue = [0, 0, 255, 128];
	await pixels('manual-green', green);

	stage = 'attach Gradient through the Color Inspector';
	await mode(parameter, 'Animation');
	const animated = await wait(read, (value) => value?.[0] > 0.15 && value[0] < 0.7);
	await wait(read, (value) => value && Math.abs(animated[0] - value[0]) > 0.1);
	const nodes = await graph();
	const color = nodes.find((node) => node.node_id === parameter);
	const animation = nodes.find(
		(node) =>
			color.children.includes(node.node_id) && node.node_type === 'parameter_animation_control'
	);
	await openNode(animation.node_id);
	const gradient = nodes.find(
		(node) => animation.children.includes(node.node_id) && node.node_type === 'gradient'
	);
	const animationField = (decl) =>
		nodes.find((node) => animation.children.includes(node.node_id) && node.decl_id === decl)
			.node_id;
	await openNode(gradient.node_id);
	await setNumber(animationField('frequency_hz'), 0);
	await editor.waitFor();

	stage = 'edit typed stops and verify independent Spout pixels';
	await paintStop(0, 'ff0000');
	await pixels('animated-red', red);
	await paintStop(100, '0000ff80');
	parameter = await showColor(reference);
	await openNode(animation.node_id);
	await setNumber(animationField('phase'), 0.5);
	await wait(
		read,
		(value) => value?.[0] === 0.5 && value[2] === 0.5 && value[3] > 0.75 && value[3] < 0.752
	);
	await delay(150);
	await output.observePixels('linear-rgba-midpoint', [188, 0, 188, 192], 1);
	await setNumber(animationField('phase'), 0.9999);
	const blueEndpoint = (value) =>
		value?.[2] > 0.999 && value[0] < 0.001 && value[3] > 0.501 && value[3] < 0.504;
	await wait(read, blueEndpoint);
	await delay(150);
	await output.observePixels('animated-blue', blue);

	stage = 'reload saved animation and restore manual color';
	const project = path.join(directory, 'Gradient.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	parameter = await showColor(reference);
	await wait(read, blueEndpoint);
	await delay(150);
	await output.observePixels('reloaded-blue', blue);
	await mode(parameter, 'Manual');
	await pixels('restored-manual-green', green);
	assert.deepEqual(errors, []);
	report = { result: 'pass', adapter: output.adapter, pixels: output.observations, errors };
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), log: native.log(), errors }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
await writeFile(path.join(directory, 'result.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify({ directory, ...report }));
