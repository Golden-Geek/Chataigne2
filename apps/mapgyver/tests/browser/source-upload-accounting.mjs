import assert from 'node:assert/strict';

const kinds = ['software', 'hap'];
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
