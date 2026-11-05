import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

const native = await openNativeEditor();
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.screens',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function waitFor(read, predicate) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, JSON.stringify(value));
		await delay(20);
	}
}
try {
	const errors = [];
	native.page.on('pageerror', (error) => errors.push(String(error)));
	token = (await post('/api/ui/session', {})).token;
	const { screen } = await edit({ kind: 'CreateScreen', label: 'AA Inspector check' });
	await edit({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			target: { kind: 'Spout', sender: `AA-${native.app.pid}`, extent: { width: 64, height: 32 } },
			enabled: true,
			anti_aliasing: 'Raw'
		}
	});
	const inspect = async () => (await edit({ kind: 'InspectScreen', screen })).definition;
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await native.page.getByRole('button', { name: 'AA Inspector check', exact: true }).click();
	const controls = screenParameterControls(native.page);
	const choice = controls.choice('Anti-aliasing');
	await choice.waitFor();
	assert.equal(await choice.inputValue(), 'Raw');
	assert.deepEqual(await choice.locator('option').allTextContents(), ['Raw', 'Smooth (2×)']);
	const original = await inspect();
	for (const [label, mode] of [
		['Smooth (2×)', 'Smooth2x'],
		['Raw', 'Raw'],
		['Smooth (2×)', 'Smooth2x']
	]) {
		await choice.selectOption({ label });
		const definition = await waitFor(inspect, (definition) => definition.anti_aliasing === mode);
		assert.deepEqual({ ...definition, anti_aliasing: 'Raw' }, original);
	}
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			pid: native.app.pid,
			scope:
				'actual WebView2 standard typed Screen Inspector, Raw default, both options and repeated switching preserve endpoint/enabled state',
			definition: await inspect(),
			page_errors: errors
		})
	);
} catch (error) {
	console.error('Inspector failure:', await native.page.locator('.inspector').innerText());
	await native.page.screenshot({ path: 'target/m12-screen-aa/inspector-failure.png' });
	throw error;
} finally {
	await native.close();
}
