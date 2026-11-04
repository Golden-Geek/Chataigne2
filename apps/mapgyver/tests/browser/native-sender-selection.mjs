import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

async function inventory() {
	const child = spawn(
		path.resolve('target/debug/native_qualification.exe'),
		['--role', 'hardware'],
		{ windowsHide: true }
	);
	let text = '',
		error = '';
	child.stdout.on('data', (chunk) => (text += chunk));
	child.stderr.on('data', (chunk) => (error += chunk));
	const code = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	assert.equal(code, 0, error);
	return JSON.parse(text);
}

async function source(adapter, name) {
	const child = spawn(
		path.resolve('target/debug/native_source_fixture.exe'),
		['--adapter', String(adapter), '--sender', name],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	let error = '';
	child.stderr.on('data', (chunk) => (error += chunk));
	const exited = new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
	async function reply() {
		const result = await Promise.race([
			lines.next(),
			delay(12000, undefined, { ref: false }).then(() => {
				throw Error(`sender reply timeout: ${name}: ${error}`);
			})
		]);
		assert(
			!result.done && result.value.startsWith('OK '),
			`${name}: ${JSON.stringify(result)} ${error}`
		);
	}
	await reply();
	return {
		name,
		pid: child.pid,
		async command(text) {
			child.stdin.write(text + '\n');
			await reply();
		},
		async close() {
			if (child.exitCode == null) {
				child.stdin.end('exit\n');
			}
			assert.equal(await exited, 0, error);
		}
	};
}

export async function verifyNativeSenderSelection({ page, edit, post, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_SENDER_SELECTION_DIRECTORY);
	await mkdir(directory);
	const errors = [],
		sources = [],
		checks = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { rejected, observed } = await observeNativeRejectedRequests(
		page,
		() => 'native live sender selection'
	);
	const firstOutput = await createPreviewOutputObserver(
		post,
		path.join(directory, 'first-output'),
		page
	);
	const peerOutput = await createPreviewOutputObserver(
		post,
		path.join(directory, 'peer-output'),
		page
	);
	async function wait(read, predicate) {
		const deadline = Date.now() + 10000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `sender selection: ${JSON.stringify(value)}`);
			await delay(50);
		}
	}
	try {
		const hardware = await inventory();
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SharedTexture',
			label: 'Live sender controls'
		});
		const { reference: peer } = await edit({ kind: 'AddChannel', media: reference.media });
		await firstOutput.demand(reference);
		await peerOutput.demand(peer);
		const adapter = await observeNativeAdapter(page, hardware);
		const a = await source(adapter.index, `MapGyver-Detect-A-${nativePid}`);
		sources.push(a);
		const b = await source(adapter.index, `MapGyver-Detect-B-${nativePid}`);
		sources.push(b);
		const read = async () =>
			(
				await edit({
					kind: 'InspectChannels',
					query: { media: reference.media, start: 0, count: 8 }
				})
			).page.channels;
		const sender = (channels, selected = reference) =>
			channels.find((channel) => channel.reference.channel === selected.channel).definition.source
				.sender;
		async function select() {
			await page.getByRole('tab').first().click();
			await page
				.locator('section[data-panel-id="media-manager"]')
				.getByRole('button', { name: 'Live sender controls', exact: true })
				.click();
		}
		await select();
		const rows = page.locator(
			'.channels-section > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
		);
		const first = rows.first(),
			second = rows.nth(1);
		const mode = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Sender selection', { exact: true }) })
			.locator('select');
		await wait(
			() => rows.count(),
			(value) => value === 2
		);
		assert.equal(await rows.getByRole('button', { name: 'Detect sender', exact: true }).count(), 2);
		await first.getByRole('button', { name: 'Detect sender', exact: true }).click();
		await a.command('start 64 64 1 0 0');
		const detected = page.getByRole('menuitem', { name: a.name, exact: true });
		await detected.waitFor();
		await detected.click();
		await wait(read, (channels) => sender(channels) === a.name && sender(channels, peer) === null);
		await wait(
			() => first.locator('input.string-editor').inputValue(),
			(name) => name === a.name
		);
		const inputBox = await first.locator('input.string-editor').boundingBox();
		const detectBox = await first
			.getByRole('button', { name: 'Detect sender', exact: true })
			.boundingBox();
		assert(
			inputBox &&
				detectBox &&
				detectBox.x >= inputBox.x + inputBox.width - 1 &&
				Math.abs(detectBox.y + detectBox.height / 2 - (inputBox.y + inputBox.height / 2)) < 3,
			'Detect is inline with the real String editor'
		);
		await page.screenshot({ path: path.join(directory, 'manual-detect.png') });
		checks.push('Manual inline Detect updates while open and writes the real String only');
		await mode.selectOption('Auto');
		await wait(
			() => rows.locator('input.string-editor').count(),
			(value) => value === 0
		);
		assert.equal(await rows.getByRole('button', { name: 'Detect sender', exact: true }).count(), 0);
		const auto = first.locator('select.dropdown-editor'),
			peerAuto = second.locator('select.dropdown-editor');
		assert.equal(await auto.inputValue(), a.name);
		await b.command('start 64 64 0 0 1');
		await wait(
			() => peerAuto.locator('option').allTextContents(),
			(names) => names.includes(b.name)
		);
		await peerAuto.selectOption(b.name);
		await wait(
			read,
			(channels) => sender(channels) === a.name && sender(channels, peer) === b.name
		);
		await delay(1500);
		await firstOutput.observePixels('initial-red', [255, 0, 0, 255]);
		await peerOutput.observePixels('initial-blue', [0, 0, 255, 255]);
		checks.push('Auto uses only real Enum widgets; channels render independent live senders');
		await a.command('stop');
		await wait(
			() => auto.locator('option:checked').textContent(),
			(label) => label.includes('(unavailable)')
		);
		assert.equal(await auto.inputValue(), a.name);
		await peerOutput.observePixels('peer-survives-absence', [0, 0, 255, 255]);
		await a.command('start 64 64 1 0 0');
		await wait(
			() => auto.locator('option:checked').textContent(),
			(label) => label === a.name
		);
		await delay(1500);
		await firstOutput.observePixels('exact-restart-red', [255, 0, 0, 255]);
		checks.push(
			'Disappearance retains exact selection, peer keeps rendering, exact-name restart recovers'
		);
		await a.command('stop');
		const renamed = await source(adapter.index, `${a.name}-renamed`);
		sources.push(renamed);
		await renamed.command('start 64 64 0 1 0');
		await wait(
			() => auto.locator('option').allTextContents(),
			(names) => names.includes(renamed.name)
		);
		assert.equal(await auto.inputValue(), a.name);
		await auto.selectOption(renamed.name);
		await wait(read, (channels) => sender(channels) === renamed.name);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => auto.inputValue(),
			(name) => name === a.name
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => auto.inputValue(),
			(name) => name === renamed.name
		);
		await mode.selectOption('Manual');
		await wait(
			() => first.locator('input.string-editor').inputValue(),
			(name) => name === renamed.name
		);
		assert.equal(await second.locator('input.string-editor').inputValue(), b.name);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => auto.inputValue(),
			(name) => name === renamed.name
		);
		await delay(1500);
		await firstOutput.observePixels('renamed-green', [0, 255, 0, 255]);
		checks.push('Rename never retargets implicitly; selection and mode changes undo atomically');
		const otherAdapter = hardware.adapters.findIndex(
			(candidate, index) => index !== adapter.index && !candidate.software
		);
		assert(otherAdapter >= 0, 'adapter mismatch acceptance requires the second hardware GPU');
		const foreign = await source(otherAdapter, `MapGyver-Foreign-${nativePid}`);
		sources.push(foreign);
		await foreign.command('start 64 64 1 0 1');
		await wait(
			() => auto.locator('option').allTextContents(),
			(names) => names.includes(foreign.name)
		);
		await auto.selectOption(foreign.name);
		await wait(read, (channels) => sender(channels) === foreign.name);
		await delay(1500);
		await peerOutput.observePixels('peer-survives-adapter-mismatch', [0, 0, 255, 255]);
		assert.equal((await observeNativeAdapter(page, hardware)).key, adapter.key);
		await auto.selectOption(renamed.name);
		await wait(read, (channels) => sender(channels) === renamed.name);
		await delay(1500);
		await firstOutput.observePixels('restored-after-adapter-mismatch', [0, 255, 0, 255]);
		checks.push(
			'Foreign-adapter input leaves the exact choice and renderer GPU intact; peer and recovery verified'
		);
		const project = path.join(directory, 'live-senders.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await post('/api/ui/project-load', { path: project });
		await select();
		await wait(
			() => auto.inputValue(),
			(name) => name === renamed.name
		);
		await delay(1500);
		await firstOutput.observePixels('project-reload-green', [0, 255, 0, 255]);
		await peerOutput.observePixels('project-reload-blue', [0, 0, 255, 255]);
		await renamed.command('stop');
		await wait(
			() => auto.locator('option:checked').textContent(),
			(label) => label.includes('(unavailable)')
		);
		assert.equal(await auto.inputValue(), renamed.name);
		await peerOutput.observePixels('peer-survives-post-load-absence', [0, 0, 255, 255]);
		await renamed.command('start 64 64 0 1 0');
		await wait(
			() => auto.locator('option:checked').textContent(),
			(label) => label === renamed.name
		);
		await delay(1500);
		await firstOutput.observePixels('post-load-exact-recovery-green', [0, 255, 0, 255]);
		await mode.selectOption('Manual');
		await wait(
			() => first.locator('input.string-editor').inputValue(),
			(name) => name === renamed.name
		);
		await mode.selectOption('Auto');
		await wait(
			() => auto.inputValue(),
			(name) => name === renamed.name
		);
		checks.push('Save/New/Load preserves typed mode, exact independent choices and GPU output');
		await wait(
			() => page.locator('.user-action-activity').getAttribute('aria-hidden'),
			(value) => value === 'true'
		);
		await wait(
			() => ({ ...observed }),
			(value) => value.requests > 0 && value.requests === value.receipts
		);
		await page.screenshot({ path: path.join(directory, 'auto-live.png') });
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		const result = {
			result: 'pass',
			nativePid,
			adapter,
			sources: sources.map(({ name, pid }) => ({ name, pid })),
			checks,
			observed,
			pixelChecks: [...firstOutput.observations, ...peerOutput.observations]
		};
		await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
		console.log(JSON.stringify(result));
	} finally {
		await Promise.all(sources.map((source) => source.close()));
		await firstOutput.finish();
		await peerOutput.finish();
	}
}
