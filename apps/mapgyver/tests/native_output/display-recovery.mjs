import assert from 'node:assert/strict';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

async function waitFor(description, predicate) {
	const deadline = performance.now() + 12000;
	for (;;) {
		const result = await predicate();
		if (result) return result;
		assert(performance.now() < deadline, description);
		await delay(120);
	}
}

export async function verifyDisplayRecovery({
	output,
	directory,
	screen,
	color,
	edit,
	post,
	displayOptions
}) {
	const option = async () =>
		(await displayOptions()).find(({ variant_id }) => output.matchesIdentity(variant_id));
	const online = () =>
		waitFor(`${output.name} never appeared in the Screen Inspector`, async () => {
			const entry = await option();
			return entry && !entry.label.endsWith(' (offline)') && entry.variant_id;
		});
	const display = await online();
	const target = { kind: 'Fullscreen', display };
	await edit('screens', { kind: 'ConfigureScreen', screen, definition: { enabled: true, target } });

	async function expectAuthoredTarget() {
		const { definition } = await edit('screens', { kind: 'InspectScreen', screen });
		assert.equal(definition.enabled, true);
		assert.deepEqual(definition.target, target);
	}
	async function expectOutput(rgb) {
		await waitFor(`${output.name} never showed RGB ${rgb}`, async () => {
			const windows = await output.ownedWindows();
			if (windows.length !== 1) return false;
			const pixels = await output.pixels(windows[0]);
			return (
				pixels.length > 0 &&
				pixels.every((pixel) => pixel.every((value, index) => Math.abs(value - rgb[index]) < 16))
			);
		});
	}
	async function setActive(active) {
		await output.setActive(active);
		await waitFor(
			`${output.name} did not become ${active ? 'active' : 'inactive'}`,
			async () => (await output.isActive()) === active
		);
	}
	async function expectOffline() {
		await waitFor('Saved display never went offline or its window remained', async () => {
			const entry = await option();
			return (
				entry?.variant_id === display &&
				entry.label.endsWith(' (offline)') &&
				(await output.ownedWindows()).length === 0
			);
		});
		await expectAuthoredTarget();
	}

	await expectOutput([255, 0, 0]);
	const project = join(directory, 'display-recovery.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	await setActive(false);
	await expectOffline();
	await color([0, 1, 0, 1]);
	await setActive(true);
	assert.equal(await online(), display);
	await expectAuthoredTarget();
	await expectOutput([0, 255, 0]);

	await setActive(false);
	await expectOffline();
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	assert.equal((await post('/api/ui/project-load', { path: project })).path, project);
	await expectOffline();
	await setActive(true);
	assert.equal(await online(), display);
	await expectAuthoredTarget();
	await expectOutput([255, 0, 0]);
	console.log(
		`${output.name} recovered the saved Screen after disconnect and reload while offline`
	);
}
