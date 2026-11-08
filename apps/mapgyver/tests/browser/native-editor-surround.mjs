// Own one finite native app. Computer Use activates its window; the existing WebView2
// test driver supplies repeatable editor gestures before foreground pixel capture.
// Read ready.json for its handles; create finish after the complete adapter cohort.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

const directory = path.resolve(process.argv[2]);
const gpu = process.argv[3];
const prepared = JSON.parse(await readFile(path.resolve(process.argv[4]), 'utf8'));
assert(/^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu));
await mkdir(directory, { recursive: true });
assert(
	!(await stat(path.join(directory, 'ready.json')).catch(() => null)),
	'Fresh cohort directory required'
);
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = JSON.parse(
	execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
assert(hardware.adapters.some((adapter) => !adapter.software && adapterKey(adapter.id) === gpu));
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
const errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
const { rejected } = await observeNativeRejectedRequests(native.page, () => 'M13/13 surround');
let token;
async function post(route, value) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(value),
		signal: AbortSignal.timeout(10000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
try {
	token = (await post('/api/ui/session', {})).token;
	await post('/api/ui/project-load', {
		path: prepared.cases.find((item) => item.name === 'half-alpha').file
	});
	await native.page.waitForURL((url) => url.href.startsWith(native.base + '/'));
	await native.page.waitForLoadState('networkidle');
	await native.page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__));
	const adapter = await observeNativeAdapter(native.page, hardware);
	assert.equal(adapter.key, gpu);
	const ready = {
		pid: native.app.pid,
		hwnd: ownedWindowHandle(native.app.pid),
		base: native.base,
		cdp: native.cdp,
		directory,
		adapter,
		prepared
	};
	await writeFile(path.join(directory, 'ready.json'), JSON.stringify(ready, null, 2));
	console.log(JSON.stringify({ ready: directory, pid: ready.pid, hwnd: ready.hwnd, adapter }));
	const deadline = Date.now() + 8 * 60 * 1000;
	while (!(await stat(path.join(directory, 'finish')).catch(() => null))) {
		assert(Date.now() < deadline, 'Owned surround cohort deadline expired');
		assert.equal(native.app.exitCode, null, native.log());
		await delay(200);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	await writeFile(
		path.join(directory, 'host-result.json'),
		JSON.stringify({ result: 'pass', adapter, errors, rejected }, null, 2)
	);
} catch (error) {
	await writeFile(
		path.join(directory, 'host-failure.json'),
		JSON.stringify({ error: String(error), errors, rejected, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
