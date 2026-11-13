import assert from 'node:assert/strict';

const kinds = ['software', 'hap'];

// Q06 HAP has two shared GPU publishers. ProRes shares HD; UHD uploads in the renderer.
export function assertQ06SourceUploads(codec, budget) {
	assert(
		['ProRes HQ', 'HAP', 'HAP Alpha', 'HAP Q'].includes(codec),
		'supported numbered Q06 codec required'
	);
	const hap = codec.startsWith('HAP');
	const kind = hap ? 'hap' : 'software';
	const allocation = hap ? 'hap_producers' : 'software_producers';
	const groups = budget.maximum_observed_classes;
	assert.equal(groups[allocation].allocations, hap ? 2 : 1, 'one publisher per shared source');
	// Each HAP retains one compressed input, three RGBA outputs and 16 constant bytes.
	const compressedBytesPerPixel = codec === 'HAP' ? 0.5 : 1;
	const hdBlocks = 1920 * 1080 * compressedBytesPerPixel;
	const expected = hap
		? [1920 * 1080, 3840 * 2160].reduce(
				(sum, pixels) => sum + pixels * (4 * 3 + compressedBytesPerPixel) + 16,
				0
			)
		: 1920 * 1080 * 4 * 3;
	assert.equal(groups[allocation].bytes, expected);
	assert.equal(budget.peak_admitted_bytes, groups[allocation].bytes);
	for (const [name, group] of Object.entries(groups))
		if (name !== allocation) assert.deepEqual(group, { allocations: 0, bytes: 0 });
	const uploads = budget.upload_budgets.filter((entry) => entry.uploads[kind].calls > 0);
	assert.equal(uploads.length, 1, 'one counted source budget');
	const total = uploads[0].uploads[kind];
	if (hap) {
		// Aggregate conservation only; the strict audit must still join each file/PTS payload.
		const uhdCalls = (total.bytes / hdBlocks - total.calls) / 3;
		assert(Number.isSafeInteger(uhdCalls) && uhdCalls > 0 && uhdCalls < total.calls);
	} else assert.equal(total.bytes, total.calls * 1920 * 1080 * 4);
	for (const entry of budget.upload_budgets)
		assert.deepEqual(entry.uploads[hap ? 'software' : 'hap'], { calls: 0, bytes: 0 });
}

function amount(value) {
	assert(Number.isSafeInteger(value) && value >= 0, 'safe nonnegative upload amount required');
	return value;
}
function read(uploads) {
	return Object.fromEntries(
		kinds.map((kind) => {
			const current = uploads[kind];
			assert.equal(current.overflowed, false, 'overflowed accounting is incomplete');
			const calls = amount(current.calls);
			const bytes = amount(current.bytes);
			assert.equal(calls === 0, bytes === 0);
			return [kind, { calls, bytes }];
		})
	);
}

// Snapshots from separate capture files can arrive out of order. One immutable budget ID
// and its monotonic cumulative calls select the latest complete pair without summing aliases.
export class SourceUploadInventory {
	#budgets = new Map();
	observe(source) {
		if (source.uploads == null) return false;
		assert.match(source.id, /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i);
		const current = read(source.uploads);
		const total = amount(current.software.calls + current.hap.calls);
		const previous = this.#budgets.get(source.id);
		if (previous) {
			const oldTotal = amount(previous.software.calls + previous.hap.calls);
			for (const kind of kinds) {
				for (const key of ['calls', 'bytes']) {
					assert(
						total >= oldTotal
							? current[kind][key] >= previous[kind][key]
							: current[kind][key] <= previous[kind][key],
						'upload counters must advance together within one budget'
					);
				}
			}
			if (total === oldTotal) assert.deepEqual(current, previous);
			if (total < oldTotal) return true;
		}
		this.#budgets.set(source.id, current);
		return true;
	}
	budgets() {
		return [...this.#budgets.entries()]
			.sort(([left], [right]) => left.localeCompare(right))
			.map(([id, uploads]) => ({ id, uploads: structuredClone(uploads) }));
	}
}

// Retain bounded startup facts independently of the host's rolling debug-log tail.
export class SourceBindingCollector {
	#records = new Map();
	#pending = new Map();
	#error;
	constructor(maxRecords = 4096, maxLine = 16384) {
		assert(Number.isSafeInteger(maxRecords) && maxRecords > 0);
		assert(Number.isSafeInteger(maxLine) && maxLine > 0);
		this.maxRecords = maxRecords;
		this.maxLine = maxLine;
	}
	#line(line) {
		const marker = '[mapgyver-video-source] ';
		if (!line.includes(marker)) return;
		try {
			assert(line.length <= this.maxLine, 'source binding line exceeds its bound');
			const record = JSON.parse(line.slice(line.indexOf(marker) + marker.length));
			for (const value of [
				record.stream,
				record.owner,
				record.file,
				record.reference?.media,
				record.reference?.channel
			])
				assert(typeof value === 'string' && value.length > 0, 'invalid source binding');
			const previous = this.#records.get(record.stream);
			if (previous) assert.deepEqual(previous, record, 'source binding changed within one stream');
			else {
				assert(this.#records.size < this.maxRecords, 'source binding capacity exhausted');
				this.#records.set(record.stream, record);
			}
		} catch (error) {
			this.#error ??= error;
		}
	}
	consume(text, stream = 'stderr') {
		if (!['stdout', 'stderr'].includes(stream)) {
			this.#error ??= new Error('unknown host log stream');
			return;
		}
		const state = this.#pending.get(stream) ?? { text: '', discard: false };
		for (const part of text.split(/(?<=\n)/)) {
			if (!state.discard) state.text += part;
			if (part.endsWith('\n')) {
				if (!state.discard) this.#line(state.text.trimEnd());
				state.text = '';
				state.discard = false;
			} else if (state.text.length > this.maxLine) {
				if (state.text.includes('[mapgyver-video-source] '))
					this.#error ??= new Error('source binding line exceeds its bound');
				state.text = '';
				state.discard = true;
			}
		}
		this.#pending.set(stream, state);
	}
	finish() {
		for (const state of this.#pending.values()) {
			if (!state.discard && state.text) this.#line(state.text);
			state.text = '';
		}
		if (this.#error) throw this.#error;
		return structuredClone([...this.#records.values()]);
	}
}
