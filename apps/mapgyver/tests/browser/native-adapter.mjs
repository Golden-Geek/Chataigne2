import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';

export function adapterKey(id) {
	return `dxgi:${(id.high >>> 0).toString(16).padStart(8, '0')}:${id.low.toString(16).padStart(8, '0')}`;
}

export function preferenceKey(adapter) {
	assert.equal(typeof adapter.physical_id?.Ok, 'string', 'physical GPU identity must be observed');
	return adapter.physical_id.Ok;
}

// Read the public native status, never infer a device from its name or display ownership.
export async function observeNativeAdapter(page, hardware) {
	const deadline = Date.now() + 8000;
	let status;
	while (true) {
		status = await page.evaluate(async () =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
				})
			)
		);
		if (status.state === 'Running' && status.gpu_adapter) break;
		assert(Date.now() < deadline, `active native GPU not observed: ${JSON.stringify(status)}`);
		await delay(100);
	}
	const index = hardware.adapters.findIndex(
		(adapter) => adapterKey(adapter.id) === status.gpu_adapter
	);
	assert(index >= 0, `active GPU ${status.gpu_adapter} must be enumerated`);
	const adapter = hardware.adapters[index];
	assert.equal(adapter.software, false);
	assert.equal(status.gpu, adapter.description);
	return { key: status.gpu_adapter, index, description: status.gpu };
}
