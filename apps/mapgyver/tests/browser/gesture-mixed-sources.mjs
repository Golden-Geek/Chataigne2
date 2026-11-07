import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';
import { crc32, deflateSync } from 'node:zlib';

export async function picture(file, rgb, inset = 0) {
	const width = 120,
		height = 90;
	const rows = Buffer.alloc(height * (1 + width * 3));
	for (let y = 0; y < height; y++)
		for (let x = 0; x < width; x++) {
			if (
				x < width * inset ||
				x >= width * (1 - inset) ||
				y < height * inset ||
				y >= height * (1 - inset)
			)
				continue;
			rgb.forEach((value, channel) => {
				rows[y * (1 + width * 3) + 1 + x * 3 + channel] = value;
			});
		}
	const chunk = (name, data) => {
		const type = Buffer.from(name),
			length = Buffer.alloc(4),
			check = Buffer.alloc(4);
		length.writeUInt32BE(data.length);
		check.writeUInt32BE(crc32(Buffer.concat([type, data])));
		return Buffer.concat([length, type, data, check]);
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(width);
	header.writeUInt32BE(height, 4);
	header[8] = 8;
	header[9] = 2;
	await writeFile(
		file,
		Buffer.concat([
			Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
			chunk('IHDR', header),
			chunk('IDAT', deflateSync(rows)),
			chunk('IEND', Buffer.alloc(0))
		])
	);
}

async function ownedSource(adapter, pixel) {
	assert(
		process.env.GOLDEN_VIDEO_TEST_BINARY,
		'Select the current compiled Golden qualification test executable'
	);
	const name = `MapGyverFormat-${randomUUID()}`;
	const child = spawn(
		path.resolve(process.env.GOLDEN_VIDEO_TEST_BINARY),
		[
			'--exact',
			'native::inputs::tests::formats::source_fixture::owned_native_source_for_app_format_checks',
			'--ignored',
			'--nocapture',
			'--test-threads=1'
		],
		{
			windowsHide: true,
			stdio: ['pipe', 'pipe', 'pipe'],
			env: {
				...process.env,
				GOLDEN_VIDEO_APP_SOURCE_ADAPTER: String(adapter),
				GOLDEN_VIDEO_APP_SOURCE_NAME: name
			}
		}
	);
	let errors = '';
	child.stdin.on('error', () => {});
	child.stderr.on('data', (data) => {
		errors = (errors + data).slice(-65536);
	});
	const exited = new Promise((resolve) => {
		child.once('error', (error) => resolve({ error: String(error) }));
		child.once('close', (code) => resolve({ code }));
	});
	const reader = createInterface({ input: child.stdout });
	const lines = reader[Symbol.asyncIterator]();
	async function reply() {
		const deadline = Date.now() + 12000;
		for (;;) {
			const row = await Promise.race([
				lines.next(),
				delay(Math.max(1, deadline - Date.now()), undefined, { ref: false }).then(() => {
					throw Error(`Owned source reply expired: ${errors}`);
				})
			]);
			assert(!row.done, errors);
			if (row.value.includes('OK ')) return row.value;
			assert(Date.now() < deadline, errors);
		}
	}
	async function close() {
		child.stdin.end('exit\n');
		let result = await Promise.race([exited, delay(5000, null, { ref: false })]);
		if (!result) {
			child.kill();
			result = await exited;
		}
		reader.close();
		assert.equal(result.code, 0, JSON.stringify({ result, errors }));
	}
	try {
		assert((await reply()).includes('OK ready'));
		child.stdin.write(
			JSON.stringify({
				format: 87,
				width: 4,
				height: 4,
				bytes: Array.from({ length: 16 }, () => pixel).flat()
			}) + '\n'
		);
		const acknowledgement = await reply();
		assert(acknowledgement.includes('OK started 87 4 4'), acknowledgement);
		return { name, pid: child.pid, acknowledgement, close };
	} catch (error) {
		await close();
		throw error;
	}
}

export async function createMixedGestureSources({ edit, directory, colors, adapter }) {
	const source = await ownedSource(adapter, [0, 188, 255, 255]);
	const records = [];
	async function create(kind, label) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: kind, label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
		});
		return reference;
	}
	async function solid(index, label) {
		const reference = await create('SolidColor', label);
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: colors[index].rgba }
		});
		return reference;
	}
	try {
		for (let index = 0; index < 9; index++) {
			const kind = [
				'Picture',
				'SolidColor',
				'Picture',
				'Reference',
				'Composition',
				'Matrix',
				'SolidColor',
				'Picture',
				'SharedTexture'
			][index];
			const label = index === 0 ? 'Gesture source' : `Gesture source ${index + 1}`;
			let reference;
			if (kind === 'SolidColor') reference = await solid(index, label);
			else if (kind === 'Reference') {
				const target = await solid(index, 'Mixed Reference leaf');
				const { media } = await edit({ kind: 'CreateReference', target: target.media, label });
				const channels = (
					await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 1 } })
				).page.channels;
				assert.equal(channels.length, 1);
				reference = channels[0].reference;
				await edit({
					kind: 'ConfigureMediaResolution',
					media,
					size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
				});
			} else {
				reference = await create(kind, label);
				if (kind === 'Picture') {
					const file = path.join(directory, `mixed-${index}.png`);
					await picture(file, colors[index].rgb);
					await edit({ kind: 'ConfigurePictureFile', reference, file });
				} else if (kind === 'SharedTexture') {
					await edit({
						kind: 'ConfigureMediaSenderSelection',
						media: reference.media,
						mode: 'Manual'
					});
					await edit({
						kind: 'ConfigureChannel',
						reference,
						source: { kind: 'Spout', sender: source.name }
					});
				} else if (kind === 'Composition') {
					const leaf = await solid(index, 'Mixed Composition leaf');
					const comp = (value) => edit({ kind: 'Composition', reference, edit: value });
					const { instances } = await comp({ kind: 'AddInstances', sources: [leaf] });
					const original = (await comp({ kind: 'Inspect' })).snapshot.instances[0];
					await comp({
						kind: 'ConfigureInstance',
						instance: instances[0].instance,
						definition: {
							...original.definition,
							transform: {
								position: [60, 45],
								scale: [1, 1],
								rotation_degrees: 0,
								pivot: [0.5, 0.5]
							}
						}
					});
				} else if (kind === 'Matrix') {
					const leaf = await solid(index, 'Mixed Matrix leaf');
					const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
					const snapshot = (await matrix({ kind: 'Inspect' })).snapshot;
					const layer = snapshot.items[0].id.id,
						column = snapshot.columns[0].id;
					await matrix({
						kind: 'ConfigureLayer',
						layer,
						settings: { position: [0, 0], size: { width: 120, height: 90 }, duration_ms: 250 }
					});
					await matrix({ kind: 'SetCells', assignments: [{ layer, column, source: leaf }] });
					await matrix({ kind: 'LaunchCell', layer, column, mode: 'Cut' });
				}
			}
			records.push({ kind, reference });
		}
		return {
			records,
			source: { name: source.name, pid: source.pid, acknowledgement: source.acknowledgement },
			close: source.close
		};
	} catch (error) {
		await source.close();
		throw error;
	}
}
