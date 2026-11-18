// Opens silent owned audio streams through the bundled Preferences and Golden Audio.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const appDataDirectory = path.join(directory, 'app-data');
const checks = [];
let expected, selected, failure;

async function wait(read, valid, label) {
	const deadline = Date.now() + 20000;
	for (;;) {
		const value = await read();
		if (valid(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(100);
	}
}

async function run(coldStart) {
	const native = await openNativeEditor({ appDataDirectory });
	const { page } = native;
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let token;
	async function post(route, body) {
		const response = await fetch(native.base + route, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				...(token ? { Authorization: `Bearer ${token}` } : {})
			},
			body: JSON.stringify(body),
			signal: AbortSignal.timeout(8000)
		});
		const value = await response.json();
		assert(response.ok, JSON.stringify(value));
		return value;
	}
	const intent = async (value) => {
		const ack = await post('/api/ui/intent', value);
		assert(ack.success, JSON.stringify(ack));
		return ack.result?.value;
	};
	const inspect = () =>
		intent({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.audio',
			operation: 'inspect-output',
			payload: null
		});
	const graph = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
	const parameter = (nodes, owner, name) =>
		nodes.find((node) => owner.children.includes(node.node_id) && node.decl_id === name);
	const setting = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const ready = (count, label) =>
		wait(inspect, (state) => state.readiness === 'ready' && state.routes.length === count, label);
	const revealPreferences = async () => {
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Preferences', { exact: true }) })
			.click();
	};
	try {
		token = (await post('/api/ui/session', {})).token;
		await revealPreferences();
		const enabled = setting('Enable output').getByRole('checkbox');
		const output = setting('Output device').getByRole('combobox');
		await output.waitFor();
		if (coldStart) {
			assert(await enabled.isChecked());
			assert.equal(await output.inputValue(), selected);
			const after = await ready(expected.length, 'cold-start output');
			assert.deepEqual([...after.routes].sort(), expected);
			checks.push({
				case: 'cold start retains the exact device and every virtual route',
				pid: native.app.pid,
				output: after
			});
			assert.deepEqual(errors, []);
			return;
		}
		assert.equal(await enabled.isChecked(), false);
		assert.equal(await output.inputValue(), 'System Default');
		await enabled.check();
		const initial = await ready(2, 'system default output with stereo routes');
		assert.equal(initial.active_device.backend, 'wasapi');
		assert(initial.format.channels >= 2);
		assert(initial.format.sample_rate > 0);
		selected = `${initial.active_device.backend}/${initial.active_device.device}`;
		await output.selectOption(selected);
		await wait(
			inspect,
			(value) => value.selected === selected && value.readiness === 'ready',
			'exact native output'
		);
		checks.push({
			case: 'Preferences opens the real WASAPI output, then selects it explicitly',
			pid: native.app.pid,
			output: initial
		});

		const nodes = await graph();
		const preferences = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
		assert(preferences);
		const left = nodes.find(
			(node) => preferences.children.includes(node.node_id) && node.decl_id === 'main-left'
		);
		const physical = parameter(nodes, left, 'physical');
		const leftChoice = page.locator(
			`.parameter-inspector[data-node-id="${physical.node_id}"] select`
		);
		const [first, second] = initial.physical_channels;
		assert(first && second && first !== second);
		await leftChoice.selectOption(second);
		await wait(
			inspect,
			(state) => state.routes.length === 2 && state.routes.every(([, port]) => port === second),
			'Left is mapped to the second physical channel'
		);

		const audio = page.locator(`.node-inspector[data-node-id="${preferences.node_id}"]`);
		await audio.getByRole('button', { name: 'Add child item', exact: true }).click();
		const extra = await wait(
			async () =>
				(await graph()).find(
					(node) =>
						node.node_type === 'mapgyver.preferences.audio.output' &&
						node.meta.label === 'Virtual Output'
				),
			Boolean,
			'new virtual output added from the Inspector'
		);
		await revealPreferences();
		const extraPhysical = parameter(await graph(), extra, 'physical');
		await page
			.locator(`.parameter-inspector[data-node-id="${extraPhysical.node_id}"] select`)
			.selectOption(first);
		const withExtra = await ready(3, 'a new virtual output can be patched');
		await intent({
			kind: 'duplicateNode',
			source: extra.node_id,
			new_parent: preferences.node_id,
			new_prev_sibling: extra.node_id,
			initial_params: []
		});
		const withCopy = await ready(4, 'duplicate virtual output has a distinct identity');
		assert.equal(new Set(withCopy.routes.map(([key]) => key)).size, 4);
		assert(
			withExtra.routes.every((route) =>
				withCopy.routes.some((copy) => copy[0] === route[0] && copy[1] === route[1])
			)
		);
		expected = [...withCopy.routes].sort();

		const device = parameter(nodes, preferences, 'device');
		const missing = 'wasapi/mapgyver-owned-absent-output';
		await intent({
			kind: 'setParam',
			node: device.node_id,
			value: { Enum: missing },
			behaviour: 'Append'
		});
		await wait(
			inspect,
			(state) =>
				state.selected === missing &&
				state.active_device === null &&
				state.routes.length === 0 &&
				['missing', 'unavailable', 'recovering'].includes(state.readiness),
			'missing explicit device preserves routes without fallback'
		);
		assert.equal(await leftChoice.inputValue(), second);
		assert.match(
			await leftChoice.locator(`option[value=${JSON.stringify(second)}]`).textContent(),
			/unavailable/
		);
		await output.selectOption(selected);
		const restored = await ready(
			expected.length,
			'returning to the selected device recovers playback routing'
		);
		assert.deepEqual([...restored.routes].sort(), expected);
		checks.push({
			case: 'virtual routes and independent duplicate identity survive a missing output choice',
			pid: native.app.pid,
			output: restored
		});

		await post('/api/ui/project-new', {});
		const reloaded = await ready(expected.length, 'project replacement');
		assert.deepEqual([...reloaded.routes].sort(), expected);
		assert.equal(reloaded.selected, selected);
		checks.push({
			case: 'project replacement retains global device and virtual route settings',
			pid: native.app.pid
		});
		assert.deepEqual(errors, []);
	} catch (error) {
		await writeFile(path.join(directory, `app-${native.app.pid}.log`), native.log());
		throw error;
	} finally {
		await native.close();
	}
}

try {
	await run(false);
	await run(true);
} catch (error) {
	failure = String(error.stack ?? error);
	throw error;
} finally {
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: failure ? 'fail' : 'pass', checks, failure }, null, 2)
	);
}
console.log(JSON.stringify({ result: 'pass', cases: checks.map((check) => check.case) }));
