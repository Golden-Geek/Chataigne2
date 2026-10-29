import assert from 'node:assert/strict';
import {
	QUALIFICATION_COMMAND_EVENT,
	QUALIFICATION_LAYOUT_EVENT,
	QUALIFICATION_REPORT_EVENT
} from '../native/generated/constants.ts';

export async function verifyQualification(browser, baseUrl) {
	const context = await browser.newContext({
		viewport: { width: 1400, height: 900 },
		deviceScaleFactor: 3
	});
	try {
		await context.addInitScript(
			({ reportEvent }) => {
				window.isTauri = true;
				const callbacks = new Map();
				const listeners = new Map();
				let callbackId = 0;
				let listenerId = 0;
				let views = [];
				let revision = 0;
				let geometry = '';
				let session = null;
				let connections = 0;
				window.__qualificationRequests = [];
				window.__qualificationReports = [];
				window.__qualificationExpire = () => {
					session = null;
					views = [];
					revision = 0;
					geometry = '';
				};
				window.__qualificationHostEvent = (event, payload) => {
					for (const [id, listener] of listeners) {
						if (listener.event === event) callbacks.get(listener.handler)?.({ event, id, payload });
					}
				};
				window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
					unregisterListener: (_event, id) => listeners.delete(id)
				};
				window.__TAURI_INTERNALS__ = {
					transformCallback(callback) {
						callbacks.set(++callbackId, callback);
						return callbackId;
					},
					unregisterCallback: (id) => callbacks.delete(id),
					invoke: async (command, args) => {
						if (command === 'plugin:event|listen') {
							listeners.set(++listenerId, args);
							return listenerId;
						}
						if (command === 'plugin:event|unlisten') return;
						if (command === 'plugin:event|emit') {
							if (args.event === reportEvent)
								window.__qualificationReports.push(structuredClone(args.payload));
							return;
						}
						if (command !== 'native_presentation_control') throw new Error(command);
						const request = JSON.parse(args.request);
						window.__qualificationRequests.push(request);
						if (request.kind === 'Connect') {
							session = `35537689-b168-461d-90a9-${String(++connections).padStart(12, '0')}`;
							views = [];
							revision = 0;
							geometry = '';
							return JSON.stringify({
								kind: 'Connected',
								session,
								version: 1,
								max_views: 64
							});
						}
						if (request.session !== session)
							return JSON.stringify({ kind: 'Rejected', code: 'Session' });
						if (request.kind === 'Disconnect') {
							session = null;
							return JSON.stringify({ kind: 'Disconnected' });
						}
						let installed = request.kind === 'Status';
						if (request.kind === 'Layout') {
							const next = JSON.stringify(request.views);
							installed = next === geometry;
							geometry = next;
							views = request.views;
							revision = request.revision;
						}
						return JSON.stringify({
							kind: 'Applied',
							revision,
							visible_views: views.filter((view) => view.visible).length,
							installed
						});
					}
				};
			},
			{ reportEvent: QUALIFICATION_REPORT_EVENT }
		);
		const page = await context.newPage();
		page.setDefaultTimeout(5000);
		const errors = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(`${baseUrl}?mode=qualification`);
		await page.waitForFunction(() =>
			window.__qualificationReports.some((report) => report.phase === 'listening')
		);
		assert.equal(await page.locator('[data-qualification-source]').count(), 0);
		const sources = ['opaque-source-a', 'opaque-source-b'];
		const initial = {
			revision: 1,
			views: [
				{ source: sources[0], cssWidth: 320, cssHeight: 180 },
				{ source: sources[1], cssWidth: 240, cssHeight: 135 }
			]
		};
		async function hostEvent(event, payload) {
			await page.evaluate(({ event, payload }) => window.__qualificationHostEvent(event, payload), {
				event,
				payload
			});
		}
		async function reportWhen(phase, commandId = null, revision = null) {
			await page.waitForFunction(
				({ phase, commandId, revision }) => {
					const report = window.__previewQualification;
					if (report?.phase === 'failed') throw new Error(report.error);
					return (
						report?.phase === phase &&
						report.commandId === commandId &&
						(revision === null || report.layout?.revision === revision)
					);
				},
				{ phase, commandId, revision },
				{ timeout: 5000 }
			);
			return page.evaluate(() => window.__previewQualification);
		}
		await hostEvent(QUALIFICATION_LAYOUT_EVENT, initial);
		const first = await reportWhen('ready', null, 1);
		assert(first.installed && first.retirementMasks === 0);
		assert.equal(first.visibleViews, 2);
		assert.deepEqual(
			first.views.map((view) => view.source),
			sources
		);
		assert.deepEqual(
			first.views.map((view) => [view.rect.width, view.rect.height]),
			[
				[320, 180],
				[240, 135]
			]
		);
		assert.equal(await page.evaluate(() => devicePixelRatio), 3);
		await page.waitForTimeout(1250);
		assert.equal(await page.locator('[data-qualification-source]').count(), 2);
		assert.equal(await page.evaluate(() => window.__previewQualification.sequence), first.sequence);
		assert.equal(
			await page.evaluate(() =>
				window.__qualificationRequests.some((item) => item.kind === 'Disconnect')
			),
			false,
			'the held fixture must not auto-dispose'
		);
		const resized = {
			revision: 2,
			views: [
				{ source: sources[0], cssWidth: 256, cssHeight: 144 },
				{ source: sources[1], cssWidth: 192, cssHeight: 108 }
			]
		};
		await hostEvent(QUALIFICATION_LAYOUT_EVENT, resized);
		const changed = await reportWhen('ready', null, 2);
		assert.deepEqual(
			changed.views.map((view) => view.id),
			first.views.map((view) => view.id)
		);
		assert.deepEqual(
			changed.views.map((view) => [view.rect.width, view.rect.height]),
			[
				[256, 144],
				[192, 108]
			]
		);
		await hostEvent(QUALIFICATION_LAYOUT_EVENT, initial);
		assert.equal(await page.evaluate(() => window.__previewQualification.layout.revision), 2);
		const overlays = await page.evaluate(() =>
			[...document.querySelectorAll('.preview')].map((element) => {
				const button = element.querySelector('button');
				const rect = button.getBoundingClientRect();
				return {
					preview: element.getBoundingClientRect().toJSON(),
					button: rect.toJSON(),
					target: document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
						?.outerHTML
				};
			})
		);
		assert(
			overlays.every((item) => item.target?.startsWith('<button')),
			JSON.stringify(overlays)
		);
		await page.getByRole('button', { name: 'Overlay 1', exact: true }).click();
		await page.getByRole('button', { name: 'Overlay 2', exact: true }).click();
		const connectionsBefore = await page.evaluate(
			() => window.__qualificationRequests.filter((request) => request.kind === 'Connect').length
		);
		await page.evaluate(() => window.__qualificationExpire());
		await hostEvent(QUALIFICATION_COMMAND_EVENT, { id: 1, action: 'ready' });
		const ready = await reportWhen('ready', 1);
		assert.deepEqual(ready.overlayClicks, [1, 1]);
		assert.deepEqual(
			ready.views.map((view) => view.id),
			first.views.map((view) => view.id)
		);
		assert.equal(
			await page.evaluate(
				() => window.__qualificationRequests.filter((request) => request.kind === 'Connect').length
			),
			connectionsBefore + 1,
			'ready must renegotiate an expired session before reporting installation'
		);
		const visibleCounts = [ready.visibleViews];
		for (const [id, action, phase, count] of [
			[2, 'hide', 'hidden', 0],
			[3, 'show', 'shown', 2],
			[4, 'disconnect', 'disconnected', 0]
		]) {
			await hostEvent(QUALIFICATION_COMMAND_EVENT, { id, action });
			const report = await reportWhen(phase, id);
			assert(report.installed && report.retirementMasks === 0);
			assert.equal(report.visibleViews, count);
			visibleCounts.push(report.visibleViews);
			assert.equal(await page.locator('[data-qualification-source]').count(), 2);
		}
		const final = await page.evaluate(() => window.__previewQualification);
		assert.equal(final.error, null);
		await page.getByRole('button', { name: 'Overlay 1', exact: true }).click();
		await page.getByRole('button', { name: 'Overlay 1', exact: true }).getByText('1: 2').waitFor();
		const requests = await page.evaluate(() => window.__qualificationRequests);
		assert.equal(requests.at(-1).kind, 'Disconnect');
		assert(final.maxRetirementMasks > 0 && final.maxRetirementMasks <= 2);
		assert.deepEqual(errors, []);

		const queryPage = await context.newPage();
		await queryPage.goto(
			`${baseUrl}?mode=qualification&source1=query-a&source2=query-b&width1=96&height1=54&width2=80&height2=45`
		);
		await queryPage.waitForFunction(() => window.__previewQualification?.phase === 'ready');
		const queryReport = await queryPage.evaluate(() => window.__previewQualification);
		assert.deepEqual(
			queryReport.views.map((view) => [view.source, view.rect.width, view.rect.height]),
			[
				['query-a', 96, 54],
				['query-b', 80, 45]
			]
		);
		await queryPage.evaluate(
			({ event, layout }) => window.__qualificationHostEvent(event, layout),
			{
				event: QUALIFICATION_LAYOUT_EVENT,
				layout: { ...initial, views: [...initial.views, initial.views[0]] }
			}
		);
		await queryPage.waitForFunction(() => window.__previewQualification?.phase === 'failed');
		assert.match(
			await queryPage.evaluate(() => window.__previewQualification.error),
			/exactly two views/
		);
		assert.equal(await queryPage.locator('[data-qualification-source]').count(), 2);
		return {
			sourceKeys: sources,
			visibleCounts,
			hostCssSizes: resized.views.map((view) => [view.cssWidth, view.cssHeight]),
			browserDeviceScale: 3,
			metadataRequests: requests.length,
			retirementMaskPeak: final.maxRetirementMasks,
			queryBootstrap: true,
			expiredSessionReadyRecovery: true,
			invalidLayoutRejected: true,
			pageErrors: errors
		};
	} finally {
		await context.close();
	}
}
