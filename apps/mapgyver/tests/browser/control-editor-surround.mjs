// Explicit test stimuli for project lifetime and CSS appearance in one owned cohort.
// Computer Use activates the owned window. Pan uses the existing trusted WebView2 test path.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { request } from 'node:http';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const ready = JSON.parse(await readFile(path.resolve(process.argv[2], 'ready.json'), 'utf8'));
const action = process.argv[3];
const projectAction = ['half-alpha', 'opaque', 'missing', 'new'].includes(action);
function post(route, value, token) {
	return new Promise((resolve, reject) => {
		const req = request(
			ready.base + route,
			{
				method: 'POST',
				agent: false,
				headers: {
					'Content-Type': 'application/json',
					...(token ? { Authorization: `Bearer ${token}` } : {})
				}
			},
			(response) => {
				let text = '';
				response.setEncoding('utf8');
				response.on('data', (chunk) => (text += chunk));
				response.on('end', () => {
					if (response.statusCode !== 200) reject(Error(text));
					else {
						try {
							resolve(JSON.parse(text));
						} catch (error) {
							reject(error);
						}
					}
				});
			}
		);
		req.on('error', reject);
		req.end(JSON.stringify(value));
	});
}
if (projectAction) {
	const { token } = await post('/api/ui/session', {});
	const route = action === 'new' ? '/api/ui/project-new' : '/api/ui/project-load';
	await post(
		route,
		action === 'new'
			? {}
			: { path: ready.prepared.cases.find((item) => item.name === action).file },
		token
	);
} else {
	assert(['theme', 'scale', 'default', 'pan'].includes(action));
	const browser = await chromium.connectOverCDP(ready.cdp);
	const pages = browser
		.contexts()
		.flatMap((context) => context.pages())
		.filter((page) => page.url().startsWith(ready.base));
	assert.equal(pages.length, 1);
	const page = pages[0];
	if (action === 'pan') {
		const view = page.getByRole('application').filter({ visible: true });
		const before = await view.locator('.active-zone').boundingBox();
		const box = await view.boundingBox();
		assert(before && box);
		await page.evaluate(() => {
			window.__surroundPan = [];
			for (const type of ['pointerdown', 'pointerup'])
				document.addEventListener(
					type,
					(event) =>
						window.__surroundPan.push({ type, trusted: event.isTrusted, button: event.button }),
					{ once: true, capture: true }
				);
		});
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down({ button: 'middle' });
		await page.mouse.move(box.x + box.width / 2 + 80, box.y + box.height / 2 + 40, { steps: 12 });
		await page.mouse.up({ button: 'middle' });
		const after = await view.locator('.active-zone').boundingBox();
		const input = await page.evaluate(() => window.__surroundPan);
		assert(input.length === 2 && input.every((event) => event.trusted && event.button === 1));
		assert(Math.abs(after.x - before.x - 80) < 1 && Math.abs(after.y - before.y - 40) < 1);
		await writeFile(
			path.join(ready.directory, 'pan-input.json'),
			JSON.stringify({ before, after, input }, null, 2)
		);
	} else
		await page.evaluate((action) => {
			const style = document.documentElement.style;
			if (action === 'theme') style.setProperty('--gc-color-panel', '#454545');
			else if (action === 'scale') style.fontSize = '18px';
			else {
				style.removeProperty('--gc-color-panel');
				style.removeProperty('font-size');
			}
		}, action);
}
console.log(JSON.stringify({ action, pid: ready.pid }));
if (!projectAction) process.exit(0);
