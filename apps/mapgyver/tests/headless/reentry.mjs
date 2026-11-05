// Ordinary-host re-entry acceptance through the same typed intents as UI and scripts.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyApplicationReentry({ edit, page = null, post = null }) {
	const directory = path.resolve(process.env.MAPGYVER_REENTRY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const application = (operation) => edit({ kind: 'Applications', edit: operation });
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Zero pre-post re-entry'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const authored = (await matrix({ kind: 'Inspect' })).snapshot;
	const [active, empty] = authored.columns;
	const { snapshot: initial } = await application({ kind: 'Inspect', application: active.id });
	const definition = {
		...initial.definition,
		executable: path.resolve('target/debug/application_process_fixture.exe')
	};
	if (page || process.env.MAPGYVER_REENTRY_SPOUT === '1') {
		const sender = `MapGyverReentry-${reference.media}`;
		definition.arguments = [sender];
		definition.readiness = { kind: 'Spout', sender, first_frame: true, timeout_ms: 5000 };
	}
	await matrix({
		kind: 'ConfigureColumnApplication',
		column: active.id,
		definition,
		pre: 0,
		post: 0
	});
	const configured = (await application({ kind: 'Inspect', application: active.id })).snapshot;
	assert.deepEqual(configured.definition, definition);
	let grid;
	if (page) {
		assert(post);
		await matrix({
			kind: 'CreateCellMedia',
			layer: authored.items[0].id.id,
			column: active.id,
			media_kind: 'SharedTexture'
		});
		const cell = (await matrix({ kind: 'Inspect' })).snapshot.cells[0];
		await edit({
			kind: 'ConfigureChannel',
			reference: cell.definition.source,
			source: { kind: 'Spout', sender: definition.readiness.sender }
		});
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Zero pre-post re-entry', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const screen = async (payload) => {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.screens',
				operation: 'edit',
				payload
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value;
		};
		const { screen: output, surface } = await screen({
			kind: 'CreateScreen',
			label: 'Re-entry output'
		});
		const { snapshot } = await screen({ kind: 'InspectSurface', reference: surface });
		await screen({
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...snapshot.definition, source: reference }
		});
		await screen({
			kind: 'ConfigureScreen',
			screen: output,
			definition: {
				enabled: true,
				target: {
					kind: 'Spout',
					sender: `MapGyverReentryOutput-${output}`,
					extent: { width: 96, height: 64 }
				}
			}
		});
	}
	const launch = async (column) => {
		if (grid)
			await grid.getByRole('button', { name: `Launch ${column.label}`, exact: true }).click();
		else await matrix({ kind: 'LaunchColumn', column: column.id, mode: 'Configured' });
	};
	const read = async () => (await application({ kind: 'InspectRuntime' })).snapshot;
	const cycles = [];
	const rapid = [];
	const statuses = [];
	async function wait(phase, predicate) {
		const deadline = Date.now() + 12000;
		let previous;
		while (true) {
			const snapshot = await read();
			assert.equal(snapshot.callbacks.fault, null);
			const state = JSON.stringify(snapshot.applications);
			if (state !== previous) statuses.push({ phase, snapshot });
			previous = state;
			if (predicate(snapshot.applications)) return snapshot;
			assert(Date.now() < deadline, `${phase}: ${JSON.stringify(snapshot)}`);
			await delay(25);
		}
	}
	try {
		for (let cycle = 1; cycle <= 20; cycle++) {
			await launch(active);
			const ready = await wait(`ready-${cycle}`, (entries) =>
				entries.some(
					(entry) =>
						entry.consumers.some((consumer) => consumer.application === active.id) &&
						entry.phase === 'Ready'
				)
			);
			const owner = ready.applications.find((entry) =>
				entry.consumers.some((consumer) => consumer.application === active.id)
			);
			assert(owner.process_id && owner.launch);
			assert(
				!cycles.some((previous) => previous.launch === owner.launch),
				'each retired activation gets a fresh launch'
			);
			if (grid) await grid.locator(`[data-column-id="${active.id}"].heading.submitted`).waitFor();
			await launch(empty);
			await wait(`idle-${cycle}`, (entries) =>
				entries.every((entry) => entry.phase === 'Idle' && entry.process_id === null)
			);
			cycles.push({ cycle, runtime: owner.runtime, launch: owner.launch, pid: owner.process_id });
			console.log(JSON.stringify({ reentryCycle: cycle, result: 'pass', pid: owner.process_id }));
		}
		for (let cycle = 1; cycle <= 5; cycle++) {
			const belongs = (entry) =>
				entry.consumers.some((consumer) => consumer.application === active.id);
			await launch(active);
			const ready = await wait(`rapid-ready-${cycle}`, (entries) =>
				entries.some((entry) => belongs(entry) && entry.phase === 'Ready')
			);
			const first = ready.applications.find(belongs);
			await launch(empty);
			await wait(`rapid-stopping-${cycle}`, (entries) =>
				entries.some((entry) => entry.process_id === first.process_id && entry.phase === 'Stopping')
			);
			await launch(active);
			const reentered = await wait(`rapid-reentered-${cycle}`, (entries) =>
				entries.some(
					(entry) => belongs(entry) && entry.phase === 'Ready' && entry.launch !== first.launch
				)
			);
			const second = reentered.applications.find(belongs);
			assert.notEqual(
				second.process_id,
				first.process_id,
				'a stopping owner retires before its replacement starts'
			);
			await launch(empty);
			await wait(`rapid-idle-${cycle}`, (entries) =>
				entries.every((entry) => entry.phase === 'Idle' && entry.process_id === null)
			);
			rapid.push({ cycle, first: first.launch, replacement: second.launch });
			console.log(JSON.stringify({ rapidReentry: cycle, result: 'pass' }));
		}
		const report = {
			result: 'pass',
			scope:
				'Twenty configured-transition Column cycles plus five re-entries during observed Stopping, pre=post=0, default stop/restart settings',
			definition,
			nativeRendered: !!page,
			configured,
			cycles,
			rapid,
			statuses
		};
		await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
		console.log(
			JSON.stringify({
				result: 'pass',
				scope: report.scope,
				cycles: cycles.length,
				rapid: rapid.length,
				report: path.join(directory, 'report.json')
			})
		);
	} catch (error) {
		await writeFile(
			path.join(directory, 'failure.json'),
			JSON.stringify(
				{ definition, configured, cycles, rapid, statuses, error: String(error) },
				null,
				2
			)
		);
		throw error;
	} finally {
		await matrix({ kind: 'LaunchColumn', column: empty.id, mode: 'Configured' });
	}
}
