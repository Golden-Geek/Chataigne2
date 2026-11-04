import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixFeedback({ page, post, edit }) {
	let stage = 'prepare actual Matrix consumer';
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	async function wait(read, predicate) {
		const deadline = Date.now() + 10000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	async function screens(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	}
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Native Matrix feedback'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	const initial = await inspect();
	for (const row of initial.items) {
		await matrix({
			kind: 'ConfigureLayer',
			layer: row.id.id,
			settings: { ...row.layer, duration_ms: 1500 }
		});
		for (const column of initial.columns.slice(0, 2))
			await matrix({
				kind: 'CreateCellMedia',
				layer: row.id.id,
				column: column.id,
				media_kind: 'SolidColor'
			});
	}
	const { screen, surface } = await screens({
		kind: 'CreateScreen',
		label: 'Matrix feedback consumer'
	});
	const target = {
		kind: 'Spout',
		sender: `MatrixFeedback-${screen}`,
		extent: { width: 96, height: 64 }
	};
	const { snapshot: originalSurface } = await screens({
		kind: 'InspectSurface',
		reference: surface
	});
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...originalSurface.definition, source: reference }
	});
	await screens({ kind: 'ConfigureScreen', screen, definition: { enabled: true, target } });
	try {
		await page.reload();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Native Matrix feedback', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const column = (index) =>
			grid.locator(`[role="columnheader"][data-column-id="${initial.columns[index].id}"]`);
		const cell = (row, col) =>
			grid.getByTitle(`${initial.items[row].label} / ${initial.columns[col].label}`, {
				exact: true
			});
		await grid.getByTitle('Select Layer 1', { exact: true }).click();
		const feedback = page.getByRole('region', { name: 'Matrix runtime feedback', exact: true });
		stage = 'actual native installation before any launch';
		await wait(
			() => feedback.innerText(),
			(text) => text.includes('Renderer:') && !text.includes('No native Matrix observation.')
		);
		assert.equal((await playback()).desired_column, null);
		stage = 'Column launch becomes observed steady submission';
		const authored = await inspect();
		await grid.getByRole('button', { name: 'Launch Column 1', exact: true }).click();
		await wait(playback, (state) => state.desired_column === initial.columns[0].id);
		await wait(
			() => feedback.innerText(),
			(text) => text.includes('Frame submitted for Column 1.') && text.includes('Steady plan')
		);
		assert(
			await column(0).evaluate(
				(element) =>
					element.classList.contains('requested') && element.classList.contains('submitted')
			)
		);
		await wait(
			() => cell(0, 0).getAttribute('data-submitted'),
			(value) => value === 'true'
		);
		const steady = await playback();
		stage = 'selection stays independent of requested and submitted Columns';
		await grid.getByTitle('Select Column 2', { exact: true }).click();
		assert.equal((await playback()).request_id, steady.request_id);
		assert.equal(
			await column(1).getByTitle('Select Column 2', { exact: true }).getAttribute('aria-pressed'),
			'true'
		);
		assert.equal(
			await column(1).evaluate((element) => element.classList.contains('requested')),
			false
		);
		assert.equal(await cell(0, 0).getAttribute('data-requested'), 'true');
		stage = 'independent Cell request and actual interrupted mix';
		await cell(0, 1).dblclick({ position: { x: 5, y: 5 } });
		await wait(playback, (state) =>
			state.layers.some(
				(layer) => layer.layer === initial.items[0].id.id && layer.column === initial.columns[1].id
			)
		);
		await wait(
			() => cell(0, 1).getAttribute('data-requested'),
			(value) => value === 'true'
		);
		assert.equal(await cell(1, 0).getAttribute('data-requested'), 'true');
		assert.equal((await playback()).desired_column, initial.columns[0].id);
		await wait(
			() => cell(0, 1).getAttribute('data-submitted'),
			(value) => value === 'true'
		);
		assert.equal(await cell(1, 1).getAttribute('data-submitted'), 'false');
		await grid.getByRole('button', { name: 'Launch Column 1', exact: true }).click();
		await grid.getByTitle('Select Layer 1', { exact: true }).click();
		await wait(
			() => feedback.innerText(),
			(text) => text.includes('Steady plan') && !text.includes('Waiting for the renderer')
		);
		await wait(
			() => cell(0, 1).getAttribute('data-submitted'),
			(value) => value === 'false'
		);
		assert.deepEqual(await inspect(), authored, 'runtime input does not author history');
		await page.screenshot({ path: 'target/m07-native-grid-feedback.png' });
		for (const index of [0, 1, 2]) {
			const heading = await column(index).boundingBox();
			for (const marker of await column(index).getByRole('img').all()) {
				const bounds = await marker.boundingBox();
				assert(
					bounds.y >= heading.y && bounds.y + bounds.height <= heading.y + heading.height,
					'native status markers fit within the header'
				);
			}
		}
		stage = 'retire actual renderer consumer';
		await screens({ kind: 'ConfigureScreen', screen, definition: { enabled: false, target } });
		await wait(
			() => feedback.innerText(),
			(text) => text.includes('No native Matrix observation.')
		);
		assert.equal(
			await column(0).evaluate((element) => element.classList.contains('submitted')),
			false
		);
		assert.equal(await cell(0, 0).getAttribute('data-submitted'), 'false');
		assert.equal(await cell(0, 0).getAttribute('data-requested'), 'true');
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Actual native Column/Cell requests, independent selection/anchor/Layer state, interrupted submitted mix, steady completion, unchanged authoring and consumer-retirement clearing',
				columns: initial.columns.map((column) => column.id),
				pageErrors: errors,
				rejected
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-native-grid-feedback-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				playback: await playback(),
				body: (await page.locator('body').innerText()).slice(-7000)
			})
		);
		throw error;
	}
}
