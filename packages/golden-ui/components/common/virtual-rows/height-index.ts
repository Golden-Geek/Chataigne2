export type RowKey = string | number;

const positive = (value: number): number => {
	if (!Number.isFinite(value) || value <= 0)
		throw new RangeError('Row heights must be positive and finite.');
	return value;
};

/** Keyed measurements and a Fenwick prefix index keep scrolling/measurement work logarithmic. */
export class RowHeightIndex {
	#keys: readonly RowKey[] = [];
	#positions = new Map<RowKey, number>();
	#measured = new Map<RowKey, number>();
	#sizes = new Float64Array();
	#tree = new Float64Array(1);
	#estimate = 1;

	get length(): number {
		return this.#keys.length;
	}
	get total(): number {
		return this.offset(this.length);
	}
	keyAt(index: number): RowKey | undefined {
		return this.#keys[index];
	}
	indexOf(key: RowKey): number | undefined {
		return this.#positions.get(key);
	}

	synchronize(keys: readonly RowKey[], estimate: number): boolean {
		positive(estimate);
		if (
			estimate === this.#estimate &&
			keys.length === this.length &&
			keys.every((key, index) => key === this.#keys[index])
		)
			return false;
		const positions = new Map<RowKey, number>();
		keys.forEach((key, index) => {
			if (positions.has(key)) throw new Error(`Duplicate virtual row key '${key}'.`);
			positions.set(key, index);
		});
		this.#keys = [...keys];
		this.#positions = positions;
		this.#estimate = estimate;
		for (const key of this.#measured.keys()) if (!positions.has(key)) this.#measured.delete(key);
		this.#sizes = Float64Array.from(keys, (key) => this.#measured.get(key) ?? estimate);
		this.#tree = new Float64Array(keys.length + 1);
		for (let index = 1; index <= keys.length; index += 1) {
			this.#tree[index] += this.#sizes[index - 1];
			const parent = index + (index & -index);
			if (parent <= keys.length) this.#tree[parent] += this.#tree[index];
		}
		return true;
	}

	measure(key: RowKey, height: number): boolean {
		positive(height);
		const index = this.#positions.get(key);
		if (index === undefined || Math.abs(this.#sizes[index] - height) < 0.01) return false;
		const delta = height - this.#sizes[index];
		this.#sizes[index] = height;
		this.#measured.set(key, height);
		for (let entry = index + 1; entry <= this.length; entry += entry & -entry)
			this.#tree[entry] += delta;
		return true;
	}

	offset(index: number): number {
		let sum = 0;
		for (
			let entry = Math.min(this.length, Math.max(0, Math.floor(index)));
			entry > 0;
			entry -= entry & -entry
		)
			sum += this.#tree[entry];
		return sum;
	}

	rowAt(offset: number): number {
		if (!this.length) return -1;
		const target = Math.max(0, Number.isFinite(offset) ? offset : 0);
		let entry = 0,
			sum = 0;
		for (
			let step = 2 ** Math.floor(Math.log2(this.length));
			step > 0;
			step = Math.floor(step / 2)
		) {
			const next = entry + step;
			if (next <= this.length && sum + this.#tree[next] <= target) {
				entry = next;
				sum += this.#tree[next];
			}
		}
		return Math.min(entry, this.length - 1);
	}

	window(top: number, height: number, overscan = 8): { start: number; end: number } {
		if (!this.length) return { start: 0, end: 0 };
		const viewport = Math.max(0, Number.isFinite(height) ? height : 0);
		const scroll = Math.min(Math.max(0, top || 0), Math.max(0, this.total - viewport));
		return {
			start: Math.max(0, this.rowAt(scroll) - overscan),
			end: Math.min(this.length, this.rowAt(scroll + viewport) + 1 + overscan)
		};
	}
}
