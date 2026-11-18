// Real generic Inspector/menu edits of app-owned Golden color metadata.
import assert from 'node:assert/strict';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const host = await openNativeEditor();
const { page } = host;
let token,
	stage = 'setup';
const checks = [],
	errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
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
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
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
async function wait(read, valid) {
	const until = Date.now() + 5000;
	for (;;) {
		const value = await read();
		if (valid(value)) return value;
		assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
const row = (label) =>
	page
		.getByRole('button', { name: label, exact: true })
		.locator('xpath=ancestor::div[contains(@class,"outliner-item-content")][1]');
async function select(target) {
	await page
		.getByRole('tab')
		.filter({ hasText: new RegExp(`^${target.manager}$`) })
		.click();
	await page
		.locator(`section[aria-label="${target.manager}"]`)
		.getByRole('button', { name: target.label, exact: true })
		.click();
	await wait(
		() => page.locator('.inspector').getAttribute('data-inspected-uuid'),
		(uuid) => uuid === target.uuid
	);
}
async function menu(action) {
	await page.locator('.inspector-header').click({ button: 'right' });
	await page.getByRole('menuitem', { name: action, exact: true }).click();
}
try {
	token = (await post('/api/ui/session', {})).token;
	const media = (
		await edit('media', { kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Colored Media' })
	).reference;
	const reference = (
		await edit('media', {
			kind: 'CreateReference',
			target: media.media,
			label: 'Colored Reference'
		})
	).media;
	const outer = (await edit('media', { kind: 'CreateFolder', parent: null, label: 'Color Folder' }))
		.folder;
	const nested = (
		await edit('media', { kind: 'CreateFolder', parent: outer, label: 'Nested Color' })
	).folder;
	const initial = (await edit('screens', { kind: 'InspectManager' })).snapshot.screens[0];
	const surface = initial.surfaces[0].reference;
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: media,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const definition = (await edit('screens', { kind: 'InspectSurface', reference: surface }))
		.snapshot.definition;
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference: surface,
		definition: {
			...definition,
			source: media,
			quad: {
				corners: [
					[0.1, 0.2],
					[0.45, 0.2],
					[0.45, 0.8],
					[0.1, 0.8]
				].map(([x, y]) => ({ x, y }))
			}
		}
	});
	const peer = (
		await edit('screens', { kind: 'AddSurface', screen: initial.id, label: 'Other Surface' })
	).reference;
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference: peer,
		definition: {
			...definition,
			quad: {
				corners: [
					[0.55, 0.2],
					[0.9, 0.2],
					[0.9, 0.8],
					[0.55, 0.8]
				].map(([x, y]) => ({ x, y }))
			}
		}
	});
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen: initial.id,
		definition: {
			...initial.definition,
			target: {
				kind: 'Spout',
				sender: `ObjectColors-${initial.id}`,
				extent: { width: 256, height: 160 }
			}
		}
	});
	await edit('screens', { kind: 'RenameScreen', screen: initial.id, label: 'Colored Screen' });
	await edit('screens', { kind: 'RenameSurface', reference: surface, label: 'Colored Surface' });
	const targets = [
		{ manager: 'Screens', label: 'Colored Screen', uuid: initial.id, hex: '40B080' },
		{ manager: 'Screens', label: 'Colored Surface', uuid: surface.surface, hex: 'E04080' },
		{ manager: 'Medias', label: 'Colored Media', uuid: media.media, hex: 'F0A030' },
		{ manager: 'Medias', label: 'Colored Reference', uuid: reference, hex: '6080E0' },
		{ manager: 'Medias', label: 'Color Folder', uuid: outer, hex: '40A8C0' },
		{ manager: 'Medias', label: 'Nested Color', uuid: nested, hex: 'A060C0' }
	];
	for (const target of targets) {
		stage = `Inspector color ${target.label}`;
		if (
			target.uuid === nested &&
			!(await page.getByRole('button', { name: target.label, exact: true }).count())
		)
			await row('Color Folder').locator(':scope > div[aria-hidden="true"]').click();
		await select(target);
		await page.locator('.inspector-header').click({ button: 'right' });
		assert(await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).isDisabled());
		await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
		const input = page.locator('.gc-node-context-color-submenu .cp-input-hex');
		await input.fill(target.hex);
		await input.press('Tab');
		await page.keyboard.press('Escape');
		await page.keyboard.press('Escape');
		await wait(
			() => row(target.label).getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		const authored = await row(target.label).evaluate((node) =>
			getComputedStyle(node).getPropertyValue('--node-accent-color').trim()
		);
		const inspector = await page
			.locator('.inspector-header')
			.evaluate((node) => getComputedStyle(node).getPropertyValue('--node-accent-color').trim());
		assert.equal(inspector, authored, 'Inspector and manager use the same authored color');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => row(target.label).getAttribute('class'),
			(value) => !value.includes('custom-color')
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => row(target.label).getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		await menu('Reset Color');
		await wait(
			() => row(target.label).getAttribute('class'),
			(value) => !value.includes('custom-color')
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => row(target.label).getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		checks.push({ kind: target.label, authored, inspector, set_reset_history: true });
	}
	stage = 'Screen frame and selected Surface cues';
	await select(targets[1]);
	await page.locator('.gc-dv-tab[data-content-panel-id="screen-warp"]').click();
	const panel = page.locator('section[data-panel-id="screen-warp"]');
	await wait(
		() => panel.getAttribute('aria-busy'),
		(value) => value === 'false'
	);
	const contour = panel.locator(`polygon[data-native-hit-id="${surface.surface}"]`);
	await wait(
		() => contour.evaluate((node) => getComputedStyle(node).stroke),
		(value) => value === 'rgb(112, 170, 255)'
	);
	assert.equal(
		await panel
			.getByRole('application', { name: 'Screen view', exact: true })
			.evaluate((node) =>
				getComputedStyle(node).getPropertyValue('--gc-viewport-frame-color').trim()
			),
		'#40b080ff'
	);
	checks.push('custom Screen frame and distinct selected Surface cue');
	await select({ manager: 'Screens', label: 'Other Surface', uuid: peer.surface });
	await page.locator('.gc-dv-tab[data-content-panel-id="screen-warp"]').click();
	await wait(
		() => contour.evaluate((node) => getComputedStyle(node).stroke),
		(value) => value === 'rgb(224, 64, 128)'
	);
	checks.push('unselected Surface uses its authored color; selected peer remains distinct');
	for (const theme of ['dark', 'light']) {
		if (theme === 'light')
			await page.evaluate(() => {
				const s = document.documentElement.style;
				for (const [key, value] of Object.entries({
					'--gc-color-background': '#f4f5f7',
					'--gc-color-panel': '#ffffff',
					'--gc-color-header': '#e8eaf0',
					'--gc-color-text': '#20242a'
				}))
					s.setProperty(key, value);
			});
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		const styles = await row('Colored Surface').evaluate((node) => ({
			color: getComputedStyle(node).color,
			background: getComputedStyle(node).backgroundColor,
			accent: getComputedStyle(node).getPropertyValue('--node-accent-color')
		}));
		await page.screenshot({ path: path.join(directory, `${theme}-ui.png`) });
		checks.push({ theme, styles, scope: 'WebView DOM; native underlay pixels are separate' });
	}
	await page.evaluate(() => {
		for (const key of [
			'--gc-color-background',
			'--gc-color-panel',
			'--gc-color-header',
			'--gc-color-text'
		])
			document.documentElement.style.removeProperty(key);
	});
	if (process.argv.includes('--review')) {
		const ready = { pid: host.app.pid, hwnd: ownedWindowHandle(host.app.pid), cdp: host.cdp };
		await writeFile(path.join(directory, 'ready.json'), JSON.stringify(ready));
		console.log(JSON.stringify({ readyForReview: true, ...ready }));
		const until = Date.now() + 8 * 60 * 1000;
		for (;;) {
			try {
				await access(path.join(directory, 'review.done'));
				break;
			} catch {}
			assert(Date.now() < until, 'visual review expired');
			assert.equal(host.app.exitCode, null);
			await delay(100);
		}
	}
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ status: 'pass', checks }, null, 2)
	);
	console.log(JSON.stringify({ status: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, log: host.log() }, null, 2)
	);
	throw error;
} finally {
	await host.close();
}
