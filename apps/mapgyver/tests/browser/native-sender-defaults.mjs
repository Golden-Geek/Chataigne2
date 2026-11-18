// Actual Screen outputs, separate SDK senders, generic Inspector and independent SDK pixels.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { inventory, source } from './native-sender-selection.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const directory = path.resolve(process.argv[2] ?? 'target/mapgyver-native-sender-defaults');
await mkdir(directory, { recursive: true });
const hardware = await inventory();
const native = await openNativeEditor();
const { page } = native;
const errors = [],
	sources = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
const extent = { width: 96, height: 64 };
const suffix = randomUUID();
const none = { value: '', label: 'None' };
const red = [255, 0, 0, 255];
const green = [0, 255, 0, 255];
const blue = [0, 0, 255, 255];
const magenta = [255, 0, 255, 255];
const internal = (name) => ({ value: name, label: `${name} (internal)` });
let token, adapter, output;
let stage = 'startup';

async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function edit(namespace, payload) {
	const value = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(value.success, JSON.stringify(value));
	return value.result.value;
}
const media = (payload) => edit('media', payload);
const screens = (payload) => edit('screens', payload);
const selected = async (reference) =>
	(await media({ kind: 'InspectChannel', reference })).snapshot.definition.source.sender;
async function wait(read, accept, label) {
	const deadline = Date.now() + 12000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(100);
	}
}
const manager = page.locator('section[data-panel-id="media-manager"]');
const mode = page
	.locator('.parameter-inspector')
	.filter({ has: page.getByText('Sender selection', { exact: true }) })
	.locator('select');
const row = page
	.locator(
		'.channels-section > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
	)
	.first();
const sender = row.locator('select.dropdown-editor');
const options = () =>
	sender.locator('option').evaluateAll((items) =>
		items.map((item) => ({
			value: item.value,
			label: item.textContent
		}))
	);
async function chooseMedia(label) {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: label, exact: true }).click();
	await page
		.locator('.inspector-header .title-text')
		.filter({ hasText: new RegExp(`^${RegExp.escape(label)}$`) })
		.waitFor();
	await mode.waitFor();
}
async function createShared(label, expected) {
	const { reference } = await media({ kind: 'CreateMedia', media_kind: 'SharedTexture', label });
	assert.equal(await selected(reference), expected, `${label} default at creation`);
	await chooseMedia(label);
	assert.equal(await mode.inputValue(), 'Auto');
	assert.equal(await sender.inputValue(), expected ?? '');
	return reference;
}
async function colorOutput(label, rgba, target) {
	const { reference } = await media({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: `${label} source`
	});
	await media({
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'SolidColor', rgba: rgba.map((value) => value / 255) }
	});
	const { screen, surface } = await screens({ kind: 'CreateScreen', label });
	const { snapshot } = await screens({ kind: 'InspectSurface', reference: surface });
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...snapshot.definition, source: reference }
	});
	await screens({ kind: 'ConfigureScreen', screen, definition: { enabled: true, target } });
	return screen;
}
async function expectPixels(phase, expected, name) {
	// The Inspector witnesses the committed value. An SDK reconnect can then be needed.
	await delay(1300);
	await output.observePixels(phase, expected, 0, name);
}
const chosenLabel = () => sender.locator('option:checked').textContent();
const sdkCatalog = (refresh) =>
	page.evaluate(
		async (refresh) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverSenders', refresh })
				})
			),
		refresh
	);

