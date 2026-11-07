// Physical output policy on real displays; read-only observations are restricted to this fixture PID.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

export async function verifyOutputCadenceRouting({
	native,
	post,
	edit,
	inspect,
	status,
	wait,
	ids,
	displays,
	directory,
	adapter,
	checks
}) {
	let retirements = 0,
		eventBuffer = '';
	const events = (chunk) => {
		const lines = (eventBuffer + chunk.toString()).split('\n');
		eventBuffer = lines.pop();
		for (const line of lines)
			if (line.includes('[mapgyver-output] native session retired')) retirements++;
	};
	native.app.stderr.on('data', events);
	try {
		const rate = (display) => display.refresh_rate.numerator / display.refresh_rate.denominator;
		const monitors = displays.displays
			.filter(
				(display) => display.refresh_rate?.numerator > 0 && display.refresh_rate?.denominator > 0
			)
			.sort((a, b) => rate(a) - rate(b));
		assert(
			monitors.length >= 2 && rate(monitors.at(-1)) - rate(monitors[0]) > 10,
			'Two actual monitors with different refresh rates are required'
		);
		const selected = [monitors[0], monitors.at(-1)];
		const physical = (display, offset = 0) => ({
			left: display.bounds.left + 80 + offset,
			top: display.bounds.top + 80,
			extent: { width: 128, height: 96 }
		});
		const windows = () =>
			JSON.parse(
				execFileSync(
					'python',
					[
						path.resolve('apps/mapgyver/tests/native_output/window_state.py'),
						String(native.app.pid)
					],
					{ encoding: 'utf8', windowsHide: true, timeout: 10000 }
				)
			);
		const media = async (payload) => {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.media',
				operation: 'edit',
				payload
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value;
		};
		const source = (
			await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Cadence physical red' })
		).reference;
		await media({
			kind: 'ConfigureMediaResolution',
			media: source.media,
			size: { kind: 'Fixed', extent: { width: 32, height: 24 } }
		});
		await media({
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		const original = (await inspect()).screens.find((screen) => screen.id === ids[0]);
		const surface = original.surfaces[0];
		await edit({
			kind: 'ConfigureSurface',
			reference: surface.reference,
			definition: { ...surface.definition, calibration: 'Off', source }
		});
		const peer = (await inspect()).screens.find((screen) => screen.id === ids[1]);
		await edit({
			kind: 'ConfigureScreen',
			screen: ids[1],
			definition: { ...peer.definition, frame_rate: { kind: 'Explicit', fps: 30 } }
		});
		let currentBounds = physical(selected[0]);
		let handle;
		async function observe(name, expectedRate, expectedHandle = handle) {
			const before = await status();
			const measured = await wait(
				status,
				(value) =>
					value.sequence >= before.sequence + 3 &&
					value.state === 'Running' &&
					!value.error &&
					value.resources?.retiring === 0 &&
					ids.every((id, index) => {
						const row = value.screens.find((row) => row.screen === id);
						const expected = index === 0 ? expectedRate : 30;
						return (
							row?.accepted_fps != null &&
							Math.abs(row.accepted_fps - expected) <= Math.max(2, expected * 0.1)
						);
					})
			);
			assert.equal(measured.gpu_adapter, adapter.key, name);
			const observed = await wait(
				windows,
				(value) =>
					value.length === 1 &&
					value[0].visible &&
					JSON.stringify(value[0].bounds) === JSON.stringify(currentBounds) &&
					JSON.stringify(value[0].center_rgb) === JSON.stringify([[255, 0, 0]])
			);
			if (expectedHandle != null)
				assert.equal(
					observed[0].hwnd,
					expectedHandle,
					`${name}: retain the physical window through rate/position reconfiguration`
				);
			handle = observed[0].hwnd;
			checks.push({
				name,
				expected_fps: expectedRate,
				native_window: observed[0],
				status: measured
			});
			return measured;
		}
		async function configure(bounds, frameRate) {
			currentBounds = bounds;
			const current = (await inspect()).screens.find((screen) => screen.id === ids[0]);
			await edit({
				kind: 'ConfigureScreen',
				screen: ids[0],
				definition: {
					...current.definition,
					target: { kind: 'Rectangle', bounds },
					frame_rate: frameRate
				}
			});
		}
		await configure(physical(selected[0]), { kind: 'FollowDisplay' });
		await observe('Follow display on slower monitor', rate(selected[0]));
		await configure(physical(selected[1]), { kind: 'FollowDisplay' });
		await observe('Follow display on faster monitor', rate(selected[1]));
		await configure(physical(selected[1], 180), { kind: 'Explicit', fps: 30 });
		await observe('Explicit30 on faster monitor', 30);
		const beforeMove = retirements;
		await configure(physical(selected[0], 180), { kind: 'Explicit', fps: 30 });
		await observe('Explicit30 after monitor move', 30);
		assert.equal(
			retirements,
			beforeMove,
			'A same-rate position-only move must not recreate the GPU owner'
		);
		await configure(physical(selected[0]), { kind: 'FollowDisplay' });
		await observe('Follow display restored', rate(selected[0]));
		const authored = await inspect();
		const file = path.join(directory, `physical-${adapter.index}.mapgyver`);
		assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
		await native.page.reload();
		await native.page.locator('footer.gc-footer output[aria-label="Video runtime"]').waitFor();
		await observe('WebView reconnect retains physical output', rate(selected[0]));
		assert.deepEqual(await inspect(), authored);
		const current = (await inspect()).screens.find((screen) => screen.id === ids[0]);
		await edit({
			kind: 'ConfigureScreen',
			screen: ids[0],
			definition: { ...current.definition, enabled: false }
		});
		await wait(windows, (value) => value.length === 0);
		await wait(status, (value) => !value.screens.some((row) => row.screen === ids[0]));
		checks.push({ name: 'Disabled output physically retired' });
		await edit({ kind: 'ConfigureScreen', screen: ids[0], definition: current.definition });
		await observe('Reenabled output restores cadence', rate(selected[0]), null);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await wait(windows, (value) => value.length === 0);
		checks.push({ name: 'New project retires the old physical window' });
		const loaded = await post('/api/ui/project-load', { path: file });
		assert.equal(path.resolve(loaded.path), file);
		assert.equal(loaded.recovery ?? null, null);
		assert.deepEqual(await inspect(), authored);
		await observe(
			'Project load restores authored cadence and output pixels',
			rate(selected[0]),
			null
		);
		return {
			checks,
			scope:
				'Actual API delivery rates, physical bounds/red compositor pixels, window identity and adapter retention. No scanout pacing, pointer latency or physical unplug/replug claim.'
		};
	} finally {
		native.app.stderr.off('data', events);
	}
}
