// M12/9: native thumbnail presentation, independent authored output and shared leases.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { clearMatrixScene } from './matrix-scene.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const execute = promisify(execFile);
const directory = path.resolve(process.argv[2]);
const gpu = process.env.MAPGYVER_THUMBNAIL_GPU;
assert(gpu);
await mkdir(directory, { recursive: true });
const appDataDirectory = path.join(directory, 'app-data');
await mkdir(path.join(appDataDirectory, 'MapGyver'), { recursive: true });
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = JSON.parse(
	(await execute(observer, ['--role', 'hardware'], { windowsHide: true })).stdout
);
await execute(
	observer,
	[
		'--role',
		'gpu-preferences',
		'--file',
		path.join(appDataDirectory, 'MapGyver/preferences.json'),
		'--selected',
		gpu
	],
	{ windowsHide: true }
);
const native = await openNativeEditor({ appDataDirectory });
const { page } = native;
let token;
const errors = [],
	observations = [];
page.on('pageerror', (error) => errors.push(String(error)));
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
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function waitFor(read, predicate, label) {
	const deadline = Date.now() + 12000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(50);
	}
}
const sizes = [
	{ width: 32, height: 128 },
	{ width: 128, height: 32 },
	{ width: 64, height: 64 }
];
const colors = [
	{ rgba: [1, 0, 0, 0.5], rgb: [188, 0, 0] },
	{ rgba: [0, 1, 0, 1], rgb: [0, 255, 0] },
	{ rgba: [0, 0, 1, 1], rgb: [0, 0, 255] }
];
let output;
try {
	token = (await post('/api/ui/session', {})).token;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.waitFor();
	const sources = [];
	for (let index = 0; index < 3; index++) {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: `Fit source ${index + 1}`
		});
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: colors[index].rgba }
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: sizes[index] }
		});
		sources.push(reference);
	}
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Thumbnail Fit acceptance'
	});
	await clearMatrixScene(edit, matrix.media);
	const matrixEdit = (value) => edit({ kind: 'Matrix', media: matrix.media, edit: value });
	const { items } = await matrixEdit({ kind: 'AddLayers', parent: null, count: 4 });
	const { group } = await matrixEdit({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Aspect group'
	});
	const { columns } = await matrixEdit({ kind: 'AddColumns', parent: group, count: 32 });
	await matrixEdit({
		kind: 'SetCells',
		assignments: items.flatMap((item) =>
			columns.map((column, index) => ({ layer: item.id, column, source: sources[index % 3] }))
		)
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Thumbnail Fit acceptance', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const panel = page.locator('[data-panel-id="matrix"]');
	const grid = panel.getByRole('grid');
	const previews = panel.locator('.cell-previews');
	const view = previews.locator('.native-preview');
	async function installed() {
		await waitFor(
			async () => ({
				count: await previews.getAttribute('data-matrix-preview-source-count'),
				available: await view.getAttribute('data-preview-state'),
				installed: await view.getAttribute('data-preview-installed')
			}),
			(v) => v.count === '3' && v.available === 'available' && v.installed === 'true',
			'three shared fitted source groups'
		);
		assert.equal(await view.count(), 1);
		await delay(150);
	}
	await installed();
	const adapter = await observeNativeAdapter(page, hardware);
	assert.equal(adapter.key, gpu);
	await writeFile(
		path.join(directory, 'activation-required.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				executable: process.env.MAPGYVER_TEST_BINARY,
				resume: path.join(directory, 'continue')
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			activate_owned_window: native.app.pid,
			resume: path.join(directory, 'continue')
		})
	);
	const deadline = Date.now() + 8 * 60 * 1000;
	for (;;) {
		try {
			await access(path.join(directory, 'continue'));
			break;
		} catch {}
		assert(Date.now() < deadline, 'owned activation checkpoint expired');
		await delay(100);
	}
	async function pixels(phase) {
		await installed();
		const cells = [];
		const rem = await page.evaluate(() =>
			parseFloat(getComputedStyle(document.documentElement).fontSize)
		);
		const clip = await view.boundingBox();
		assert(clip);
		const labelHeight = await panel
			.locator('.source-label')
			.first()
			.evaluate((el) => parseFloat(getComputedStyle(el).maxHeight));
		for (let index = 0; index < Math.min(columns.length, 12); index++) {
			const cell = panel.locator(
				`.row[data-row-id="${items[0].id}"] [role="gridcell"][aria-colindex="${index + 2}"]`
			);
			if ((await cell.count()) === 0) continue;
			const box = await cell.boundingBox();
			if (!box) continue;
			const aperture = {
				x: box.x + 0.2 * rem,
				y: box.y + 0.2 * rem,
				width: box.width - 0.4 * rem,
				height: box.height - labelHeight - 0.4 * rem
			};
			const size = sizes[index % 3],
				color = colors[index % 3];
			const scale = Math.min(aperture.width / size.width, aperture.height / size.height);
			const image = {
				x: aperture.x + (aperture.width - size.width * scale) / 2,
				y: aperture.y + (aperture.height - size.height * scale) / 2,
				width: size.width * scale,
				height: size.height * scale
			};
			const candidates = [
				{
					xy: [image.x + image.width / 2, image.y + image.height / 2],
					rgb: color.rgb,
					label: `source-${index}`
				}
			];
			if (image.x - aperture.x > 10)
				candidates.push({
					xy: [(image.x + aperture.x) / 2, image.y + image.height / 2],
					rgb: [0, 0, 0],
					label: `horizontal-margin-${index}`
				});
			if (image.y - aperture.y > 10)
				candidates.push({
					xy: [image.x + image.width / 2, (image.y + aperture.y) / 2],
					rgb: [0, 0, 0],
					label: `vertical-margin-${index}`
				});
			for (const probe of candidates)
				if (
					probe.xy[0] > clip.x + 3 &&
					probe.xy[0] < clip.x + clip.width - 3 &&
					probe.xy[1] > clip.y + 3 &&
					probe.xy[1] < clip.y + clip.height - 3
				)
					cells.push(probe);
		}
		assert(cells.length >= 4, `enough visible aspect/margin probes: ${cells.length}`);
		const folder = path.join(directory, phase);
		await mkdir(folder, { recursive: true });
		await writeFile(
			path.join(folder, 'ready.json'),
			JSON.stringify({
				pid: native.app.pid,
				dpr: await page.evaluate(() => devicePixelRatio),
				probes: cells
			})
		);
		const result = JSON.parse(
			(
				await execute('python', ['apps/mapgyver/tests/native/thumbnail_fit_pixels.py', folder], {
					windowsHide: true
				})
			).stdout
		);
		assert(result.passed);
		observations.push({ phase, ...result });
		assert.equal(
			(await panel.locator('[data-column-group-id]').count()) > 0,
			true,
			'Column Group header remains visible'
		);
		await output.observePixels(phase, [0, 255, 0, 255]);
	}
	output = await createPreviewOutputObserver(
		post,
		path.join(directory, 'independent-output'),
		page
	);
	await output.demand(sources[1]);
	await pixels('portrait-landscape-square');
	[sizes[0], sizes[1], sizes[2]] = [
		{ width: 128, height: 32 },
		{ width: 64, height: 64 },
		{ width: 32, height: 128 }
	];
	for (let index = 0; index < 3; index++)
		await edit({
			kind: 'ConfigureMediaResolution',
			media: sources[index].media,
			size: { kind: 'Fixed', extent: sizes[index] }
		});
	await delay(300);
	await pixels('live-source-dimensions');
	await grid.evaluate((el) => {
		el.scrollLeft = 40;
		el.dispatchEvent(new Event('scroll'));
	});
	await pixels('partial-scroll');
	await grid.evaluate((el) => {
		el.scrollLeft = 750;
		el.dispatchEvent(new Event('scroll'));
	});
	await pixels('virtualized-reuse');
	await grid.evaluate((el) => {
		el.scrollLeft = 0;
		el.style.width = '85%';
		el.dispatchEvent(new Event('scroll'));
	});
	await pixels('resized-viewport');
	assert.deepEqual(errors, []);
	await output.finish();
	output = null;
	await writeFile(
		path.join(directory, 'results.json'),
		JSON.stringify(
			{
				passed: true,
				pid: native.app.pid,
				adapter,
				observations,
				errors,
				scope:
					'actual native images, alpha/margins, groups, live dimensions, scroll/reuse, DOM viewport resize, three sources on one registration and independent SDK output'
			},
			null,
			2
		)
	);
} finally {
	if (output) await output.finish();
	await writeFile(path.join(directory, 'app.log'), native.log());
	await native.close();
}
