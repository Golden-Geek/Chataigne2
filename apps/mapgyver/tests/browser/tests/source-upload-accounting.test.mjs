import assert from 'node:assert/strict';
import test from 'node:test';
import {
	assertQ06SourceUploads,
	SourceStartupCollector,
	SourceUploadInventory
} from '../source-upload-accounting.mjs';

const first = '76f39a00-2bfb-4567-a28b-006cf031a1ae';
const second = '135d4930-c077-4fd6-ae55-a66d09477bb5';

test('resident Q06 requires actual hardware pools and rejects CPU fallback or unmeasured uploads', () => {
	for (const codec of ['H.264 High', 'HEVC Main']) {
		const budget = {
			maximum_observed_classes: {
				decoder_pools: { allocations: 2, bytes: 80000000 },
				hardware_producers: { allocations: 2, bytes: 20000000 },
				software_producers: { allocations: 0, bytes: 0 },
				hap_producers: { allocations: 0, bytes: 0 }
			},
			upload_budgets: [
				{ uploads: { software: { calls: 0, bytes: 0 }, hap: { calls: 0, bytes: 0 } } }
			]
		};
		assertQ06SourceUploads(codec, budget);
		for (const mutate of [
			(value) => value.maximum_observed_classes.decoder_pools.allocations--,
			(value) => value.maximum_observed_classes.hardware_producers.allocations--,
			(value) => (value.maximum_observed_classes.hardware_producers.bytes = 0),
			(value) => value.maximum_observed_classes.software_producers.allocations++,
			(value) => value.maximum_observed_classes.hap_producers.bytes++,
			(value) => (value.upload_budgets.length = 0),
			(value) => value.upload_budgets[0].uploads.software.calls++,
			(value) => value.upload_budgets[0].uploads.hap.bytes++
		]) {
			const wrong = structuredClone(budget);
			mutate(wrong);
			assert.throws(() => assertQ06SourceUploads(codec, wrong));
		}
	}
});

test('Q06 checks codec-specific payloads and rejects absent, expanded or mixed HAP uploads', () => {
	for (const codec of ['ProRes HQ', 'ProRes 4444', 'HAP', 'HAP Alpha', 'HAP Q']) {
		const hap = codec.startsWith('HAP');
		const kind = hap ? 'hap' : 'software';
		const allocation = hap ? 'hap_producers' : 'software_producers';
		const bytes = hap ? (codec === 'HAP' ? 129600032 : 134784032) : 1920 * 1080 * 4 * 3;
		const budget = {
			peak_admitted_bytes: bytes,
			maximum_observed_classes: Object.fromEntries(
				['decoder_pools', 'hardware_producers', 'hap_producers', 'software_producers'].map(
					(name) => [
						name,
						{
							allocations: name === allocation ? (hap ? 2 : 1) : 0,
							bytes: name === allocation ? bytes : 0
						}
					]
				)
			),
			upload_budgets: [
				{ uploads: { software: { calls: 0, bytes: 0 }, hap: { calls: 0, bytes: 0 } } }
			]
		};
		budget.upload_budgets[0].uploads[kind] = {
			calls: 2,
			bytes: 1920 * 1080 * (hap ? (codec === 'HAP' ? 2.5 : 5) : 8)
		};
		assertQ06SourceUploads(codec, budget);
		for (const mutate of [
			(value) => (value.upload_budgets.length = 0),
			(value) => value.upload_budgets[0].uploads[kind].bytes++,
			(value) => value.upload_budgets[0].uploads[hap ? 'software' : 'hap'].calls++,
			(value) => value.maximum_observed_classes[allocation].allocations++,
			(value) => value.peak_admitted_bytes++
		]) {
			const wrong = structuredClone(budget);
			mutate(wrong);
			assert.throws(() => assertQ06SourceUploads(codec, wrong));
		}
	}
	assert.throws(() => assertQ06SourceUploads('unknown', {}));
});

function binding(stream = first) {
	return { stream, owner: first, file: 'é😀.mp4', reference: { media: first, channel: stream } };
}

test('source bindings survive fragmented/interleaved streams and rolling debug-log loss', () => {
	const collector = new SourceStartupCollector();
	const line = '[mapgyver-video-source] ' + JSON.stringify(binding()) + '\n';
	collector.consume(line.slice(0, 17));
	collector.consume('unrelated output\n', 'stdout');
	collector.consume(line.slice(17));
	collector.consume('other logs\n'.repeat(10000));
	collector.consume(line);
	const records = collector.finish().bindings;
	assert.deepEqual(records, [binding()]);
	records[0].file = 'mutated';
	assert.deepEqual(collector.finish().bindings, [binding()]);
});

