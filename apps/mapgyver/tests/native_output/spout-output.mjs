import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtemp } from 'node:fs/promises';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { adapterKey } from '../browser/native-adapter.mjs';

const run = promisify(execFile);

export async function openSpoutOutputObserver(directory) {
	const receiver = process.env.MAPGYVER_TEST_SPOUT_RECEIVER;
	assert(receiver, 'Set MAPGYVER_TEST_SPOUT_RECEIVER to the native qualification executable');
	const options = { windowsHide: true, timeout: 7000 };
	const { stdout } = await run(receiver, ['--role', 'hardware'], options);
	const hardware = JSON.parse(stdout);
	const preferred = hardware.preferred_adapters.find(({ software }) => !software);
	assert(preferred, `No hardware adapter: ${JSON.stringify(hardware.adapters)}`);
	const key = adapterKey(preferred.id);
	const index = hardware.adapters.findIndex(({ id }) => key === adapterKey(id));
	assert(index >= 0, `Preferred adapter ${key} is missing from the DXGI catalog`);
	const sender = `MapGyver-Arm64-${randomUUID()}`;
	const extent = { width: 96, height: 64 };

	async function expectPixels(expected, centerOnly = false) {
		const deadline = performance.now() + 12000;
		for (;;) {
			const frame = await mkdtemp(join(directory, 'spout-frame-'));
			const capture = await run(
				receiver,
				['--role', 'pixels', '--adapter', String(index), '--name', sender, '--directory', frame],
				options
			);
			assert.deepEqual(JSON.parse(capture.stdout), { ...extent, retired: true });
			const pixels = await run(
				'python',
				[
					'-c',
					'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGBA"); ' +
						'x,y=im.width//2,im.height//2; im=im.crop((x-2,y-2,x+3,y+3)) if sys.argv[2]=="true" else im; ' +
						'print(json.dumps(sorted(set(im.get_flattened_data()))))',
					join(frame, 'pixels.png'),
					String(centerOnly)
				],
				options
			);
			const actual = JSON.parse(pixels.stdout);
			const tolerance = centerOnly ? 4 : 0;
			if (
				actual.length &&
				actual.every(
					([r, g, b, a]) =>
						a === 255 &&
						[r, g, b].every(
							(value, component) => Math.abs(value - expected[component]) <= tolerance
						)
				)
			) {
				console.log(
					JSON.stringify({
						result: 'pass',
						scope: 'Independent Spout pixels',
						adapter: key,
						expected,
						actual
					})
				);
				return;
			}
			assert(
				performance.now() < deadline,
				`Spout expected ${expected}; read ${JSON.stringify(actual)}`
			);
		}
	}

	return { target: { kind: 'Spout', sender, extent }, expectPixels };
}
