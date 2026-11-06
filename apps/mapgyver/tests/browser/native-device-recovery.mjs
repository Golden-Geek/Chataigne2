// Public authoring/status control for two already running, isolated native fixtures.
import assert from 'node:assert/strict';
import { readFile, writeFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';

const root = path.resolve(process.argv[2]);
const action = process.argv[3];
assert(['prepare', 'watch', 'observe', 'green'].includes(action));
const apps = [];
try {
	for (const name of ['nvidia', 'intel']) {
		const directory = path.join(root, name);
		const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
		const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
		const pages = browser
			.contexts()
			.flatMap((c) => c.pages())
			.filter((p) => p.url().startsWith(ready.base + '/'));
		assert.equal(pages.length, 1);
		let token;
		const post = async (route, value) => {
			const response = await fetch(ready.base + route, {
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
		token = (await post('/api/ui/session', {})).token;
		const edit = async (namespace, payload) => {
			const response = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: `mapgyver.${namespace}`,
				operation: 'edit',
				payload
			});
			assert(response.success);
			return response.result.value;
		};
		apps.push({ name, directory, ready, browser, page: pages[0], edit });
	}
	if (action === 'prepare') {
		for (const app of apps) {
			const { definition } = await app.edit('screens', {
				kind: 'InspectScreen',
				screen: app.ready.reference.screen
			});
			const sender = `DeviceCheck-${app.ready.reference.screen}`;
			await app.edit('screens', {
				kind: 'ConfigureScreen',
				screen: app.ready.reference.screen,
				definition: {
					...definition,
					target: { kind: 'Spout', sender, extent: { width: 320, height: 180 } }
				}
			});
			const { snapshot } = await app.edit('screens', {
				kind: 'InspectSurface',
				reference: app.ready.reference
			});
			await writeFile(
				path.join(app.directory, 'device-ready.json'),
				JSON.stringify(
					{
						sender,
						source: snapshot.definition.source,
						surface: snapshot,
						gpu: app.ready.adapter.key,
						pid: app.ready.pid
					},
					null,
					2
				)
			);
			console.log(JSON.stringify({ name: app.name, pid: app.ready.pid, sender }));
		}
		await delay(500);
	} else if (action === 'green') {
		const app = apps.find((a) => a.name === 'nvidia');
		const { source } = JSON.parse(
			await readFile(path.join(app.directory, 'device-ready.json'), 'utf8')
		);
		await app.edit('media', {
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba: [0, 1, 0, 1] }
		});
		console.log(JSON.stringify({ nvidia_source_green: true }));
	} else {
		const observations = [];
		const started = Date.now();
		do {
			const statuses = await Promise.all(
				apps.map(async (app) => {
					try {
						const status = await app.page.evaluate(async () =>
							JSON.parse(
								await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
									request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
								})
							)
						);
						return { name: app.name, pid: app.ready.pid, status };
					} catch (error) {
						return { name: app.name, pid: app.ready.pid, observation_error: String(error) };
					}
				})
			);
			const sample = { time: new Date().toISOString(), elapsed_ms: Date.now() - started, statuses };
			observations.push(sample);
			if (action === 'observe') {
				console.log(JSON.stringify(sample));
				break;
			}
			await writeFile(path.join(root, 'device-latest.json'), JSON.stringify(sample, null, 2));
			await writeFile(
				path.join(root, 'device-observations.json'),
				JSON.stringify(observations, null, 2)
			);
			if (await stat(path.join(root, 'watch-stop')).catch(() => null)) break;
			await delay(500);
		} while (Date.now() - started < 300000);
	}
} finally {
	await Promise.allSettled(apps.map((app) => app.browser.close()));
}
