// The existing complete Matrix spatial contract in the ordinary owned WebView2 application.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { verifyMatrixLayoutSelection } from './matrix-layout-selection.mjs';
import { verifyMatrixSpatialLifetime } from './matrix-spatial-lifetime.mjs';

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
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const contract = await verifyMatrixLayoutSelection({
		base: native.base,
		edit,
		post,
		page: native.page
	});
	const lifetime = await verifyMatrixSpatialLifetime({
		page: native.page,
		post,
		edit,
		directory,
		scene: contract
	});
	const adapter = await observeNativeAdapter(native.page, hardware);
	assert.equal(adapter.index, gpuIndex);
	const installedViews = await native.page.waitForFunction(
		() => {
			const views = [
				...document.querySelectorAll(
					'.workbench-editor[data-spatial-kind="Matrix"] .native-preview'
				)
			]
				.filter((node) => node.getBoundingClientRect().width > 0)
				.map((node) => ({
					installed: node.dataset.previewInstalled,
					state: node.dataset.previewState
				}));
			if (
				views.length === 2 &&
				views.every((view) => view.installed === 'true' && view.state === 'available')
			)
				return views;
			return null;
		},
		null,
		{ timeout: 7000 }
	);
	const views = await installedViews.jsonValue();
	await installedViews.dispose();
	assert.deepEqual(errors, []);
	const result = {
		result: 'pass',
		pid: native.app.pid,
		gpuIndex,
		adapter,
		contract,
		lifetime,
		views,
		scope:
			'actual native WebView/Inspector and installed native previews; physical input and independent pixel/hit alignment remain separate',
		errors
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify({ result: 'pass', pid: native.app.pid, gpuIndex }));
} catch (error) {
	const views = await native.page.locator('.native-preview').evaluateAll((nodes) =>
		nodes.map((node) => ({
			attributes: [...node.attributes]
				.filter((attribute) => attribute.name.startsWith('data-'))
				.map((attribute) => [attribute.name, attribute.value]),
			text: node.textContent,
			width: node.getBoundingClientRect().width
		}))
	);
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{ error: String(error), proof: error.proof, errors, views, log: native.log() },
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
