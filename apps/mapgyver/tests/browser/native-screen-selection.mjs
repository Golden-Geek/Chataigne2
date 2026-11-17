// Run the complete existing Screen contract and its added lifetime scope in WebView2.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { verifyScreenSelection } from './screen-selection.mjs';
import { verifyScreenSelectionLifetime } from './screen-selection-lifetime.mjs';

const directory = path.resolve(process.argv[2]);
const gpuIndex = Number(process.argv[3]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the current compiled ordinary application');
const observer = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
assert(
	Number.isInteger(gpuIndex) && hardware.adapters[gpuIndex] && !hardware.adapters[gpuIndex].software
);
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const folder = path.join(root, 'MapGyver');
		await mkdir(folder, { recursive: true });
		execFileSync(
			observer,
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(folder, 'preferences.json'),
				'--selected',
				adapterKey(hardware.adapters[gpuIndex].id)
			],
			{ windowsHide: true }
		);
	}
});
let token;
const errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const contract = await verifyScreenSelection({
		base: native.base,
		post,
		page: native.page,
		beforeNew: async (scene) => {
			const lifetime = await verifyScreenSelectionLifetime({
				page: native.page,
				post,
				scene,
				directory
			});
			const adapter = await observeNativeAdapter(native.page, hardware);
			assert.equal(adapter.index, gpuIndex);
			await native.page.waitForFunction(
				() => {
					const views = [
						...document.querySelectorAll(
							'.workbench-editor[data-panel-type="screenWarp"] .native-preview'
						)
					].filter((node) => node.getBoundingClientRect().width > 0);
					return (
						views.length === 2 &&
						views.every(
							(node) =>
								node.dataset.previewInstalled === 'true' &&
								node.dataset.previewState === 'available'
						)
					);
				},
				null,
				{ timeout: 7000 }
			);
			return { lifetime, adapter, installed_previews: 2 };
		}
	});
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: native.app.pid,
				gpuIndex,
				contract,
				errors,
				scope:
					'actual workbench/Inspector and installed native previews; physical input and independent native pixels/hits remain separate'
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', pid: native.app.pid, gpuIndex }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				error: String(error),
				proof: error.proof,
				errors,
				log: native.log(),
				dom: await native.page
					.locator('.workbench-editor[data-panel-type="screenWarp"]')
					.evaluateAll((nodes) =>
						nodes.map((node) => ({
							id: node.dataset.panelId,
							busy: node.getAttribute('aria-busy'),
							zone: node.querySelector('.active-zone')?.getBoundingClientRect().toJSON(),
							controls: node
								.querySelector('svg[aria-label="Surface mapping controls"]')
								?.getBoundingClientRect()
								.toJSON(),
							polygons: [...node.querySelectorAll('polygon')].map((n) => [
								n.getAttribute('data-surface-drop-target'),
								n.getAttribute('points'),
								n.getAttribute('class')
							])
						}))
					),
				tabs: await native.page.getByRole('tab').allTextContents()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
