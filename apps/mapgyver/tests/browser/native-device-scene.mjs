// A device-recovery fixture whose completion does not depend on the selected editor.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';

const directory = path.resolve(process.argv[2]);
const gpu = process.argv[3];
assert(/^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu));
const exists = async (name) => !!(await stat(path.join(directory, name)).catch(() => null));
assert(!(await exists('ready.json')), 'Use a fresh device-recovery directory');
await mkdir(directory, { recursive: true });
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = JSON.parse(
	execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const preferences = path.join(root, 'MapGyver');
		await mkdir(preferences, { recursive: true });
		execFileSync(
			observer,
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(preferences, 'preferences.json'),
				'--selected',
				gpu
			],
			{ windowsHide: true }
		);
	}
});
let token;
const post = async (route, value) => {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(value),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
};
const edit = async (namespace, payload) => {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
};
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Device recovery source'
	});
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const { screen, surface: reference } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Device recovery output'
	});
	const sender = `DeviceCheck-${screen}`;
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender, extent: { width: 320, height: 180 } }
		}
	});
	const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference });
	const definition = {
		...snapshot.definition,
		source,
		quad: {
			corners: [
				{ x: 0.2, y: 0.2 },
				{ x: 0.8, y: 0.2 },
				{ x: 0.8, y: 0.8 },
				{ x: 0.2, y: 0.8 }
			]
		}
	};
	await edit('screens', { kind: 'ConfigureSurface', reference, definition });
	const initial = (await edit('screens', { kind: 'InspectSurface', reference })).snapshot;
	await native.page.waitForURL((url) => url.href.startsWith(native.base + '/'));
	await native.page.waitForFunction(() => !!window.__TAURI_INTERNALS__?.invoke);
	const adapter = await observeNativeAdapter(native.page, hardware);
	assert.equal(adapter.key, gpu);
	await writeFile(
		path.join(directory, 'ready.json'),
		JSON.stringify({ pid: native.app.pid, base: native.base, cdp: native.cdp, reference, adapter })
	);
	console.log(JSON.stringify({ ready: true, pid: native.app.pid, sender, gpu }));
	const deadline = Date.now() + 480000;
	while (!(await exists('finish'))) {
		assert(Date.now() < deadline, 'device-recovery fixture expired');
		assert.equal(native.app.exitCode, null, native.log());
		await delay(100);
	}
	const final = (await edit('screens', { kind: 'InspectSurface', reference })).snapshot;
	assert.deepEqual(final, initial, 'Device recovery must preserve authored Surface state');
	await writeFile(path.join(directory, 'authored-state.json'), JSON.stringify({ initial, final }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), log: native.log() })
	);
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'closed.json'),
		JSON.stringify({ pid: native.app.pid, exit_code: native.app.exitCode, log: native.log() })
	);
}
