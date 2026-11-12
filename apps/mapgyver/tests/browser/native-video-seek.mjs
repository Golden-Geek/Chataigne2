// Owned-app correctness check for the reported intraframe seek regression.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { inspectLocalVideoLibraries } from './video-runtime-libraries.mjs';

const directory = path.resolve(process.argv[2]);
const file = path.resolve(process.argv[3] ?? 'D:/Documents/MapGyver/test videos/ball_turn.mp4');
const selected = process.env.MAPGYVER_VIDEO_ADAPTER;
assert(selected, 'select an observed runtime GPU');
await mkdir(directory, { recursive: true });
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const preferences = path.join(root, 'MapGyver');
		await mkdir(preferences, { recursive: true });
		execFileSync(
			path.resolve('target/debug/native_qualification.exe'),
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(preferences, 'preferences.json'),
				'--selected',
				selected
			],
			{ windowsHide: true }
		);
	}
});
const { page } = native;
const pageErrors = [],
	observations = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
let token, reference, report;
async function post(endpoint, body) {
	const response = await fetch(`${native.base}${endpoint}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function edit(payload, namespace = 'media') {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
const control = (request) =>
	page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
async function wait(read, accept, label) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
async function state() {
	return {
		transport: (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot,
		source: (await edit({ kind: 'InspectVideoSource', reference })).status,
		frame: (await edit({ kind: 'InspectVideoFrame', reference })).frame
	};
}
try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	const runtime = await wait(
		() => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) => value.state === 'Running' && value.gpu_adapter,
		'renderer'
	);
	assert.equal(runtime.gpu_adapter, selected);
	const libraries = await inspectLocalVideoLibraries(
		native.app.pid,
		process.env.MAPGYVER_TEST_BINARY
	);
	({ reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'Intraframe seek'
	}));
	await edit({ kind: 'ConfigureVideoFile', reference, file });
	const created = await edit({ kind: 'CreateScreen', label: 'Owned seek output' }, 'screens');
	const surface = (await edit({ kind: 'InspectSurface', reference: created.surface }, 'screens'))
		.snapshot.definition;
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: created.surface,
			definition: { ...surface, source: reference }
		},
		'screens'
	);
	const displays = (await control({ kind: 'mapgyverDisplays', refresh: true })).displays;
	assert(displays.length);
	const display = displays.find((value) => !value.primary) ?? displays[0];
	await edit(
		{
			kind: 'ConfigureScreen',
			screen: created.screen,
			definition: {
				enabled: true,
				target: {
					kind: 'Rectangle',
					bounds: {
						left: display.bounds.left + Math.max(0, display.bounds.extent.width - 400),
						top: display.bounds.top + 120,
						extent: { width: 320, height: 240 }
					}
				}
			}
		},
		'screens'
	);
	await wait(
		state,
		(value) => {
			assert(!value.source.error, JSON.stringify(value));
			return value.source.state === 'Ready' && value.frame?.position_ms === 0;
		},
		'initial frame'
	);
	for (const position_ms of [2000, 500, 6000, 1000, 8000, 0]) {
		const started = performance.now();
		await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms } });
		const settled = await wait(
			state,
			(value) => {
				assert(!value.source.error, JSON.stringify(value));
				assert.equal(value.transport.settings.mode, 'Paused');
				assert.equal(value.transport.playing, false);
				return (
					value.source.state === 'Ready' && Math.abs(value.frame?.position_ms - position_ms) < 0.002
				);
			},
			`paused seek${position_ms}`
		);
		observations.push({
			requested_ms: position_ms,
			metadata_settled_ms: performance.now() - started,
			...settled
		});
	}
	const bursts = [];
	for (const playing of [false, true]) {
		await edit({
			kind: 'Video',
			media: reference.media,
			edit: { kind: playing ? 'Play' : 'Pause' }
		});
		const started = performance.now(),
			requests = [],
			samples = [];
		let sampling = true,
			samplingFailure;
		const sampled = (async () => {
			while (sampling) {
				const value = await state();
				assert(!value.source.error, JSON.stringify(value));
				assert.equal(value.transport.playing, playing, 'Seek preserves transport mode');
				samples.push({ elapsed_ms: performance.now() - started, ...value });
				await delay(5);
			}
		})().catch((error) => {
			samplingFailure = error;
		});
		try {
			for (let index = 0; index < 120; index++) {
				if (samplingFailure) throw samplingFailure;
				await delay(Math.max(0, started + index * 25 - performance.now()));
				const position_ms = (((index * 37) % 180) * 1000) / 24;
				await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms } });
				requests.push({ index, position_ms, elapsed_ms: performance.now() - started });
			}
		} finally {
			sampling = false;
			await sampled;
		}
		if (samplingFailure) throw samplingFailure;
		await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Pause' } });
		await edit({
			kind: 'Video',
			media: reference.media,
			edit: { kind: 'Seek', position_ms: 2000 }
		});
		const final = await wait(
			state,
			(value) => {
				assert(!value.source.error, JSON.stringify(value));
				return !value.transport.playing && Math.abs(value.frame?.position_ms - 2000) < 0.002;
			},
			'latest seek after burst'
		);
		bursts.push({ playing, requests, samples, final });
	}
	const endpoints = [];
	const duration = (await state()).source.duration_ms;
	assert(Math.abs(duration - 8291.666667) < 0.01, 'reported199-frame intraframe fixture');
	for (const [position_ms, expected_ms] of [
		[duration, 8250],
		[0, 0]
	]) {
		await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms } });
		endpoints.push(
			await wait(
				state,
				(value) => {
					assert(!value.source.error, JSON.stringify(value));
					assert.equal(value.transport.settings.mode, 'Paused');
					return (
						!value.transport.playing && Math.abs(value.frame?.position_ms - expected_ms) < 0.002
					);
				},
				`exact endpoint${position_ms}`
			)
		);
	}
	const handoffs = [];
	const handoffStarted = performance.now();
	let playing = false;
	for (let index = 0; index < 120; index++) {
		await delay(Math.max(0, handoffStarted + index * 25 - performance.now()));
		if (index > 0 && index % 30 === 0) {
			playing = !playing;
			await edit({
				kind: 'Video',
				media: reference.media,
				edit: { kind: playing ? 'Play' : 'Pause' }
			});
		}
		const position_ms = (((index * 37) % 180) * 1000) / 24;
		await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms } });
		const value = await state();
		assert(!value.source.error, JSON.stringify(value));
		assert.equal(value.transport.settings.mode, playing ? 'Playing' : 'Paused');
		handoffs.push({ index, position_ms, playing, ...value });
	}
	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Pause' } });
	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms: 2000 } });
	const handoffFinal = await wait(
		state,
		(value) => {
			assert(!value.source.error, JSON.stringify(value));
			return !value.transport.playing && Math.abs(value.frame?.position_ms - 2000) < 0.002;
		},
		'latest exact frame after pause/play handoffs'
	);
	let physical = null;
	if (process.env.MAPGYVER_SEEK_PIXELS === '1') {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Intraframe seek', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Preview$/ })
			.click();
		const viewport = page.locator(
			'.media-preview-panel .native-preview[data-preview-installed="true"]'
		);
		await viewport.waitFor({ state: 'visible' });
		const outputs = JSON.parse(
			execFileSync(
				'python',
				['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid), '--main'],
				{ encoding: 'utf8', windowsHide: true }
			)
		);
		assert.equal(outputs.length, 1);
		await writeFile(
			path.join(directory, 'physical-ready.json'),
			JSON.stringify(
				{
					pid: native.app.pid,
					selected,
					output: outputs[0]
				},
				null,
				2
			)
		);
		const deadline = Date.now() + 8 * 60 * 1000;
		while (
			!(await access(path.join(directory, 'physical-start')).then(
				() => true,
				() => false
			))
		) {
			assert(Date.now() < deadline, 'owned-window activation deadline');
			await delay(100);
		}
		const { measureSeekPixels } = await import('./native-video-seek-pixels.mjs');
		physical = await measureSeekPixels({
			native,
			directory,
			viewport,
			state,
			seek: (position_ms) =>
				edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms } }),
			mode: (playing) =>
				edit({ kind: 'Video', media: reference.media, edit: { kind: playing ? 'Play' : 'Pause' } })
		});
	}
	let visualReview = null;
	if (process.env.MAPGYVER_SEEK_REVIEW === '1') {
		const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
		await tab('Medias').click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Intraframe seek', exact: true })
			.click();
		await tab('Media Preview').click();
		await wait(
			state,
			(value) => Math.abs(value.frame?.position_ms - 2000) < 0.002,
			'review baseline'
		);
		await writeFile(
			path.join(directory, 'visual-ready.json'),
			JSON.stringify({ pid: native.app.pid, position_ms: 2000 })
		);
		async function reviewSignal(name) {
			const end = Date.now() + 8 * 60 * 1000;
			while (
				!(await access(path.join(directory, name)).then(
					() => true,
					() => false
				))
			) {
				assert(Date.now() < end, 'native visual review deadline');
				await delay(100);
			}
		}
		await reviewSignal('visual-next');
		const input = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Current Position', { exact: true }) })
			.locator('input.time-field');
		await input.fill('00:00:06.000');
		await input.press('Enter');
		const final = await wait(
			state,
			(value) => {
				assert(!value.source.error && !value.transport.playing, JSON.stringify(value));
				return Math.abs(value.frame?.position_ms - 6000) < 0.002;
			},
			'native Inspector review seek'
		);
		await writeFile(
			path.join(directory, 'visual-next-ready.json'),
			JSON.stringify({ pid: native.app.pid, position_ms: 6000 })
		);
		await reviewSignal('visual-done');
		visualReview = { baseline_ms: 2000, inspector_seek_ms: 6000, final };
	}
	assert.deepEqual(pageErrors, []);
	report = {
		result: 'pass',
		pid: native.app.pid,
		file,
		runtime,
		libraries,
		observations,
		pageErrors,
		bursts,
		endpoints,
		handoffs,
		handoffFinal,
		physical,
		visualReview,
		scope: physical
			? 'Seek correctness plus physical Media Preview observations; hardware-specific rate acceptance and observer limits require the owning audit. Latency is a conservative pre-intent-to-pixel-copy bound, not scanout timing.'
			: 'Paused/playing correctness and optional native visual review; decoder metadata is not presented FPS or latency',
		normalShutdown: false
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				error: String(error),
				file,
				pid: native.app.pid,
				observations,
				pageErrors,
				last: reference ? await state().catch(() => null) : null,
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
report.normalShutdown = true;
await writeFile(path.join(directory, 'seek.json'), JSON.stringify(report, null, 2));
console.log(
	JSON.stringify({
		result: report.result,
		pid: report.pid,
		observations: observations.length,
		normalShutdown: true
	})
);
