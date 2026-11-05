import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { inventory, source } from './native-sender-selection.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function verifyNativeApplicationEndpoint({ page, edit, post }) {
	const directory = path.resolve(process.env.MAPGYVER_REENTRY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Endpoint recovery source'
	});
	const sender = `MapGyverEndpoint-${reference.media}`;
	await edit({ kind: 'ConfigureChannel', reference, source: { kind: 'Spout', sender } });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Endpoint recovery source', exact: true })
		.click();
	const adapter = await observeNativeAdapter(page, await inventory());
	const endpoint = await source(adapter.index, sender);
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	const { reference: matrixMedia } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Endpoint lifetime'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: matrixMedia.media, edit: operation });
	const apps = (operation) => edit({ kind: 'Applications', edit: operation });
	const [active, empty] = (await matrix({ kind: 'Inspect' })).snapshot.columns;
	const { definition } = (await apps({ kind: 'Inspect', application: active.id })).snapshot;
	await matrix({
		kind: 'ConfigureColumnApplication',
		column: active.id,
		pre: 0,
		post: 0,
		definition: {
			...definition,
			executable: path.resolve('target/debug/application_process_fixture.exe'),
			readiness: { kind: 'Spout', sender, first_frame: true, timeout_ms: 5000 }
		}
	});
	const owner = async () =>
		(await apps({ kind: 'InspectRuntime' })).snapshot.applications.find((entry) =>
			entry.consumers.some((consumer) => consumer.application === active.id)
		);
	async function ready() {
		const deadline = Date.now() + 8000;
		while (true) {
			const entry = await owner();
			if (entry?.phase === 'Ready') return entry;
			assert(Date.now() < deadline, JSON.stringify(entry));
			await delay(25);
		}
	}
	const preview = page.locator(
		'.media-preview-panel[data-panel-id="media-preview"] .native-preview'
	);
	try {
		await endpoint.command('start 64 64 1 0 0');
		await matrix({ kind: 'LaunchColumn', column: active.id, mode: 'Configured' });
		const first = await ready();
		await preview
			.locator('xpath=self::*[@data-preview-state="available"][@data-preview-installed="true"]')
			.waitFor();
		await output.demand(reference);
		await delay(250);
		await output.observePixels('before-loss', [255, 0, 0, 255]);
		await endpoint.command('stop');
		await preview.locator('.message').waitFor();
		const lostMessage = await preview.locator('.message').innerText();
		assert(lostMessage.trim(), 'endpoint loss has explicit source feedback');
		const lost = await owner();
		assert.equal(lost.process_id, first.process_id);
		assert.equal(
			lost.phase,
			'Ready',
			'startup readiness is separate from current endpoint availability'
		);
		await endpoint.command('start 64 64 0 1 0');
		await preview.locator('.message').waitFor({ state: 'hidden' });
		await preview
			.locator('xpath=self::*[@data-preview-state="available"][@data-preview-installed="true"]')
			.waitFor();
		await delay(250);
		await output.observePixels('after-recovery', [0, 255, 0, 255]);
		const recovered = await ready();
		assert.equal(recovered.process_id, first.process_id);
		assert.equal(recovered.launch, first.launch);
		const report = {
			result: 'pass',
			adapter,
			first,
			lost,
			lostMessage,
			recovered,
			observations: output.observations,
			limits:
				'An independently controlled, owned endpoint is removed/restored while the configured application process stays alive; no readiness injection, timing or cadence claim.'
		};
		await writeFile(path.join(directory, 'endpoint.json'), JSON.stringify(report, null, 2));
		console.log(
			JSON.stringify({ result: 'pass', scope: report.limits, pid: first.process_id, adapter })
		);
	} finally {
		await matrix({ kind: 'LaunchColumn', column: empty.id, mode: 'Configured' });
		await output.finish();
		await endpoint.close();
	}
}
