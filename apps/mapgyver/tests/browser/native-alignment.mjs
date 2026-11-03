// Owned native scene for same-frame image/overlay acceptance. Capture it with native window pixels,
// never Playwright screenshots (which omit the underlay). No production instrumentation.
// Use a fresh output directory; write its `finish` marker after capture to stop normally.
import assert from 'node:assert/strict';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-native-alignment-${Date.now()}`);
await mkdir(directory, { recursive: true });
const exists = async (name) => !!(await stat(path.join(directory, name)).catch(() => null));
assert(!(await exists('ready.json')), 'Alignment fixture requires a fresh output directory');
const native = await openNativeEditor();
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
}
async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Alignment red'
	});
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const { screen, surface: reference } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Alignment screen'
	});
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Alignment-${screen}`, extent: { width: 800, height: 600 } }
		}
	});
	const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference });
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference,
		definition: {
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
		}
	});
	const page = native.page;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Manager$/ })
		.click();
	await page
		.locator('section[aria-label="Screen Manager"]')
		.getByRole('button', { name: snapshot.label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Warping$/ })
		.click();
	const view = page.getByRole('application', { name: 'Screen view', exact: true });
	await view
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	await delay(250);
	const bounds = await view.locator('.active-zone').boundingBox();
	assert(bounds);
	const metadata = {
		pid: native.app.pid,
		directory,
		base: native.base,
		reference,
		viewport: await page.evaluate(() => ({
			width: innerWidth,
			height: innerHeight,
			dpr: devicePixelRatio
		})),
		active: bounds
	};
	await writeFile(path.join(directory, 'ready.json'), JSON.stringify(metadata, null, 2));
	console.log(JSON.stringify({ ready: metadata }));
	const deadline = Date.now() + 5 * 60_000;
	while (!(await exists('finish'))) {
		assert(Date.now() < deadline, 'alignment fixture timeout');
		await delay(50);
	}
	console.log(
		JSON.stringify({
			result: 'fixture complete',
			project: (await edit('screens', { kind: 'InspectSurface', reference })).snapshot
		})
	);
} finally {
	await native.close();
}
