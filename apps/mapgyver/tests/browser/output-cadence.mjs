// Typed Screen policy through the actual bundled host and its public API-delivery observations.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { verifyOutputCadenceRouting } from './output-cadence-routing.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY, 'Use a verified current binary');
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
const results = [];
for (const adapter of hardware.adapters.filter((adapter) => !adapter.software)) {
	const native = await openNativeEditor({
		prepareProfile: async (root) => {
			const folder = path.join(root, 'MapGyver');
			await mkdir(folder, { recursive: true });
			execFileSync(
				probe,
				[
					'--role',
					'gpu-preferences',
					'--file',
					path.join(folder, 'preferences.json'),
					'--selected',
					adapterKey(adapter.id)
				],
				{ windowsHide: true }
			);
		}
	});
	const errors = [];
	native.page.on('pageerror', (error) => errors.push(String(error)));
	let token,
		stage = 'startup';
	const checks = [];
	async function post(route, body) {
		const response = await fetch(native.base + route, {
			method: 'POST',
			signal: AbortSignal.timeout(10000),
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
	const control = (request) =>
		native.page.evaluate(
			async (request) =>
				JSON.parse(
					await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
						request: JSON.stringify(request)
					})
				),
			request
		);
	const status = () => control({ kind: 'mapgyverRuntimeStatus' });
	const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	async function wait(read, ready) {
		const until = Date.now() + 15000;
		for (;;) {
			const value = await read();
			if (ready(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
			await delay(100);
		}
	}
	try {
		await native.page.locator('footer.gc-footer output[aria-label="Video runtime"]').waitFor();
		token = (await post('/api/ui/session', {})).token;
		const first = (await inspect()).screens[0];
		const second = await edit({ kind: 'CreateScreen', label: 'Cadence peer' });
		const ids = [first.id, second.screen];
		const names = ids.map((_, index) => `MapGyverCadence-${randomUUID()}-${index}`);
		for (let index = 0; index < ids.length; index++) {
			const current = (await inspect()).screens.find((screen) => screen.id === ids[index]);
			await edit({
				kind: 'ConfigureSurface',
				reference: current.surfaces[0].reference,
				definition: { ...current.surfaces[0].definition, calibration: 'ColorBars' }
			});
			await edit({
				kind: 'ConfigureScreen',
				screen: ids[index],
				definition: {
					...current.definition,
					target: { kind: 'Spout', sender: names[index], extent: { width: 96, height: 64 } },
					frame_rate: { kind: 'Explicit', fps: index ? 60 : 30 }
				}
			});
		}
		const observedAdapter = await observeNativeAdapter(native.page, hardware);
		assert.equal(observedAdapter.key, adapterKey(adapter.id));
		const displays = await wait(
			() => control({ kind: 'mapgyverDisplays', refresh: false }),
			(value) => value.revision > 0 && value.displays.length > 0
		);
		assert(
			displays.displays.every(
				(display) => display.refresh_rate?.numerator > 0 && display.refresh_rate?.denominator > 0
			)
		);
		if (process.argv.includes('--routing')) {
			stage = 'physical cadence routing and recovery';
			const report = await verifyOutputCadenceRouting({
				native,
				post,
				edit,
				inspect,
				status,
				wait,
				ids,
				displays,
				directory,
				adapter: observedAdapter,
				checks
			});
			assert.deepEqual(errors, []);
			results.push({
				pid: native.app.pid,
				adapter: observedAdapter,
				displays,
				checks,
				errors,
				scope: report.scope
			});
			continue;
		}
		for (const rates of [
			[30, 60],
			[50, 59.94],
			[120, 30]
		]) {
			stage = `rates ${rates}`;
			for (let index = 0; index < ids.length; index++) {
				const current = (await inspect()).screens.find((screen) => screen.id === ids[index]);
				await edit({
					kind: 'ConfigureScreen',
					screen: ids[index],
					definition: { ...current.definition, frame_rate: { kind: 'Explicit', fps: rates[index] } }
				});
			}
			const before = await status();
			// Wait for two fresh coarse samples after normal topology retirement/recovery.
			const measured = await wait(
				status,
				(value) =>
					value.sequence >= before.sequence + 3 &&
					value.state === 'Running' &&
					ids.every((id, index) => {
						const row = value.screens.find((screen) => screen.screen === id);
						return (
							row?.accepted_fps != null &&
							Math.abs(row.accepted_fps - rates[index]) <= Math.max(2, rates[index] * 0.1)
						);
					})
			);
			assert.equal(measured.gpu_adapter, adapterKey(adapter.id));
			checks.push({ stage, status: measured });
		}
		stage = 'ordinary Inspector';
		await native.page
			.getByRole('tab')
			.filter({ hasText: /^Screens(?::|$)/ })
			.click();
		await native.page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: first.label, exact: true })
			.click();
		const fps = native.page
			.locator('.parameter-inspector')
			.filter({ has: native.page.getByText('FPS', { exact: true }) });
		await fps.waitFor();
		assert.equal(Number(await fps.locator('input.number-field').inputValue()), 120);
		const current = (await inspect()).screens.find((screen) => screen.id === ids[0]);
		await edit({
			kind: 'ConfigureScreen',
			screen: ids[0],
			definition: { ...current.definition, frame_rate: { kind: 'FollowDisplay' } }
		});
		await fps.waitFor({ state: 'detached' });
		const followed = await wait(status, (value) =>
			value.screens.some(
				(row) =>
					row.screen === ids[0] && row.accepted_fps != null && Math.abs(row.accepted_fps - 60) <= 2
			)
		);
		checks.push({ stage: 'Shared Texture follow-display fallback', status: followed });
		assert.deepEqual(errors, []);
		results.push({ pid: native.app.pid, adapter: observedAdapter, displays, checks, errors });
	} catch (error) {
		await writeFile(
			path.join(directory, 'failure.json'),
			JSON.stringify(
				{ stage, error: String(error), results, checks, errors, log: native.log() },
				null,
				2
			)
		);
		throw error;
	} finally {
		await native.close();
	}
}
await writeFile(
	path.join(directory, 'result.json'),
	JSON.stringify(
		{
			result: 'pass',
			results,
			scope:
				'Actual bundled typed controls and output API-delivery rates. No scanout, presented manipulation FPS, latency or final pacing-budget claim.'
		},
		null,
		2
	)
);
console.log(JSON.stringify({ result: 'pass', adapters: results.map((result) => result.adapter) }));