try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	output = await createPreviewOutputObserver(post, path.join(directory, 'captures'), page);

	stage = 'empty SDK catalog';
	await sdkCatalog(true);
	const empty = await wait(
		() => sdkCatalog(false),
		(result) => !result.refreshing && !result.error,
		'initial complete SDK enumeration'
	);
	assert.deepEqual(empty.names, [], 'run with unrelated Spout senders closed');
	const witness = await createShared('Empty catalog witness', null);
	assert.deepEqual(await options(), [none]);
	checks.push({ stage, options: await options() });

	stage = 'two actual internal Screen outputs';
	const label = `Defaults red ${suffix}`;
	const redName = `MapGyver - ${label}`;
	const greenName = `MapGyver-Defaults-Green-${suffix}`;
	const redScreen = await colorOutput(label, red, { kind: 'AutomaticSpout', extent });
	await colorOutput('Defaults green', green, { kind: 'Spout', sender: greenName, extent });
	adapter = await observeNativeAdapter(page, hardware);
	const owned = await wait(
		options,
		(items) =>
			items.length === 3 &&
			items.some(({ value, label }) => value === redName && label === internal(redName).label) &&
			items.some(({ value, label }) => value === greenName && label === internal(greenName).label),
		'both Screen outputs classified by their registered SDK identities'
	);
	assert.equal(await selected(witness), null);
	await createShared('Internal-only creation', null);
	await expectPixels('direct-red', red, redName);
	await expectPixels('direct-green', green, greenName);
	checks.push({ stage, options: owned });

	stage = 'mixed SDK catalog';
	// This external SDK name deliberately resembles MapGyver's derived Screen names.
	const first = await source(adapter.index, `MapGyver - External ${suffix}`);
	sources.push(first);
	const second = await source(adapter.index, `Spout-Defaults-External-${suffix}`);
	sources.push(second);
	await first.command('start 96 64 0 0 1');
	await second.command('start 96 64 1 0 1');
	const mixed = await wait(
		options,
		(items) => [first, second].every(({ name }) => items.some((item) => item.value === name)),
		'both external SDK registrations available'
	);
	for (const { name } of sources)
		assert.deepEqual(
			mixed.find(({ value }) => value === name),
			{ value: name, label: name }
		);
	const external = new Map([
		[first.name, { source: first, color: blue, start: 'start 96 64 0 0 1' }],
		[second.name, { source: second, color: magenta, start: 'start 96 64 1 0 1' }]
	]);
	await sdkCatalog(true);
	const discovered = await wait(
		() => sdkCatalog(false),
		(result) => !result.refreshing && !result.error && result.names.includes(second.name),
		'mixed complete SDK enumeration'
	);
	assert.deepEqual(
		mixed.slice(1).map(({ value }) => value),
		discovered.names,
		'generic Inspector preserves SDK catalog order without changing any key'
	);
	const defaultName = discovered.names.find((name) => external.has(name));
	const choice = external.get(defaultName);
	const probe = await createShared('Mixed catalog creation', defaultName);
	assert.deepEqual(await options(), mixed);
	await output.demand(probe);
	await expectPixels('first-external-default', choice.color);
	checks.push({ stage, defaultName, options: mixed });

	stage = 'external loss';
	await first.command('stop');
	await second.command('stop');
	const lost = await wait(
		options,
		(items) =>
			items.some(({ label }) => label === `${defaultName} (unavailable)`) &&
			items.every(({ value, label }) => !external.has(value) || label.endsWith('(unavailable)')),
		'exact external selection stays unavailable while internal outputs remain'
	);
	assert.equal(await selected(probe), defaultName);
	assert.deepEqual(
		lost.find(({ value }) => value === redName),
		internal(redName)
	);
	assert.deepEqual(
		lost.find(({ value }) => value === greenName),
		internal(greenName)
	);
	await createShared('Creation after external loss', null);
	checks.push({ stage, options: lost });

	stage = 'same exact name registered by a Screen';
	const replacement = await colorOutput('Same-name internal replacement', green, {
		kind: 'Spout',
		sender: defaultName,
		extent
	});
	const replacementOptions = await wait(
		options,
		(items) =>
			items.some(
				({ value, label }) => value === defaultName && label === internal(defaultName).label
			),
		'the now-vacant exact name is classified internal'
	);
	await createShared('Creation with internal replacement', null);
	await expectPixels('same-name-internal-green', green, defaultName);
	await chooseMedia('Mixed catalog creation');
	assert.equal(await selected(probe), defaultName);
	await expectPixels('seeded-receiver-awaits-external', [0, 0, 0, 0]);
	checks.push({ stage, selected: await selected(probe), options: replacementOptions });

	stage = 'exact external recovery after internal retirement';
	await screens({ kind: 'RemoveScreen', screen: replacement });
	await wait(
		chosenLabel,
		(label) => label === `${defaultName} (unavailable)`,
		'exact-name Screen has retired'
	);
	await choice.source.command(choice.start);
	await wait(chosenLabel, (label) => label === defaultName, 'exact SDK sender returns');
	assert.equal(await selected(probe), defaultName);
	await expectPixels('recovered-external', choice.color);
	checks.push({ stage, selected: await selected(probe) });

	stage = 'deliberate internal choice and mode changes';
	await mode.selectOption('Manual');
	const manual = row.locator('input.string-editor');
	await wait(
		() => manual.inputValue(),
		(name) => name === defaultName,
		'Manual keeps external'
	);
	await row.getByRole('button', { name: 'Detect sender', exact: true }).click();
	await page.getByRole('menuitem', { name: internal(redName).label, exact: true }).click();
	await wait(
		() => selected(probe),
		(name) => name === redName,
		'Manual Detect stores exact name'
	);
	await wait(
		() => manual.inputValue(),
		(name) => name === redName,
		'Manual Inspector reflects the committed exact sender'
	);
	await expectPixels('explicit-internal-manual', red);
	await mode.selectOption('Auto');
	await wait(
		() => sender.inputValue(),
		(name) => name === redName,
		'Auto keeps internal'
	);
	assert.equal(await chosenLabel(), internal(redName).label);
	await expectPixels('explicit-internal-auto', red);
	checks.push({ stage, selected: await selected(probe) });

	stage = 'internal rename and independent peer';
	const renamed = `Renamed ${label}`;
	const renamedName = `MapGyver - ${renamed}`;
	await screens({ kind: 'RenameScreen', screen: redScreen, label: renamed });
	const renamedOptions = await wait(
		options,
		(items) =>
			items.some(
				({ value, label }) => value === renamedName && label === internal(renamedName).label
			) &&
			items.some(({ value, label }) => value === redName && label === `${redName} (unavailable)`),
		'rename updates catalog without retargeting the exact selection'
	);
	assert.equal(await selected(probe), redName);
	assert.deepEqual(
		renamedOptions.find(({ value }) => value === greenName),
		internal(greenName)
	);
	await sender.selectOption(greenName);
	await wait(
		() => selected(probe),
		(name) => name === greenName,
		'explicit alternate Screen'
	);
	await expectPixels('explicit-internal-peer', green);
	await sender.selectOption(renamedName);
	await wait(
		() => selected(probe),
		(name) => name === renamedName,
		'explicit renamed Screen'
	);
	await expectPixels('explicit-internal-renamed', red);
	checks.push({ stage, options: renamedOptions });
	assert.deepEqual(errors, []);
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, errors, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	try {
		await Promise.all(sources.map((fixture) => fixture.close()));
	} finally {
		await native.close();
	}
}
const result = {
	result: 'pass',
	pid: native.app.pid,
	adapter,
	sources: sources.map(({ name, pid }) => ({ name, pid })),
	checks,
	pixels: output.observations
};
await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
console.log(
	`Sender defaults passed on ${adapter.description}. ${path.join(directory, 'result.json')}`
);
