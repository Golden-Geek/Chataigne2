import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';

// Actual input in the ordinary native WebView; animation samples detect unrelated row re-entry.
export async function verifyInspectorStability({ page, edit, post, observe = async () => {} }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Inspector stability'
	});
	const { reference: peer } = await edit({
		kind: 'AddChannel',
		media: reference.media,
		label: 'Edited sibling'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
	});
	await page.getByRole('tab').first().click();
	await page.getByRole('button', { name: 'Inspector stability', exact: true }).click();
	const document = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	// Keep the unchanged sibling above the edited folder, so collapse does not move
	// its native pixel witness. Both branches still contain actual Parameter controls.
	const channel = document.nodes.find((node) => node.uuid === peer.channel);
	const sibling = document.nodes.find((node) => node.uuid === reference.channel);
	assert(channel && sibling);
	const root = page.locator(`.inspector .node-inspector[data-node-id="${channel.node_id}"]`);
	const siblingRoot = page.locator(`.inspector .node-inspector[data-node-id="${sibling.node_id}"]`);
	await root.locator('.node-title').first().waitFor();
	if ((await siblingRoot.locator('.node-title').first().getAttribute('aria-checked')) === 'false') {
		await siblingRoot.locator('.node-title').first().locator('svg.arrow').click();
	}
	await siblingRoot.locator('.parameter-inspector').first().waitFor();
	if ((await root.locator('.node-title').first().getAttribute('aria-checked')) === 'false') {
		await root.locator('.node-title').first().locator('svg.arrow').click();
	}
	const width = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Resolution', { exact: true }) })
		.locator('input.number-field')
		.first();
	await width.waitFor();
	await delay(300);
	await observe('ready', {
		uiBoxes: await Promise.all(
			(await siblingRoot.locator('.parameter-inspector').all()).map((field) => field.boundingBox())
		)
	});
	await page.evaluate(
		({ sibling }) => {
			const selector = `.inspector .node-inspector[data-node-id="${sibling}"]`;
			const original = document.querySelector(selector);
			if (!original) throw new Error('Missing Inspector sibling');
			const fields = [...original.querySelectorAll('.parameter-inspector')];
			if (!fields.length) throw new Error('Stability witness requires mounted sibling Parameters');
			window.__inspectorStability = {
				selector,
				original,
				fields,
				samples: [],
				removed: 0,
				actions: []
			};
			const observer = new MutationObserver((records) => {
				for (const record of records)
					for (const removed of record.removedNodes) {
						if (
							removed === original ||
							removed.contains?.(original) ||
							fields.some((field) => removed === field || removed.contains?.(field))
						)
							window.__inspectorStability.removed++;
					}
			});
			observer.observe(document.querySelector('.inspector'), { childList: true, subtree: true });
			window.__inspectorStability.observer = observer;
		},
		{ sibling: sibling.node_id }
	);
	async function sample(action, input) {
		await page.evaluate((action) => {
			const state = window.__inspectorStability;
			state.actions.push(action);
			state.sampling = true;
			const until = performance.now() + 350;
			function frame() {
				const row = document.querySelector(state.selector);
				const elements = row
					? [
							row,
							...row.querySelectorAll(
								'.node-inspector-children-wrapper,.node-inspector-custom-content-wrapper'
							)
						]
					: [];
				state.samples.push({
					action,
					connected: state.original.isConnected,
					sameRow: row === state.original,
					fieldsConnected: state.fields.every((field) => field.isConnected),
					opacity: elements.length
						? Math.min(...elements.map((element) => Number(getComputedStyle(element).opacity)))
						: 0
				});
				if (performance.now() < until) requestAnimationFrame(frame);
				else state.sampling = false;
			}
			requestAnimationFrame(frame);
		}, action);
		await input();
		await page.waitForFunction(() => !window.__inspectorStability.sampling);
	}
	for (let index = 0; index < 3; index++) {
		await sample(`collapse-${index}`, () =>
			root.locator('.node-title').first().locator('svg.arrow').click()
		);
		assert.equal(await root.locator('.node-title').first().getAttribute('aria-checked'), 'false');
		await sample(`expand-${index}`, () =>
			root.locator('.node-title').first().locator('svg.arrow').click()
		);
		assert.equal(await root.locator('.node-title').first().getAttribute('aria-checked'), 'true');
		await sample(`setting-width-32-${index}`, async () => {
			await width.fill('32');
			await width.press('Enter');
		});
		await sample(`setting-width-64-${index}`, async () => {
			await width.fill('64');
			await width.press('Enter');
		});
	}
	const result = await page.evaluate(() => {
		const state = window.__inspectorStability;
		state.observer.disconnect();
		return {
			actions: state.actions.length,
			frames: state.samples.length,
			removed: state.removed,
			parameterWitnesses: state.fields.length,
			disconnectedFrames: state.samples.filter(
				(frame) => !frame.connected || !frame.sameRow || !frame.fieldsConnected
			).length,
			fadedFrames: state.samples.filter((frame) => frame.opacity < 0.99).length,
			minimumOpacity: Math.min(...state.samples.map((frame) => frame.opacity))
		};
	});
	console.log(
		JSON.stringify({
			scope: 'native Inspector sibling stability during real folder/control input',
			...result
		})
	);
	assert.equal(result.removed, 0, 'unrelated Inspector fields must remain mounted');
	assert.equal(result.disconnectedFrames, 0, 'unrelated Inspector fields must stay connected');
	assert.equal(
		result.fadedFrames,
		0,
		'unrelated Inspector content must not re-enter its slide animation'
	);
	await observe('complete', result);
	return result;
}
