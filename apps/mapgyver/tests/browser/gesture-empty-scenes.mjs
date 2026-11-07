import assert from 'node:assert/strict';

export async function createEmptyGestureScenes(edit, screens) {
	const media = [];
	for (const kind of ['Composition', 'Matrix']) {
		const label = `Empty gesture ${kind}`;
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: kind, label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
		});
		media.push({ kind, label, reference });
	}
	const { screen, surface } = await screens({
		kind: 'CreateScreen',
		label: 'Empty gesture Screen'
	});
	await screens({ kind: 'RenameSurface', reference: surface, label: 'Empty gesture Surface' });
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: {
				kind: 'Spout',
				sender: `EmptyGesture-${screen}`,
				extent: { width: 400, height: 300 }
			}
		}
	});
	const original = (await screens({ kind: 'InspectSurface', reference: surface })).snapshot;
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...original.definition, source: null, calibration: 'Off' }
	});
	return { media, surface };
}

export async function verifyEmptyGestureScenes({ page, edit, screens, scene, qualification }) {
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
	const reports = [];
	async function navigate(label, phase) {
		const view = page.getByRole('application', { name: label, exact: true });
		await view.waitFor();
		const identity = await view.elementHandle();
		await view.getByRole('button', { name: 'Home view', exact: true }).click();
		await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await page.waitForFunction(
			(label) =>
				Number(
					document.querySelector(`[role="application"][aria-label="${label}"] output[data-zoom]`)
						?.dataset.zoom
				) === 1.2,
			label
		);
		await view.focus();
		await page.keyboard.press('Home');
		await page.waitForFunction(
			(label) =>
				document.querySelector(`[role="application"][aria-label="${label}"] output[data-zoom]`)
					?.dataset.zoom === '1',
			label
		);
		assert(
			await identity.evaluate((element) => element.isConnected),
			'Empty navigation preserves viewport identity'
		);
		reports.push({ phase, zoom_in: 1.2, keyboard_home: 1, same_viewport: true });
		await qualification?.after(phase);
	}
	for (const item of scene.media) {
		await tab('Medias').click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: item.label, exact: true })
			.click();
		if (item.kind === 'Composition') {
			const inspect = async () =>
				(await edit({ kind: 'Composition', reference: item.reference, edit: { kind: 'Inspect' } }))
					.snapshot;
			const before = await inspect();
			assert.equal(before.instances.length, 0);
			await tab('Composition').click();
			await navigate('Composition view', 'Empty Composition');
			assert.deepEqual(await inspect(), before, 'Empty Composition navigation never authors media');
		} else {
			const matrix = (operation) =>
				edit({ kind: 'Matrix', media: item.reference.media, edit: operation });
			const before = (await matrix({ kind: 'Inspect' })).snapshot;
			assert.equal(before.cells.length, 0);
			await tab('Matrix').click();
			const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
			await grid.getByTitle(`Select ${before.items[0].label}`, { exact: true }).click();
			await tab('Composition').click();
			await navigate('Matrix Layer layout view', 'Empty Matrix Layers');
			await page.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
			await navigate('Matrix Cell content view', 'Empty Matrix Cells');
			assert.deepEqual(
				(await matrix({ kind: 'Inspect' })).snapshot,
				before,
				'Empty navigation never materializes Cell addresses or edits Layers'
			);
		}
	}
	const before = (await screens({ kind: 'InspectSurface', reference: scene.surface })).snapshot;
	assert.equal(before.definition.source, null);
	await tab('Screens').click();
	const tree = page.locator('section[aria-label="Screens"]');
	const row = tree.getByRole('treeitem', { name: 'Empty gesture Screen', exact: true });
	await row.getByRole('button', { name: 'Empty gesture Screen', exact: true }).click();
	if ((await row.getAttribute('aria-expanded')) === 'false') {
		await row.focus();
		await row.press('ArrowRight');
	}
	await tree.getByRole('button', { name: 'Empty gesture Surface', exact: true }).click();
	await tab('Screen Warping').click();
	await navigate('Screen view', 'Empty Screen');
	assert.deepEqual(
		(await screens({ kind: 'InspectSurface', reference: scene.surface })).snapshot,
		before,
		'Source-less Screen navigation retains its authored Surface'
	);
	return reports;
}
