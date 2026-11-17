import assert from 'node:assert/strict';
import path from 'node:path';

/** Actual bundled grid; use its existing Cell source/preview rather than a synthetic renderer. */
export async function verifyMatrixAppearance({
	page,
	grid,
	cell,
	layer,
	column,
	matrix,
	post,
	reference,
	directory,
	wait
}) {
	const screens = async (payload) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	};
	// Cell thumbnails alone do not demand the final Matrix render. Observe real submission
	// through one owned offscreen output, as an ordinary running composition does.
	const { screen, surface } = await screens({ kind: 'CreateScreen', label: 'Appearance consumer' });
	const { snapshot } = await screens({ kind: 'InspectSurface', reference: surface });
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...snapshot.definition, source: reference }
	});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: {
				kind: 'Spout',
				sender: `MatrixAppearance-${screen}`,
				extent: { width: 96, height: 64 }
			}
		}
	});
	const panel = grid.locator('xpath=ancestor::section[1]');
	const previews = panel.locator('.native-preview');
	const sourceCounts = () =>
		panel
			.locator('[data-matrix-preview-source-count]')
			.getAttribute('data-matrix-preview-source-count');
	await matrix({ kind: 'ClearPlayback' });
	await cell.scrollIntoViewIfNeeded();
	await wait(
		() => cell.getAttribute('data-submitted'),
		(value) => value === 'false'
	);
	const before = await cell.boundingBox();
	assert(before);
	const layout = () =>
		cell.evaluate((node) => {
			const viewport = node.closest('[role="grid"]');
			if (!(viewport instanceof HTMLElement)) throw new Error('Matrix viewport missing');
			const item = node.getBoundingClientRect(),
				area = viewport.getBoundingClientRect();
			// Focus may scroll a native viewport. Compare authored placement within that viewport.
			return [
				item.x - area.x + viewport.scrollLeft,
				item.y - area.y + viewport.scrollTop,
				item.width,
				item.height
			].map((value) => Math.round(value * 1000) / 1000);
		});
	const originalLayout = await layout();
	const previewCount = await previews.count(),
		sources = await sourceCounts();
	await cell.dblclick({ position: { x: before.width * 0.8, y: before.height * 0.75 } });
	await wait(
		() => cell.getAttribute('data-submitted'),
		(value) => value === 'true'
	);
	await wait(
		() => cell.evaluate((node) => getComputedStyle(node).boxShadow),
		(value) => value !== 'none'
	);
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	assert.equal(
		await grid.locator('.cell[data-submitted="true"]').count(),
		1,
		'Only the launched Layer/Column address is active'
	);
	assert.deepEqual(
		await layout(),
		originalLayout,
		'Elevation keeps hit/preview geometry unchanged'
	);
	assert.equal(await previews.count(), previewCount);
	await grid.evaluate((node) => {
		node.scrollTop = 0;
		node.scrollLeft = 0;
	});
	await wait(sourceCounts, (value) => value === sources);
	const active = await cell.evaluate((node) => {
		const style = getComputedStyle(node);
		return {
			radius: style.borderRadius,
			shadow: style.boxShadow,
			transition: style.transitionDuration,
			border: style.borderColor,
			color: style.getPropertyValue('--matrix-item-color'),
			dpr: devicePixelRatio
		};
	});
	assert(parseFloat(active.radius) > 0);
	assert(active.color.includes('214'), 'Authored custom color survives active playback');
	await page.screenshot({ path: path.join(directory, 'matrix-appearance-active.png') });
	await grid
		.locator('[role="rowheader"][data-row-id]')
		.first()
		.locator('button[title^="Select "]')
		.click();
	assert.equal(await cell.getAttribute('aria-selected'), 'false');
	await wait(
		() => cell.getAttribute('data-submitted'),
		(value) => value === 'true'
	);
	assert.notEqual(await cell.evaluate((node) => getComputedStyle(node).boxShadow), 'none');
	await page.mouse.move(0, 0);
	await page.screenshot({ path: path.join(directory, 'matrix-appearance-active-unselected.png') });
	await cell.click({ position: { x: before.width * 0.8, y: before.height * 0.75 } });
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	await page.emulateMedia({ reducedMotion: 'reduce' });
	assert.equal(await cell.evaluate((node) => getComputedStyle(node).transitionDuration), '0s');
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	await panel
		.locator('[data-matrix-stop-all]')
		.getByRole('button', { name: 'Trigger Stop All', exact: true })
		.click();
	await wait(
		() => cell.getAttribute('data-submitted'),
		(value) => value === 'false'
	);
	await wait(
		() => cell.evaluate((node) => getComputedStyle(node).boxShadow),
		(value) => value === 'none'
	);
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	await page.mouse.move(0, 0);
	await page.screenshot({ path: path.join(directory, 'matrix-appearance-idle.png') });
	await screens({ kind: 'ConfigureScreen', screen, definition: { enabled: false, target: null } });
	return {
		result: 'pass',
		layer: layer.id.id,
		column: column.id,
		active,
		previews: previewCount,
		sourceGroups: sources,
		geometryStable: true,
		reducedMotion: true,
		normalStopRetiresElevation: true
	};
}
