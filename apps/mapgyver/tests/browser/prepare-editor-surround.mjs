// Prepare reusable M13/13 projects without launching a desktop window or injecting input.
// MAPGYVER_TEST_BINARY must select the software-validated ordinary app executable.
import assert from 'node:assert/strict';
import { mkdtemp, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

const directory = await mkdtemp(path.resolve('target/m1313-surround-scenes-'));
const host = await openInitialScreenHeadless(directory);
let token;
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(10000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(namespace, payload) {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Surround red'
	});
	await edit('media', {
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 800, height: 600 } }
	});
	const { screen, surface } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Surround screen'
	});
	await edit('screens', { kind: 'RenameSurface', reference: surface, label: 'Surround surface' });
	const sender = `MapGyver-Surround-${screen}`;
	const originalScreen = await edit('screens', { kind: 'InspectScreen', screen });
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			...originalScreen.definition,
			enabled: true,
			target: { kind: 'Spout', sender, extent: { width: 800, height: 600 } }
		}
	});
	const originalSurface = (await edit('screens', { kind: 'InspectSurface', reference: surface }))
		.snapshot;
	const definition = {
		...originalSurface.definition,
		source,
		opacity: 1,
		// Binary-exact quarters keep authored f32 geometry and pixel landmarks exact.
		crop: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 },
		quad: {
			corners: [
				{ x: 0.25, y: 0.25 },
				{ x: 0.75, y: 0.25 },
				{ x: 0.75, y: 0.75 },
				{ x: 0.25, y: 0.75 }
			]
		}
	};
	await edit('screens', { kind: 'ConfigureSurface', reference: surface, definition });
	const inspect = async () => ({
		screen: await edit('screens', { kind: 'InspectScreen', screen }),
		surface: (await edit('screens', { kind: 'InspectSurface', reference: surface })).snapshot,
		channels: (
			await edit('media', {
				kind: 'InspectChannels',
				query: { media: source.media, start: 0, count: 1 }
			})
		).page
	});
	const cases = [];
	for (const [name, alpha] of [
		['half-alpha', 0.5],
		['opaque', 1],
		['missing', null]
	]) {
		if (alpha === null) await edit('media', { kind: 'RemoveMedia', media: source.media });
		else
			await edit('media', {
				kind: 'ConfigureChannel',
				reference: source,
				source: { kind: 'SolidColor', rgba: [1, 0, 0, alpha] }
			});
		const snapshot = await inspect();
		assert.deepEqual(snapshot.surface.definition, definition);
		assert.equal(snapshot.screen.definition.target.sender, sender);
		assert.equal(snapshot.screen.definition.enabled, true);
		if (alpha === null) {
			assert.equal(snapshot.channels.status, 'MissingMedia');
			assert.equal(snapshot.channels.total, 0);
		} else {
			assert.equal(snapshot.channels.status, 'Ready');
			assert.deepEqual(snapshot.channels.channels[0].reference, source);
			assert.deepEqual(snapshot.channels.channels[0].definition.source, {
				kind: 'SolidColor',
				rgba: [1, 0, 0, alpha]
			});
			assert.deepEqual(snapshot.channels.channels[0].definition.size, {
				kind: 'Fixed',
				extent: { width: 800, height: 600 }
			});
		}
		const file = path.join(directory, `${name}.mapgyver`);
		await post('/api/ui/project-save', { path: file });
		assert((await stat(file)).size < 32768, 'Acceptance scenes must stay small');
		cases.push({ name, file, snapshot });
	}
	for (const item of cases) {
		await post('/api/ui/project-new', {});
		await post('/api/ui/project-load', { path: item.file });
		assert.deepEqual(await inspect(), item.snapshot, `${item.name}: exact project reload`);
	}
	const result = {
		result: 'prepared',
		directory,
		source,
		screen,
		surface,
		sender,
		cases: cases.map(({ name, file }) => ({ name, file })),
		geometry: { extent: { width: 800, height: 600 }, crop: definition.crop, quad: definition.quad },
		remaining:
			'Actual desktop pixels on both GPUs, pan/zoom/resize, native imagery/overlay/hit alignment, missing/alpha, theme/scale, docking and project transitions; unchanged independent Spout output. Desktop input resumption remains required. No native acceptance is claimed.'
	};
	await writeFile(path.join(directory, 'prepared.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} finally {
	await host.close();
}
