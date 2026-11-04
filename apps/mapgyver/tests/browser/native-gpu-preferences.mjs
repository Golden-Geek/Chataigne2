// Actual generic Inspector, private persistent app data, cold starts and exact-adapter pixels.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey } from './native-adapter.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-gpu-preferences-${Date.now()}`);
await mkdir(directory);
const appDataDirectory = path.join(directory, 'app-data');
const executable = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(executable, ['--role', 'hardware'], { windowsHide: true, encoding: 'utf8' })
);
const preferred = hardware.preferred_adapters.find((adapter) => !adapter.software);
const other = hardware.adapters.find(
	(adapter) => !adapter.software && adapterKey(adapter.id) !== adapterKey(preferred.id)
);
assert(other, 'acceptance needs two available hardware GPUs');
const preferredKey = adapterKey(preferred.id),
	otherKey = adapterKey(other.id);
const missing = 'dxgi:ffffffff:ffffffff';
assert(!hardware.adapters.some((adapter) => adapterKey(adapter.id) === missing));
const results = [];

async function stage(name, selected, expected, next) {
	const native = await openNativeEditor({ appDataDirectory });
	let output, recreated;
	try {
		let token;
		async function post(route, body) {
			const response = await fetch(`${native.base}${route}`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					...(token ? { Authorization: `Bearer ${token}` } : {})
				},
				body: JSON.stringify(body),
				signal: AbortSignal.timeout(8000)
			});
			const value = await response.json();
			assert(response.ok, JSON.stringify(value));
			return value;
		}
		token = (await post('/api/ui/session', {})).token;
		const edit = async (payload) => {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.media',
				operation: 'edit',
				payload
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value;
		};
		async function waitStatus(predicate) {
			const deadline = Date.now() + 10000;
			while (true) {
				const status = await native.page.evaluate(async () =>
					JSON.parse(
						await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
							request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
						})
					)
				);
				if (predicate(status)) return status;
				assert(Date.now() < deadline, JSON.stringify(status));
				await delay(100);
			}
		}
		await native.page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await native.page
			.getByRole('menuitem')
			.filter({ has: native.page.getByText('Preferences', { exact: true }) })
			.click();
		const widget = native.page
			.locator('.parameter-inspector')
			.filter({ has: native.page.getByText('GPU', { exact: true }) });
		const choice = widget.locator('select.dropdown-editor');
		try {
			await choice.waitFor();
		} catch (error) {
			await writeFile(
				path.join(directory, `${name}-inspector-failure.txt`),
				`${await native.page.locator('body').innerText()}\n\n${native.log()}`
			);
			throw error;
		}
		for (const key of [preferredKey, otherKey])
			await choice.locator(`option[value="${key}"]`).waitFor({ state: 'attached' });
		await choice.locator(`option[value="${selected}"]`).waitFor({ state: 'attached' });
		assert.equal(await choice.inputValue(), selected);
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'GPU acceptance'
		});
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		output = await createPreviewOutputObserver(post, path.join(directory, name), native.page);
		if (expected === null) {
			await output.demand(reference);
			const status = await waitStatus(
				(status) => status.state === 'Faulted' && status.gpu_preference === missing
			);
			assert.equal(status.gpu_adapter, null);
			assert.equal(status.gpu_restart_required, false);
			assert.match(status.error, /selected GPU.*unavailable/i);
			assert.match(await choice.locator(`option[value="${missing}"]`).innerText(), /unavailable/);
			results.push({ name, status });
		} else {
			await output.observe('first-grid', { first: reference });
			const status = await waitStatus(
				(status) => status.state === 'Running' && status.gpu_preference === selected
			);
			assert.equal(status.gpu_adapter, expected);
			assert.equal(status.gpu_restart_required, false);
			results.push({ name, status });
		}
		if (next) {
			// Creation updates shared workbench selection; reveal the actual Preferences object again.
			await native.page.getByRole('button', { name: 'Open File menu', exact: true }).click();
			await native.page
				.getByRole('menuitem')
				.filter({ has: native.page.getByText('Preferences', { exact: true }) })
				.click();
			await choice.selectOption(next);
			const changed = await waitStatus(
				(status) => status.gpu_preference === next && status.gpu_restart_required
			);
			assert.equal(changed.gpu_adapter, expected);
			assert.match(
				await native.page.locator('output[aria-label="Video runtime"]').innerText(),
				/GPU restart required/
			);
			if (expected) {
				await output.observePixels('preference-changed', [255, 0, 0, 255]);
				await output.finish();
				recreated = await createPreviewOutputObserver(
					post,
					path.join(directory, `${name}-recreated`),
					native.page
				);
				await recreated.observe('first-grid', { first: reference });
				const afterRecreation = await waitStatus(
					(status) => status.state === 'Running' && status.gpu_restart_required
				);
				assert.equal(afterRecreation.gpu_adapter, expected);
				results.at(-1).afterRecreation = afterRecreation;
			}
			results.at(-1).changed = changed;
		}
		await delay(1000);
		results.at(-1).output = await output.finish();
		if (recreated) results.at(-1).recreatedOutput = await recreated.finish();
		console.log(JSON.stringify(results.at(-1)));
	} finally {
		try {
			await recreated?.finish();
		} finally {
			try {
				await output?.finish();
			} finally {
				await native.close();
			}
		}
	}
}

await stage('automatic', 'Automatic', preferredKey, otherKey);
await stage('explicit-other', otherKey, otherKey, preferredKey);
await stage('explicit-preferred', preferredKey, preferredKey, null);
const files = (await readdir(appDataDirectory, { recursive: true })).filter(
	(file) => path.basename(file) === 'preferences.json'
);
assert.equal(files.length, 1);
const preferences = path.resolve(appDataDirectory, files[0]);
assert(preferences.startsWith(appDataDirectory + path.sep));
execFileSync(
	executable,
	['--role', 'gpu-preferences', '--file', preferences, '--selected', missing],
	{ windowsHide: true }
);
await stage('missing', missing, null, 'Automatic');
await stage('recovered', 'Automatic', preferredKey, null);
await writeFile(
	path.join(directory, 'gpu-preferences.json'),
	JSON.stringify({ hardware, preferredKey, otherKey, missing, results }, null, 2)
);
