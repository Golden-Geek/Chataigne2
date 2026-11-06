import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { captureAlignment, ownedWindowGeometry, ownedWindowHandle } from './capture-alignment.mjs';

export async function verifyNativeWindowLayout({ native, directory, prefix = '' }) {
	const page = native.page;
	const hwnd = ownedWindowHandle(native.app.pid);
	const geometry = () => ownedWindowGeometry(native.app.pid, hwnd);
	const initialBounds = geometry();
	assert.equal(initialBounds.window_maximized, false);
	const button = page.getByRole('button', { name: 'Maximize app', exact: true });
	await button.evaluate((element) => {
		window.__windowLayoutClicks = [];
		element.addEventListener('click', (event) => window.__windowLayoutClicks.push(event.isTrusted));
	});
	const stages = [];
	async function capture(stage, maximized) {
		const result = await captureAlignment({ directory, stage: prefix + stage, maximized, page });
		assert.equal(
			result.hwnd,
			Number(hwnd),
			'One native window must survive all layout transitions'
		);
		stages.push(result);
	}
	async function toggle(maximized) {
		const before = await page.evaluate(() => [innerWidth, innerHeight]);
		await button.click();
		const deadline = Date.now() + 5000;
		while (geometry().window_maximized !== maximized) {
			assert(Date.now() < deadline, 'Owned OS maximize/restore transition timed out');
			await delay(25);
		}
		await page.waitForFunction(
			([width, height]) => innerWidth !== width || innerHeight !== height,
			before
		);
	}
	await capture('initial', false);
	await toggle(true);
	await capture('maximized', true);
	await toggle(false);
	await capture('restored', false);
	assert.deepEqual(
		geometry(),
		initialBounds,
		'Restore must preserve original physical bounds/DPI/monitor'
	);
	const clicks = await page.evaluate(() => window.__windowLayoutClicks);
	assert.deepEqual(
		clicks,
		[true, true],
		'Both native window operations need actual trusted app-button clicks'
	);
	const result = {
		result: 'pass',
		pid: native.app.pid,
		hwnd: Number(hwnd),
		clicks,
		stages,
		scope:
			'Actual same-window maximize/restore and settled native image/overlay/projection alignment. No mixed-DPI move, transition-frame or latency acceptance.'
	};
	await writeFile(path.join(directory, 'window-layout.json'), JSON.stringify(result, null, 2));
	console.log(
		JSON.stringify({
			result: 'window-layout pass',
			pid: native.app.pid,
			maximum_error_px: Math.max(
				...stages.flatMap(({ alignment }) => [
					alignment.image_to_projection_max_px,
					alignment.overlay_to_projection_max_px,
					alignment.image_to_overlay_max_px
				])
			)
		})
	);
}
