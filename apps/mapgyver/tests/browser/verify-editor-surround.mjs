// Complete the prepared, foreground native cohort through the existing WebView2 test path.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
const browser = await chromium.connectOverCDP(ready.cdp);
const pages = browser
	.contexts()
	.flatMap((context) => context.pages())
	.filter((page) => page.url().startsWith(ready.base));
assert.equal(pages.length, 1);
const page = pages[0];
page.setDefaultTimeout(6000);
const stages = [],
	outputs = [],
	actions = [];
const resumeMissing = process.argv.includes('--resume-missing');
const prefix = resumeMissing ? 'recovered-' : process.argv.includes('--retry') ? 'retry-' : '';
const resumeMaximized = process.argv.includes('--resume-maximized');
const resume = process.argv.includes('--resume-theme') || resumeMaximized || resumeMissing;
if (resume) {
	const prior = JSON.parse(
		await readFile(
			path.join(directory, resumeMissing ? 'retry-matrix-failure.json' : 'matrix-failure.json'),
			'utf8'
		)
	);
	stages.push(...prior.stages);
	outputs.push(...prior.outputs);
	actions.push(...prior.actions);
}
const run = (file, args) =>
	execFileSync(
		process.execPath,
		[path.resolve('apps/mapgyver/tests/browser', file), directory, ...args],
		{ encoding: 'utf8', windowsHide: true }
	);
