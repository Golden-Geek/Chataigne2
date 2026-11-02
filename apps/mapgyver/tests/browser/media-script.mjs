import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMediaScriptUi({ base, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'media script creation and named API');
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		const manager = page.locator('section[aria-label="Media Manager"]');
		const background = manager.locator('.manager-list-content');
		const bounds = await background.boundingBox();
		await background.click({ position: { x: bounds.width / 2, y: bounds.height - 5 } });
		const scripts = page.getByRole('region', { name: 'Media scripts', exact: true });
		await scripts.waitFor();
		const loading = page.waitForResponse(
			(response) => response.url().endsWith('/script-state') && response.ok()
		);
		await scripts.getByRole('button', { name: 'Add Media Script', exact: true }).click();
		const state = await (await loading).json();
		assert.equal(state.config.source.kind, 'inline');
		assert(state.config.source.text.includes('local.configureMediaResolution'));
		assert(state.config.source.text.includes('onAppOperationResult'));
		assert.equal(state.config.source.text.includes('function update('), false);
		const editor = page.locator('.source-editor .cm-content');
		await editor.waitFor();
		assert.equal(
			await manager.getByRole('treeitem', { name: 'Media Script', exact: true }).count(),
			0
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await background.click({ position: { x: bounds.width / 2, y: bounds.height - 5 } });
		await scripts.waitFor();
		await scripts.locator('.node-inspector').waitFor({ state: 'detached' });
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await scripts.getByText('Media Script', { exact: true }).waitFor();
		// A new explicit creation selects the standard root Script Inspector and editor again.
		await scripts.getByRole('button', { name: 'Add Media Script', exact: true }).click();
		await editor.waitFor();
		const source = `function init() {
  local.listen({level: 1});
  local.createMedia("script-create", "SolidColor", "From Script");
}
function onAppOperationResult(reply) {
  if (reply.request_id === "script-create" && reply.status === "applied")
    local.configureMediaResolution("script-size", reply.value.reference.media, true, 17, 9);
}`;
		await editor.click();
		await editor.press('Control+a');
		await page.keyboard.insertText(source);
		const saving = page.waitForResponse((response) => response.url().endsWith('/script-config'));
		await editor.press('Control+s');
		const saved = await saving;
		assert(saved.ok(), 'Script source save must succeed');
		assert.equal(saved.request().postDataJSON().config.source.text, source);
		await page.getByRole('button', { name: 'Reload Runtime', exact: true }).click();
		const deadline = Date.now() + 5000;
		while (true) {
			const media = (await inspectMedia()).media.find((media) => media.label === 'From Script');
			if (media?.channels[0].definition.size.kind === 'Fixed') {
				assert.deepEqual(media.channels[0].definition.size.extent, { width: 17, height: 9 });
				break;
			}
			assert(
				Date.now() < deadline,
				'Named source API and committed callback must create and size media'
			);
			await delay(20);
		}
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled explicit media script creation/template, shared selection, Undo/Redo, standard code editor and named source API committed callback chain'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-4500)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
