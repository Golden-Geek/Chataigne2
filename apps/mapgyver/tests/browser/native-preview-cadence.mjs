// Bounded production-host window trace using the pinned existing PresentMon collector.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyNativePreviewCadence({ page, edit, post, nativePid, nativeLog }) {
	const directory = path.resolve(process.env.MAPGYVER_PREVIEW_CADENCE_DIRECTORY);
	await mkdir(directory);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Live preview cadence'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'Spout', sender: 'Spout Demo Sender' }
	});
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
	const { screen, surface } = await screens({ kind: 'CreateScreen', label: 'Cadence output' });
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: {
			source: reference,
			enabled: true,
			opacity: 1,
			calibration: 'Off',
			crop: { x: 0, y: 0, width: 1, height: 1 },
			quad: {
				corners: [
					{ x: 0, y: 0 },
					{ x: 1, y: 0 },
					{ x: 1, y: 1 },
					{ x: 0, y: 1 }
				]
			}
		}
	});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: {
				kind: 'Spout',
				sender: `PreviewCadence-${nativePid}`,
				extent: { width: 1920, height: 1080 }
			}
		}
	});
	await page.getByRole('tab').first().click();
	await page.getByRole('button', { name: 'Live preview cadence', exact: true }).click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview$/ })
		.click();
	const view = page.locator('.media-preview-panel .native-preview').first();
	await page.waitForFunction(() => {
		const view = document.querySelector('.media-preview-panel .native-preview');
		return view?.dataset.previewState === 'available' && view.dataset.previewInstalled === 'true';
	});
	if (process.env.MAPGYVER_PREVIEW_CADENCE_PIXELS === '1') {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		await page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: 'Cadence output', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping(?::|$)/ })
			.click();
		const screenView = page.locator('[data-panel-id="screen-warp"] .native-preview').first();
		await page.waitForFunction(() => {
			const view = document.querySelector('[data-panel-id="screen-warp"] .native-preview');
			return view?.dataset.previewState === 'available' && view.dataset.previewInstalled === 'true';
		});
		const phases =
			process.env.MAPGYVER_PREVIEW_CADENCE_REPAIRED === '1'
				? ['screen-warp', 'media-preview']
				: ['normal', 'webview-repaint'];
		const pixels = [];
		for (const phase of phases) {
			if (phase === 'media-preview') {
				await page
					.getByRole('tab')
					.filter({ hasText: /^Medias$/ })
					.click();
				await page.getByRole('button', { name: 'Live preview cadence', exact: true }).click();
				await page
					.getByRole('tab')
					.filter({ hasText: /^Media Preview$/ })
					.click();
				await page.waitForFunction(() => {
					const view = document.querySelector('.media-preview-panel .native-preview');
					return (
						view?.dataset.previewState === 'available' && view.dataset.previewInstalled === 'true'
					);
				});
			}
			if (phase === 'webview-repaint')
				await page.evaluate(() => {
					const marker = document.createElement('div');
					marker.style.cssText =
						'position:fixed;left:0;top:0;width:1px;height:1px;pointer-events:none;z-index:99999';
					document.body.append(marker);
					let frame = 0;
					const repaint = () => {
						marker.style.background = ++frame % 2 ? 'white' : 'black';
						requestAnimationFrame(repaint);
					};
					requestAnimationFrame(repaint);
				});
			const target = path.join(directory, phase);
			await mkdir(target);
			await writeFile(
				path.join(target, 'ready.json'),
				JSON.stringify({
					pid: nativePid,
					box: await (phase === 'media-preview' ? view : screenView).boundingBox(),
					dpr: await page.evaluate(() => devicePixelRatio)
				})
			);
			const child = spawn(
				'python',
				['apps/mapgyver/tests/native/preview_cadence_frames.py', target],
				{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 20000 }
			);
			let output = '',
				error = '';
			child.stdout.on('data', (chunk) => (output += chunk));
			child.stderr.on('data', (chunk) => (error += chunk));
			const code = await new Promise((resolve, reject) => {
				child.once('error', reject);
				child.once('close', resolve);
			});
			console.log(JSON.stringify({ phase, code, output, error }));
			await writeFile(path.join(target, 'native.log'), nativeLog());
			assert.equal(code, 0, error);
			pixels.push({ phase, ...JSON.parse(output) });
		}
		if (process.env.MAPGYVER_PREVIEW_CADENCE_REPAIRED === '1') {
			for (const observation of pixels)
				assert(
					observation.observed_changes_per_second >= 8,
					`${observation.phase}: visible motion remains slow (${observation.observed_changes_per_second}/s)`
				);
		}
		return;
	}
	const status = () =>
		page.evaluate(async () =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
				})
			)
		);
	await delay(1000);
	const before = await status();
	const session = `MapGyverPreviewCadence-${nativePid}`;
	const collector = spawn(
		path.resolve('target/qualification-tools/presentmon-2.6.0/PresentMon-2.6.0-x64.exe'),
		[
			'--process_id',
			String(nativePid),
			'--session_name',
			session,
			'--output_file',
			path.join(directory, 'window.csv'),
			'--v1_metrics',
			'--qpc_time',
			'--no_console_stats',
			'--no_track_input',
			'--timed',
			'8',
			'--terminate_after_timed'
		],
		{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 20000 }
	);
	let stdout = '',
		stderr = '';
	collector.stdout.on('data', (chunk) => {
		stdout += chunk;
	});
	collector.stderr.on('data', (chunk) => {
		stderr += chunk;
	});
	const code = await new Promise((resolve, reject) => {
		collector.once('error', reject);
		collector.once('close', resolve);
	});
	const result = {
		nativePid,
		session,
		collectorExit: code,
		stdout,
		stderr,
		before,
		after: await status(),
		box: await view.boundingBox(),
		dpr: await page.evaluate(() => window.devicePixelRatio),
		limits: 'Window Present trace; moving pixel cadence requires separate verification.'
	};
	await writeFile(path.join(directory, 'window.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
	assert.equal(code, 0, stderr);
}