const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
const view = () => page.getByRole('application').filter({ visible: true });
async function select(name) {
	await tab('Screens').click();
	await page
		.locator('section[aria-label=Screens]')
		.getByRole('button', { name, exact: true })
		.click();
}
async function capture(stage, image = true) {
	stage = prefix + stage;
	let verified;
	for (let attempt = 0; ; attempt++) {
		const name = attempt ? `${stage}-settle-${attempt}` : stage;
		run('observe-editor-surround.mjs', [name]);
		try {
			verified = JSON.parse(
				execFileSync(
					'python',
					['apps/mapgyver/tests/native/verify_editor_surround.py', directory, name],
					{ encoding: 'utf8', windowsHide: true }
				)
			);
			break;
		} catch (error) {
			if (!stage.includes('theme') || attempt === 4) throw error;
			actions.push({ kind: 'theme-native-paint-pending', attempt, capture: name });
			await delay(200);
		}
	}
	if (image) assert(verified.red_pixels > 10000, `${stage}: native content must be present`);
	else assert.equal(verified.red_pixels, 0, `${stage}: old image must retire`);
	stages.push(verified);
}
async function output(stage, alpha) {
	stage = prefix + stage;
	const destination = path.join(directory, stage + '-output');
	execFileSync(
		path.resolve('target/debug/native_qualification.exe'),
		[
			'--role',
			'surround-pixels',
			'--adapter',
			String(ready.adapter.index),
			'--name',
			ready.prepared.sender,
			'--directory',
			destination
		],
		{ windowsHide: true }
	);
	const data = JSON.parse(
		execFileSync(
			'python',
			[
				'-c',
				'import sys,json,hashlib; from PIL import Image; from collections import Counter; im=Image.open(sys.argv[1]).convert("RGBA"); print(json.dumps({"sha256":hashlib.sha256(im.tobytes()).hexdigest(),"colors":[[list(c),n] for c,n in Counter(im.get_flattened_data()).items()]}))',
				path.join(destination, 'pixels.png')
			],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
	let colored = 0;
	for (const [color, count] of data.colors) {
		if (color.every((value) => value === 0)) continue;
		assert(
			alpha !== 0 &&
				color[0] === 255 &&
				color[1] === 0 &&
				color[2] === 0 &&
				Math.abs(color[3] - alpha) <= 2,
			JSON.stringify(data)
		);
		colored += count;
	}
	assert.equal(colored, alpha === 0 ? 0 : 120000);
	outputs.push({ stage, ...data });
	return data.sha256;
}
try {
	// Computer Use can park the physical pointer on the maximized titlebar first.
	// Restore through the existing test path so that pointer stays outside every viewport.
	if (process.argv.includes('--start-maximized'))
		await page.getByRole('button', { name: 'Maximize app', exact: true }).click();
	await page.evaluate(() => {
		window.__surroundTrusted ??= [];
		for (const type of ['pointerdown', 'pointerup', 'click'])
			document.addEventListener(
				type,
				(event) => {
					if (event.isTrusted && window.__surroundTrusted.length < 512)
						window.__surroundTrusted.push({ type, button: event.button });
				},
				{ capture: true }
			);
	});
	let baseline = outputs[0]?.sha256;
	if (!resume) {
		await select('Surround screen');
		await tab('Screen Warping').click();
		await capture('screen-bare');
		assert(stages.at(-1).image_edges_verified);
		baseline = await output('baseline', 128);
		await select('Surround surface');
		await capture('screen-selected');
		assert.equal(stages.at(-1).physical_handles, 4);
		await view().getByRole('button', { name: 'Zoom in', exact: true }).click();
		run('control-editor-surround.mjs', ['pan']);
		await capture('screen-pan-zoom');
		await tab('Surface Source').click();
		await capture('source-home');
		assert(stages.at(-1).image_edges_verified);
		await view().getByRole('button', { name: 'Zoom in', exact: true }).click();
		run('control-editor-surround.mjs', ['pan']);
		await capture('source-pan-zoom');
	}
	if (!resumeMissing) {
		if (!resumeMaximized) {
			if (!resume) run('control-editor-surround.mjs', ['theme']);
			await capture(resume ? 'source-theme-resumed' : 'source-theme');
			assert(stages.at(-1).gray_samples > 0);
			run('control-editor-surround.mjs', ['scale']);
			await capture('source-scale');
			await page.getByRole('button', { name: 'Maximize app', exact: true }).click();
		}
		await capture('source-maximized');
		assert(
			JSON.parse(
				await readFile(path.join(directory, `${prefix}source-maximized/capture.json`), 'utf8')
			).window_maximized
		);
		await page.getByRole('button', { name: 'Maximize app', exact: true }).click();
		await capture('source-restored');
		const box = await view().boundingBox();
		const sashes = await page
			.locator('.dv-sash.dv-enabled')
			.evaluateAll((elements) =>
				elements.map((element) => element.getBoundingClientRect().toJSON())
			);
		const candidates = sashes.filter(
			(rect) => rect.height > 100 && Math.abs(rect.x + rect.width / 2 - box.x - box.width) < 12
		);
		assert.equal(candidates.length, 1);
		const sash = candidates[0],
			x = sash.x + sash.width / 2,
			y = sash.y + sash.height / 2;
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x - 80, y, { steps: 12 });
		await page.mouse.up();
		const after = await view().boundingBox();
		assert(Math.abs(after.width - box.width) > 50);
		actions.push({ kind: 'dock-resize', before: box, after });
		await capture('source-docked');
		await tab('Screen Warping').click();
		await capture('screen-docked');
		assert.equal(
			await output('after-viewport', 128),
			baseline,
			'UI navigation/theme/scale/docking must not change output pixels'
		);
		run('control-editor-surround.mjs', ['opaque']);
		await select('Surround surface');
		await tab('Surface Source').click();
		await capture('source-opaque');
		await output('opaque', 255);
		run('control-editor-surround.mjs', ['missing']);
		await select('Surround surface');
		await tab('Screen Warping').click();
		await capture('screen-missing', false);
		await output('missing', 0);
		await tab('Surface Source').click();
	}
	await capture('source-missing', false);
	run('control-editor-surround.mjs', ['half-alpha']);
	await select('Surround surface');
	await tab('Surface Source').click();
	await capture('source-reloaded');
	assert.equal(await output('reloaded', 128), baseline);
	run('control-editor-surround.mjs', ['new']);
	const old = await page.locator('.native-preview[data-preview-source]').count();
	// New must withdraw the previous Surface binding rather than retain a red viewport.
	await page.waitForFunction(
		() =>
			![...document.querySelectorAll('.native-preview[data-preview-source]')].some((element) =>
				element.dataset.previewSource?.includes('e3e7e48a-45df-430b-8396-d5732579ff7c')
			)
	);
	actions.push({ kind: 'new-retires-old-source', priorViews: old });
	run('control-editor-surround.mjs', ['half-alpha']);
	await select('Surround surface');
	await tab('Screen Warping').click();
	await capture('screen-after-new-load');
	assert.equal(await output('after-new-load', 128), baseline);
	const trusted = await page.evaluate(() => window.__surroundTrusted);
	assert(trusted.length >= 20);
	const result = {
		result: 'pass',
		adapter: ready.adapter,
		stages,
		outputs,
		actions,
		trusted,
		scope:
			'Actual foreground desktop pixels, native image/DOM hit/physical handle alignment, pan/zoom/clipping, live CSS theme and rem scale stimuli, maximize/restore, dock resize, missing/opaque source and New/Load; byte-identical independent outputs through viewport and project changes.'
	};
	await writeFile(path.join(directory, 'matrix-result.json'), JSON.stringify(result, null, 2));
	await writeFile(path.join(directory, 'finish'), 'finish');
	console.log(
		JSON.stringify({
			result: 'pass',
			adapter: ready.adapter,
			stages: stages.length,
			outputs: outputs.length
		})
	);
} catch (error) {
	await writeFile(
		path.join(directory, prefix + 'matrix-failure.json'),
		JSON.stringify({ error: String(error), stages, outputs, actions }, null, 2)
	);
	throw error;
}
process.exit(0);
