// One independent SDK consumer stays alive across preview lifetime changes.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { observeNativeAdapter } from './native-adapter.mjs';

async function run(executable, args) {
	const child = spawn(executable, args, {
		windowsHide: true,
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: 15000
	});
	let out = '',
		error = '';
	child.stdout.on('data', (chunk) => (out += chunk));
	child.stderr.on('data', (chunk) => (error += chunk));
	const code = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	assert.equal(code, 0, error);
	return out;
}
export async function createPreviewOutputObserver(post, directory, page) {
	await mkdir(directory, { recursive: true });
	const executable = path.resolve('target/debug/native_qualification.exe');
	const sender = `MapGyverPreviewAcceptance-${randomUUID()}`;
	const observations = [];
	const hardware = JSON.parse(await run(executable, ['--role', 'hardware']));
	let adapter;
	let consumer = null,
		exited = null,
		stderr = '',
		summary = null;
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
	async function demand(first) {
		const { screen, surface } = await screens({
			kind: 'CreateScreen',
			label: 'Independent preview acceptance'
		});
		await screens({
			kind: 'ConfigureSurface',
			reference: surface,
			definition: {
				source: first,
				crop: { x: 0, y: 0, width: 1, height: 1 },
				quad: {
					corners: [
						{ x: 0, y: 0 },
						{ x: 1, y: 0 },
						{ x: 1, y: 1 },
						{ x: 0, y: 1 }
					]
				},
				opacity: 1,
				enabled: true,
				calibration: 'Off'
			}
		});
		await screens({
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				target: { kind: 'Spout', sender, extent: { width: 96, height: 64 } }
			}
		});
	}
	async function pixels(phase, expected) {
		const actual = await observeNativeAdapter(page, hardware);
		adapter ??= actual;
		assert.equal(actual.key, adapter.key, 'shared-texture connection retains its exact GPU');
		const target = path.join(directory, phase);
		const image = JSON.parse(
			await run(executable, [
				'--role',
				'pixels',
				'--adapter',
				String(adapter.index),
				'--name',
				sender,
				'--directory',
				target
			])
		);
		assert.deepEqual(image, { width: 96, height: 64, retired: true });
		const colors = JSON.parse(
			await run('python', [
				'-c',
				'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGBA"); print(json.dumps(sorted(set(im.getdata()))))',
				path.join(target, 'pixels.png')
			])
		);
		assert.deepEqual(colors, [expected], `${phase}: complete output pixels`);
		observations.push({ phase, rgba: expected, extent: [96, 64] });
	}
	async function finish() {
		if (!consumer) {
			if (summary)
				await writeFile(
					path.join(directory, 'output-summary.json'),
					JSON.stringify({ summary, observations }, null, 2)
				);
			return summary;
		}
		consumer.stdin.end('stop\n');
		const code = await exited;
		consumer = null;
		assert.equal(code, 0, stderr);
		const rows = (await readFile(path.join(directory, 'consumer', 'consumer.jsonl'), 'utf8'))
			.trim()
			.split('\n')
			.map(JSON.parse);
		const final = rows.find((row) => row.kind === 'consumer_final').consumer;
		for (const key of ['failed_receives', 'mismatch', 'overflow', 'absent'])
			assert.equal(final[key], 0, key);
		assert(final.receive_copies >= 20);
		const frames = rows.filter((row) => row.kind === 'consumer_frame').map((row) => row.frame);
		assert.equal(frames.length, final.logged_frames);
		const versions = frames.map((frame) => frame.spout_image);
		assert(
			versions.every((version) => version && version.shared_handle === versions[0].shared_handle),
			'one stable sender handle throughout panel changes'
		);
		assert(
			versions.at(-1).frame > versions[0].frame,
			'independent output kept producing newer frames'
		);
		const frequency = rows.find((row) => row.kind === 'consumer_config').qpc_frequency;
		const gaps = frames
			.slice(1)
			.map(
				(frame, index) =>
					((frame.receive_begin_qpc - frames[index].receive_begin_qpc) * 1000) / frequency
			);
		summary = {
			adapter,
			copies: final.receive_copies,
			loggedFrames: frames.length,
			absent: final.absent,
			failedReceives: final.failed_receives,
			descriptorMismatch: final.mismatch,
			stableSharedHandle: versions[0].shared_handle,
			firstFrame: versions[0].frame,
			lastFrame: versions.at(-1).frame,
			maxReceiveGapMs: Math.max(...gaps),
			exitCode: code
		};
		await writeFile(
			path.join(directory, 'output-summary.json'),
			JSON.stringify({ summary, observations }, null, 2)
		);
		return summary;
	}
	return {
		demand,
		observations,
		observePixels: pixels,
		finish,
		async observe(phase, { first }) {
			if (phase === 'first-grid') {
				await demand(first);
				await pixels(phase, [255, 0, 0, 255]);
				const target = path.join(directory, 'consumer');
				await mkdir(target);
				consumer = spawn(
					executable,
					[
						'--role',
						'consumer',
						'--adapter',
						String(adapter.index),
						'--name',
						sender,
						'--width',
						'96',
						'--height',
						'64',
						'--instance',
						randomUUID(),
						'--directory',
						target,
						'--max-seconds',
						'120',
						'--max-frames',
						'8192',
						'--trace-capacity',
						'65536'
					],
					{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'], timeout: 130000 }
				);
				exited = new Promise((resolve, reject) => {
					consumer.once('error', reject);
					consumer.once('close', resolve);
				});
				consumer.stderr.on('data', (chunk) => (stderr += chunk));
				let stdout = '';
				await new Promise((resolve, reject) => {
					consumer.stdout.on('data', (chunk) => {
						stdout += chunk;
						if (stdout.includes('"kind":"ready"')) resolve();
					});
					exited.then(
						(code) => reject(new Error(`consumer exited before readiness: ${code} ${stderr}`)),
						reject
					);
				});
			} else if (
				[
					'concurrent-eight',
					'transport-reconnected',
					'peer-closed',
					'lock-changed',
					'all-previews-closed',
					'project-reloaded'
				].includes(phase)
			) {
				await pixels(phase, [255, 0, 0, 255]);
			} else if (phase === 'output-edited-without-previews') {
				await pixels(phase, [0, 255, 0, 255]);
			} else if (phase === 'before-new') {
				await finish();
			}
		}
	};
}
