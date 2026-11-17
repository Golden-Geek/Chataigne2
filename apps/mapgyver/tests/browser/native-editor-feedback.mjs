// Focused paint/input check in the bundled app; no GPU timing or physical scanout qualification.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { verifyScreenSelection } from './screen-selection.mjs';
import { verifySurfaceSourceUi } from './surface-source.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
let paintFrames = [];
const pending = new Map();
const observer = await page.context().newCDPSession(page);
observer.on('Network.requestWillBeSent', ({ requestId, request, timestamp }) => {
	if (!request.url.includes('native_presentation_control')) return;
	try {
		const args = JSON.parse(request.postData ?? '{}');
		const payload = JSON.parse(args.request);
		if (payload.kind !== 'Layout' && payload.kind !== 'Status') return;
		const record = { at: timestamp, request: payload };
		paintFrames.push(record);
		pending.set(requestId, record);
		if (paintFrames.length > 256) paintFrames.shift();
	} catch {}
});
observer.on('Network.loadingFinished', async ({ requestId }) => {
	const record = pending.get(requestId);
	if (!record) return;
	pending.delete(requestId);
	try {
		const { body } = await observer.send('Network.getResponseBody', { requestId });
		const value = JSON.parse(body);
		record.reply = typeof value === 'string' ? JSON.parse(value) : value;
	} catch {}
});
await observer.send('Network.enable');
let token;
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
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
	if (process.argv.includes('--foreground')) {
		console.log(JSON.stringify({ ownedWindowReady: native.app.pid }));
		await page.waitForTimeout(30000);
	}
	let source = null,
		screen = null;
	if (process.argv.includes('--source-only')) {
		await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Feedback source' });
		const inspectMedia = async () => (await edit({ kind: 'InspectManager' })).snapshot;
		source = await verifySurfaceSourceUi({
			base: native.base,
			post,
			page,
			media: await inspectMedia(),
			inspectMedia
		});
		await page.screenshot({ path: path.join(directory, 'source-feedback.png') });
	} else {
		screen = await verifyScreenSelection({
			base: native.base,
			post,
			page,
			beforeNew: async ({ a, mainId }) => {
				const panel = page.locator(`[data-panel-id="${mainId}"]`);
				const zone = await panel.locator('.active-zone').boundingBox();
				assert(zone);
				await page.mouse.click(zone.x + zone.width * 0.48, zone.y + zone.height * 0.48);
				await panel
					.getByRole('group', { name: 'Warp handles for Selection A', exact: true })
					.waitFor({ state: 'detached' });
				await page.waitForTimeout(150);
				paintFrames = [];
				await page.mouse.click(zone.x + zone.width * 0.25, zone.y + zone.height * 0.25);
				await page.waitForTimeout(250);
				const frames = paintFrames;
				await writeFile(path.join(directory, 'native-paint.json'), JSON.stringify(frames));
				const primitives = frames.flatMap((frame) =>
					(frame.request.views ?? []).flatMap((view) =>
						view.overlays.filter((item) => item.hit === a.surface)
					)
				);
				const intermediate = primitives.filter(
					(item) => item.fill[3] > 0 && item.fill[3] < 15 / 255
				);
				assert(
					new Set(intermediate.map((item) => item.fill[3])).size >= 2,
					'native Layout submits intermediate selection paint'
				);
				const revisions = new Set(
					frames
						.filter((frame) => frame.request.kind === 'Layout')
						.map((frame) => frame.request.revision)
				);
				const installed = frames.some(
					(frame) =>
						frame.reply?.kind === 'Applied' &&
						frame.reply.installed &&
						revisions.has(frame.reply.revision)
				);
				if (process.argv.includes('--require-installation'))
					assert(installed, 'matching native installation is acknowledged');
				const handle = panel.getByRole('button', { name: 'Warp corner 1', exact: true });
				await handle.focus();
				await page.keyboard.press('Enter');
				assert.equal(await handle.getAttribute('aria-pressed'), 'true');
				await page.screenshot({ path: path.join(directory, 'screen-feedback.png') });
				return {
					intermediateNativePaint: new Set(intermediate.map((item) => item.fill[3])).size,
					installed,
					keyboardCorner: true
				};
			}
		});
	}
	const result = {
		result: 'functional-pass',
		pid: native.app.pid,
		screen,
		source,
		qualification:
			'Pending matching native installation/presented-paint and actual Composition/Matrix presentation checks before M09',
		scope:
			'Bundled Screen/Source selection, intermediate accepted native paint requests, keyboard corner, input/history/cancel and independent editors. Installation/presented-paint, actual Composition/Matrix presentation and retained M14 lifetime/stress acceptance remain open.'
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failed.png') }).catch(() => {});
	throw error;
} finally {
	await observer.detach().catch(() => {});
	await native.close();
}
