import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
import { verifyApplicationCommandUi } from './application-command.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMediaCommandUi({ base, edit, intent, post, inspectMedia }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Command target'
	});
	await edit({ kind: 'AddChannel', media: reference.media, label: null });
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'typed source command');
	const parameter = (name) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(name, { exact: true }) });
	async function commandResolution() {
		const root = Number(await parameter('Resolution').getAttribute('data-node-id'));
		const snapshot = await post('/api/ui/snapshot', { scope: { subtree: { root, max_depth: 0 } } });
		return snapshot.nodes[0].data.param.value;
	}
	async function waitResolutionProjection() {
		const deadline = Date.now() + 5000;
		while (true) {
			const inputs = await parameter('Resolution').locator('input.number-field').all();
			const values = await Promise.all(inputs.map((input) => input.inputValue()));
			if (JSON.stringify(values) === '["17","9"]') return;
			assert(Date.now() < deadline, `Command Resolution projection: ${JSON.stringify(values)}`);
			await delay(20);
		}
	}
	async function waitSize(kind) {
		const deadline = Date.now() + 5000;
		while (true) {
			const media = (await inspectMedia()).media.find((media) => media.id === reference.media);
			if (
				media.channels.every(
					(channel) =>
						channel.definition.size.kind === kind &&
						(kind !== 'Fixed' ||
							(channel.definition.size.extent.width === 17 &&
								channel.definition.size.extent.height === 9))
				)
			)
				return;
			assert(Date.now() < deadline, JSON.stringify(media));
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		const manager = page.locator('section[aria-label="Medias"]');
		const background = manager.locator('.manager-list-content');
		const bounds = await background.boundingBox();
		await background.click({ position: { x: bounds.width / 2, y: bounds.height - 5 } });
		const commands = page.getByRole('region', { name: 'Media commands', exact: true });
		await commands.getByRole('button', { name: 'Add child item', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Media', exact: true }).hover();
		await page.getByRole('menuitem', { name: 'Set Resolution', exact: true }).click();
		await parameter('Force resolution').waitFor();
		assert.equal(await parameter('Color').count(), 0);
		assert.equal(await parameter('Sender').count(), 0);
		assert.equal(
			await manager.getByRole('treeitem', { name: 'Set Resolution', exact: true }).count(),
			0
		);
		// Configure the real typed reference through the same public intent used by Golden's picker.
		const node = Number(await parameter('Media').getAttribute('data-node-id'));
		const ack = await intent({
			kind: 'setParam',
			node,
			value: { Reference: { uuid: reference.media } },
			behaviour: 'Append'
		});
		assert(ack.success, JSON.stringify(ack));
		await parameter('Force resolution').getByRole('checkbox').check();
		for (const [index, value] of [17, 9].entries()) {
			const input = parameter('Resolution').locator('input.number-field').nth(index);
			await input.fill(String(value));
			await input.press('Enter');
		}
		await waitSize('Automatic');
		await parameter('Execute').locator('button.trigger').click();
		await waitSize('Fixed');
		assert.deepEqual(await commandResolution(), { Vec2: [17, 9] });
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await waitSize('Automatic');
		assert.deepEqual(
			await commandResolution(),
			{ Vec2: [17, 9] },
			'Undo preserves authored command configuration'
		);
		await waitResolutionProjection();
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await waitSize('Fixed');
		await verifyApplicationCommandUi({ page, edit, intent, parameter, background, commands });
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled command catalog/selection, typed generic configuration, passive editing, firing and atomic Undo/Redo across two source variants'
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
