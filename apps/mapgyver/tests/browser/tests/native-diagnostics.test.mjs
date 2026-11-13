import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { inspectNativeDiagnostics } from '../native-diagnostics.mjs';

function fixture() {
	const source = {
		limit_bytes: 64,
		retained_bytes: 8,
		peak_bytes: 8,
		decoder_pools: { allocations: 0, bytes: 0 },
		hardware_producers: { allocations: 0, bytes: 0 },
		hap_producers: { allocations: 0, bytes: 0 },
		software_producers: { allocations: 1, bytes: 8 }
	};
	const snapshot = {
		kind: 'snapshot',
		process: { process_id: 123, memory: { private_bytes: 1 } },
		process_error: null,
		native: { initialized: true },
		allocations: { plan: { live: 1 }, owned_texel_bytes: 8 },
		source_gpu: source
	};
	const retired = structuredClone(snapshot);
	retired.native.initialized = false;
	retired.allocations = { plan: { live: 0 }, owned_texel_bytes: 0 };
	retired.source_gpu.retained_bytes = 0;
	retired.source_gpu.software_producers = { allocations: 0, bytes: 0 };
	return [
		{ kind: 'capture', format_version: 1, process_id: 123, qpc_frequency: 1000, max_bytes: 65536 },
		...['FrameCpu', 'Output', 'Gpu'].map((kind) => ({ kind: 'diagnostic', event: { [kind]: {} } })),
		{
			kind: 'diagnostic',
			event: {
				SourceUse: {
					frame: { pts_nanos: 0 },
					software_upload: { stream: 'fixture', sequence: 1, bytes: 8 }
				}
			}
		},
		snapshot,
		retired,
		{
			kind: 'capture_end',
			summary: {
				events: 4,
				snapshots: 2,
				duration_limit: false,
				diagnostics: {
					enabled: true,
					queued: 0,
					dropped_full: 0,
					clock_failures: 0,
					written: 4,
					drained: 4
				}
			}
		}
	];
}

test('source-budget qualification rejects missing, incoherent and unretired accounting', async (t) => {
	const directory = await mkdtemp(path.join(os.tmpdir(), 'mapgyver-source-counter-tests-'));
	const previous = process.env.MAPGYVER_NATIVE_DIAGNOSTICS;
	const inspect = () =>
		inspectNativeDiagnostics(123, '', { requireVideoFrames: true, requireSourceBudget: true });
	try {
		await t.test('an absent capture cannot qualify source ownership', async () => {
			delete process.env.MAPGYVER_NATIVE_DIAGNOSTICS;
			await assert.rejects(inspect(), /actual capture/);
		});
		process.env.MAPGYVER_NATIVE_DIAGNOSTICS = directory;
		const save = (rows) =>
			writeFile(
				path.join(directory, 'native-123-fixture.jsonl'),
				rows.map(JSON.stringify).join('\n')
			);
		await t.test('real category totals retain their maximum after zero retirement', async () => {
			await save(fixture());
			const result = await inspect();
			assert.equal(result.sourceGpu.snapshots, 2);
			assert.equal(result.sourceGpu.peak_admitted_bytes, 8);
			assert.deepEqual(result.sourceGpu.maximum_observed_classes.software_producers, {
				allocations: 1,
				bytes: 8
			});
		});
		for (const [name, mutate] of [
			['missing snapshot', (rows) => delete rows[5].source_gpu],
			['unbalanced categories', (rows) => (rows[5].source_gpu.retained_bytes = 7)],
			['noninteger bytes', (rows) => (rows[5].source_gpu.software_producers.bytes = 7.5)],
			['missing owner count', (rows) => (rows[5].source_gpu.software_producers.allocations = 0)],
			['ceiling exceeded', (rows) => (rows[5].source_gpu.limit_bytes = 7)],
			['peak lower than current', (rows) => (rows[5].source_gpu.peak_bytes = 7)],
			['decreasing lifetime peak', (rows) => (rows[6].source_gpu.peak_bytes = 7)],
			['unretired source', (rows) => (rows[6].source_gpu = structuredClone(rows[5].source_gpu))]
		]) {
			await t.test(name, async () => {
				const rows = fixture();
				mutate(rows);
				await save(rows);
				await assert.rejects(inspect());
			});
		}
	} finally {
		if (previous === undefined) delete process.env.MAPGYVER_NATIVE_DIAGNOSTICS;
		else process.env.MAPGYVER_NATIVE_DIAGNOSTICS = previous;
		await rm(directory, { recursive: true, force: true });
	}
});