test('first CPU layout facts survive fragmentation, deduplicate mutable samples and share the startup bound', () => {
	const cpu = {
		...binding(),
		reservation_bytes: 1024,
		reader: { decoded_storage: { limit_bytes: 512 } }
	};
	delete cpu.stream;
	const line = '[mapgyver-video-cpu] ' + JSON.stringify(cpu) + '\n';
	const collector = new SourceStartupCollector();
	collector.consume(line.slice(0, 16));
	collector.consume('other stream\n', 'stdout');
	collector.consume(line.slice(16));
	collector.consume('[mapgyver-video-cpu] ' + JSON.stringify({ ...cpu, reader: {} }) + '\n');
	const facts = collector.finish();
	assert.deepEqual(facts.cpuLayouts, [cpu]);
	facts.cpuLayouts[0].reader = {};
	assert.deepEqual(collector.finish().cpuLayouts, [cpu]);
	const bounded = new SourceStartupCollector(1);
	bounded.consume(line);
	bounded.consume('[mapgyver-video-source] ' + JSON.stringify(binding()) + '\n');
	assert.throws(() => bounded.finish());
	for (const malformed of ['{}', JSON.stringify({ ...cpu, reservation_bytes: -1 })]) {
		const invalid = new SourceStartupCollector();
		invalid.consume('[mapgyver-video-cpu] ' + malformed + '\n');
		assert.throws(() => invalid.finish());
	}
	const tooLong = new SourceStartupCollector(1, 30);
	tooLong.consume(line);
	assert.throws(() => tooLong.finish());
});

test('source binding capacity, malformed metadata and changed identity fail explicitly', () => {
	for (const rows of [
		[binding(), binding(second)],
		[binding(), { ...binding(), file: 'other' }],
		[{ stream: first }]
	]) {
		const collector = new SourceStartupCollector(1);
		for (const row of rows)
			collector.consume('[mapgyver-video-source] ' + JSON.stringify(row) + '\n');
		assert.throws(() => collector.finish());
	}
	const collector = new SourceStartupCollector(1, 30);
	collector.consume('[mapgyver-video-source] ' + 'x'.repeat(100));
	assert.throws(() => collector.finish());
});
function source(id, softwareCalls, hapCalls) {
	return {
		id,
		uploads: {
			software: { calls: softwareCalls, bytes: softwareCalls * 8, overflowed: false },
			hap: { calls: hapCalls, bytes: hapCalls * 32, overflowed: false }
		}
	};
}
test('unmeasured snapshots stay unknown', () => {
	const inventory = new SourceUploadInventory();
	assert.equal(inventory.observe({}), false);
	assert.equal(inventory.observe({ uploads: null }), false);
	assert.deepEqual(inventory.budgets(), []);
});
test('repeated, older and independently captured aliases retain one lifetime total', () => {
	const inventory = new SourceUploadInventory();
	for (const row of [source(first, 3, 2), source(first, 1, 0), source(first, 3, 2)])
		assert(inventory.observe(row));
	assert.deepEqual(inventory.budgets(), [
		{ id: first, uploads: { software: { calls: 3, bytes: 24 }, hap: { calls: 2, bytes: 64 } } }
	]);
	assert(inventory.observe(source(first, 4, 2)));
	assert.equal(inventory.budgets()[0].uploads.software.calls, 4);
});
test('new budget lifetimes do not inherit counters; returned data cannot change the inventory', () => {
	const inventory = new SourceUploadInventory();
	inventory.observe(source(first, 10, 0));
	inventory.observe(source(second, 1, 1));
	const rows = inventory.budgets();
	assert.equal(rows.length, 2);
	rows[0].uploads.software.calls = 999;
	assert.equal(inventory.budgets().find((row) => row.id === first).uploads.software.calls, 10);
});
test('incoherent equal-count bytes and partial counter reversals are rejected', () => {
	const inventory = new SourceUploadInventory();
	inventory.observe(source(first, 3, 2));
	assert.throws(() => inventory.observe(source(first, 4, 1)));
	assert.throws(() => inventory.observe(source(first, 2, 4)));
	const row = source(first, 3, 2);
	row.uploads.software.bytes++;
	assert.throws(() => inventory.observe(row));
});
test('missing identity, overflow, negative, unsafe and inconsistent counts cannot become evidence', () => {
	for (const mutate of [
		(row) => delete row.id,
		(row) => (row.uploads.software.overflowed = true),
		(row) => (row.uploads.hap.bytes = -1),
		(row) => (row.uploads.software.calls = Number.MAX_SAFE_INTEGER + 1),
		(row) => (row.uploads.hap.calls = 0)
	]) {
		const row = source(first, 1, 1);
		mutate(row);
		assert.throws(() => new SourceUploadInventory().observe(row));
	}
});
