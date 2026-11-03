import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyApplicationCommandUi({
	page,
	edit,
	intent,
	parameter,
	background,
	commands
}) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'App command target'
	});
	const { snapshot } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'Inspect' }
	});
	const owner = snapshot.columns[0].application;
	const read = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: owner } })).snapshot;
	async function waitFile(executable) {
		const deadline = Date.now() + 5000;
		while ((await read()).definition.executable !== executable) {
			assert(Date.now() < deadline, 'App command executable projection timeout');
			await delay(20);
		}
	}
	const bounds = await background.boundingBox();
	await background.click({ position: { x: bounds.width / 2, y: bounds.height - 5 } });
	await commands.getByRole('button', { name: 'Add child item', exact: true }).click();
	await page.getByRole('menuitem', { name: 'App control', exact: true }).hover();
	for (const name of [
		'Set App File',
		'Launch Matrix Column',
		'Launch Matrix Cell',
		'Unload Matrix',
		'Inspect App Control',
		'Inspect App Runtime',
		'Inspect App Callbacks'
	]) {
		await page.getByRole('menuitem', { name, exact: true }).waitFor();
	}
	await page.getByRole('menuitem', { name: 'Set App File', exact: true }).click();
	await parameter('App control owner').waitFor();
	assert.equal(await parameter('Launch mode').count(), 0);
	assert.equal(await parameter('Media').count(), 0);
	const node = Number(await parameter('App control owner').getAttribute('data-node-id'));
	const ack = await intent({
		kind: 'setParam',
		node,
		value: { Reference: { uuid: owner } },
		behaviour: 'Append'
	});
	assert(ack.success, JSON.stringify(ack));
	const file = path.resolve('target/debug/application_process_fixture.exe');
	const input = parameter('App file').locator('input.path-input');
	await input.fill(file);
	await input.press('Tab');
	await waitFile(null);
	await parameter('Execute').locator('button.trigger').click();
	await waitFile(file);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await waitFile(null);
	assert.equal(await input.inputValue(), file, 'Undo keeps the command configuration');
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await waitFile(file);
	const runtime = (await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } })).snapshot;
	assert(runtime.applications.every((application) => application.process_id === null));
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'App control command menu, typed owner/File Inspector, passive edits, Execute and exact Undo/Redo without launching the owner'
		})
	);
}
