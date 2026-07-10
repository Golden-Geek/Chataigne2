export class MultiSeriesRingBuffer {
	readonly capacity: number;
	readonly seriesCount: number;

	private head = 0;
	private size = 0;
	private readonly timestamps: Float64Array;
	private readonly series: Float64Array[];

	constructor(seriesCount: number, capacity = 16384) {
		this.capacity = Math.max(16, Math.floor(capacity));
		this.seriesCount = Math.max(1, Math.floor(seriesCount));
		this.timestamps = new Float64Array(this.capacity);
		this.series = Array.from({ length: this.seriesCount }, () => new Float64Array(this.capacity));
	}

	get length(): number {
		return this.size;
	}

	clear(): void {
		this.head = 0;
		this.size = 0;
	}

	push(timeMs: number, values: ReadonlyArray<number>): void {
		const writeIndex = this.head;
		this.timestamps[writeIndex] = timeMs;

		for (let channelIndex = 0; channelIndex < this.seriesCount; channelIndex += 1) {
			const nextValue = Number(values[channelIndex] ?? 0);
			this.series[channelIndex][writeIndex] = Number.isFinite(nextValue) ? nextValue : 0;
		}

		this.head = (writeIndex + 1) % this.capacity;
		if (this.size < this.capacity) {
			this.size += 1;
		}
	}

	valueAt(channelIndex: number, sampleIndex: number): number {
		const series = this.series[channelIndex];
		if (!series) {
			return 0;
		}
		return series[sampleIndex] ?? 0;
	}

	lastValue(channelIndex: number): number | null {
		if (this.size === 0) {
			return null;
		}
		const series = this.series[channelIndex];
		if (!series) {
			return null;
		}
		const lastIndex = (this.head - 1 + this.capacity) % this.capacity;
		return series[lastIndex] ?? null;
	}

	countSince(minTimeMs: number): number {
		let count = 0;
		this.forEachSince(minTimeMs, () => {
			count += 1;
		});
		return count;
	}

	findLatestIndexAtOrBefore(timeMs: number): number | null {
		let result: number | null = null;
		this.forEachInOrder((sampleIndex, sampleTimeMs) => {
			if (sampleTimeMs <= timeMs) {
				result = sampleIndex;
			}
		});
		return result;
	}

	minMaxSince(minTimeMs: number): { min: number; max: number } | null {
		let min = Number.POSITIVE_INFINITY;
		let max = Number.NEGATIVE_INFINITY;
		let hasValue = false;

		this.forEachSince(minTimeMs, (sampleIndex) => {
			for (let channelIndex = 0; channelIndex < this.seriesCount; channelIndex += 1) {
				const value = this.valueAt(channelIndex, sampleIndex);
				if (!Number.isFinite(value)) {
					continue;
				}
				hasValue = true;
				if (value < min) {
					min = value;
				}
				if (value > max) {
					max = value;
				}
			}
		});

		return hasValue ? { min, max } : null;
	}

	forEachSince(
		minTimeMs: number,
		callback: (sampleIndex: number, sampleTimeMs: number) => void
	): void {
		this.forEachInOrder((sampleIndex, sampleTimeMs) => {
			if (sampleTimeMs < minTimeMs) {
				return;
			}
			callback(sampleIndex, sampleTimeMs);
		});
	}

	private forEachInOrder(callback: (sampleIndex: number, sampleTimeMs: number) => void): void {
		if (this.size === 0) {
			return;
		}

		const oldestIndex = (this.head - this.size + this.capacity) % this.capacity;
		for (let offset = 0; offset < this.size; offset += 1) {
			const sampleIndex = (oldestIndex + offset) % this.capacity;
			callback(sampleIndex, this.timestamps[sampleIndex] ?? 0);
		}
	}
}

export class TimeRingBuffer<T> {
	private readonly capacity: number;
	private head = 0;
	private size = 0;
	private readonly timestamps: Float64Array;
	private readonly values: Array<T | null>;

	constructor(capacity = 4096) {
		this.capacity = Math.max(16, Math.floor(capacity));
		this.timestamps = new Float64Array(this.capacity);
		this.values = Array.from({ length: this.capacity }, () => null);
	}

	get length(): number {
		return this.size;
	}

	clear(): void {
		this.head = 0;
		this.size = 0;
	}

	push(timeMs: number, value: T): void {
		const writeIndex = this.head;
		this.timestamps[writeIndex] = timeMs;
		this.values[writeIndex] = value;

		this.head = (writeIndex + 1) % this.capacity;
		if (this.size < this.capacity) {
			this.size += 1;
		}
	}

	forEachSince(minTimeMs: number, callback: (sampleTimeMs: number, value: T) => void): void {
		this.forEachInOrder((sampleTimeMs, value) => {
			if (sampleTimeMs < minTimeMs) {
				return;
			}
			callback(sampleTimeMs, value);
		});
	}

	forEachRecent(limit: number, callback: (sampleTimeMs: number, value: T) => void): void {
		if (this.size === 0) {
			return;
		}

		const count = Math.max(0, Math.min(this.size, Math.floor(limit)));
		for (let offset = 0; offset < count; offset += 1) {
			const sampleIndex = (this.head - 1 - offset + this.capacity) % this.capacity;
			const value = this.values[sampleIndex];
			if (value === null) {
				continue;
			}
			callback(this.timestamps[sampleIndex] ?? 0, value);
		}
	}

	private forEachInOrder(callback: (sampleTimeMs: number, value: T) => void): void {
		if (this.size === 0) {
			return;
		}

		const oldestIndex = (this.head - this.size + this.capacity) % this.capacity;
		for (let offset = 0; offset < this.size; offset += 1) {
			const sampleIndex = (oldestIndex + offset) % this.capacity;
			const value = this.values[sampleIndex];
			if (value === null) {
				continue;
			}
			callback(this.timestamps[sampleIndex] ?? 0, value);
		}
	}
}
