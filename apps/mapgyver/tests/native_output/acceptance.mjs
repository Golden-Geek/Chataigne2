// Finite M05 gate-4 flow. Native UI observations are recorded separately by computer use.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function runFixture(arguments_) {
	const child = spawn(path.resolve('target/debug/native_qualification.exe'), arguments_, {
		windowsHide: true,
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: 15000
	});
	let output = '';
	let error = '';
	child.stdout.on('data', (chunk) => (output += chunk));
	child.stderr.on('data', (chunk) => (error += chunk));
	const code = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	assert.equal(code, 0, error);
	return JSON.parse(output);
}

export async function prepareUiApp(directory) {
	const control = path.resolve('target/m05-native-ui-launch.json');
	await writeFile(control, JSON.stringify({ directory }), { flag: 'wx' });
	const app = { pid: null };
	let polling;
	const exited = new Promise((resolve, reject) => {
		polling = setInterval(async () => {
			try {
				app.pid = JSON.parse(
					await readFile(path.join(directory, 'launcher-start.json'), 'utf8')
				).pid;
				const result = JSON.parse(
					await readFile(path.join(directory, 'launcher-result.json'), 'utf8')
				);
				clearInterval(polling);
				resolve(result);
			} catch (error) {
				if (error.code !== 'ENOENT') {
					clearInterval(polling);
					reject(error);
				}
			}
		}, 100);
	});
	console.log(
		JSON.stringify({
			launch_required: true,
			launcher: path.resolve('target/debug/native_output_launcher.exe'),
			directory
		})
	);
	return {
		app,
		exited,
		async cleanup() {
			clearInterval(polling);
			await unlink(control);
		}
	};
}

export async function runAcceptance(context) {
	const { directory, sender, appPid, screens, receive, definition, surfaces, displays } = context;
	const checkpoints = [];
	async function checkpoint(phase) {
		const state = { phase, appPid, directory, sender, displays };
		await writeFile(path.join(directory, 'acceptance-state.json'), JSON.stringify(state, null, 2));
		console.log(JSON.stringify({ inspection_ready: true, ...state }));
		const deadline = Date.now() + 150000;
		while (true) {
			try {
				await readFile(path.join(directory, `continue-${phase}`));
				break;
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
			assert(Date.now() < deadline, `native observation checkpoint timed out: ${phase}`);
			await delay(100);
		}
		checkpoints.push(phase);
	}
	async function observe(phase) {
		await receive(96, `${phase}-delivery`, true);
		const pixels = await runFixture([
			'--role',
			'pixels',
			'--adapter',
			'0',
			'--name',
			sender,
			'--directory',
			path.join(directory, `${phase}-pixels`)
		]);
		assert.deepEqual(pixels, { width: 96, height: 64, retired: true });
	}
	await observe('initial-bars');
	await checkpoint('initial'); // Open a real Screen editor and observe both native outputs.
	await observe('editor-open-bars');
	await checkpoint('editor-open'); // Minimize the app after observing its actual editor.
	for (const reference of surfaces) {
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: { ...definition, calibration: 'Grid' }
		});
	}
	await observe('minimized-grid');
	await checkpoint('minimized'); // Observe changed outputs, restore and close the editor tab.
	for (const reference of surfaces) {
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: { ...definition, calibration: 'Off' }
		});
	}
	await observe('editor-closed-source');
	await checkpoint('editor-closed');
	await writeFile(
		path.join(directory, 'acceptance-results.json'),
		JSON.stringify(
			{
				api_and_spout_observations_passed: true,
				native_ui_and_window_pixels: 'require separate recorded observations',
				checkpoints,
				appPid,
				displays
			},
			null,
			2
		)
	);
}
