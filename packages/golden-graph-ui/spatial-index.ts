export interface SpatialBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface LimitedSpatialQuery<T> {
  values: T[];
  complete: boolean;
}

interface SpatialEntry<T> {
  value: T;
  order: number;
  bounds: SpatialBounds;
  cells: string[] | null;
}

const MAXIMUM_CELLS = 4096;

const normalizedBounds = (bounds: SpatialBounds): SpatialBounds => {
  if (
    ![bounds.left, bounds.top, bounds.right, bounds.bottom].every(
      Number.isFinite,
    )
  ) {
    throw new RangeError("Spatial bounds must contain finite coordinates.");
  }
  return {
    left: Math.min(bounds.left, bounds.right),
    top: Math.min(bounds.top, bounds.bottom),
    right: Math.max(bounds.left, bounds.right),
    bottom: Math.max(bounds.top, bounds.bottom),
  };
};

const intersects = (left: SpatialBounds, right: SpatialBounds): boolean =>
  left.left <= right.right &&
  left.right >= right.left &&
  left.top <= right.bottom &&
  left.bottom >= right.top;

/** Retained spatial membership with stable document order and bounded grid expansion. */
export class SpatialIndex<T> {
  readonly #cellSize: number;
  readonly #entries = new Map<string, SpatialEntry<T>>();
  readonly #buckets = new Map<string, Set<string>>();
  readonly #oversized = new Set<string>();
  #nextOrder = 0;

  constructor(cellSize = 32) {
    if (!Number.isFinite(cellSize) || cellSize <= 0) {
      throw new RangeError(
        "Spatial index cell size must be finite and greater than zero.",
      );
    }
    this.#cellSize = cellSize;
  }

  get size(): number {
    return this.#entries.size;
  }

  get(id: string): T | undefined {
    return this.#entries.get(id)?.value;
  }

  insert(id: string, bounds: SpatialBounds, value: T): void {
    if (this.#entries.has(id)) {
      throw new Error(`Spatial index already contains '${id}'.`);
    }
    const normalized = normalizedBounds(bounds);
    const entry = {
      value,
      order: this.#nextOrder++,
      bounds: normalized,
      cells: this.#cells(normalized),
    };
    this.#entries.set(id, entry);
    this.#attach(id, entry);
  }

  update(id: string, bounds: SpatialBounds, value: T): void {
    const entry = this.#require(id);
    const normalized = normalizedBounds(bounds);
    const cells = this.#cells(normalized);
    this.#detach(id, entry);
    entry.value = value;
    entry.bounds = normalized;
    entry.cells = cells;
    this.#attach(id, entry);
  }

  /** Payload-only replacement leaves geometry, buckets and authored order untouched. */
  setValue(id: string, value: T): void {
    this.#require(id).value = value;
  }

  remove(id: string): boolean {
    const entry = this.#entries.get(id);
    if (!entry) return false;
    this.#detach(id, entry);
    this.#entries.delete(id);
    return true;
  }

  query(bounds: SpatialBounds): T[] {
    const normalized = normalizedBounds(bounds);
    const cells = this.#cells(normalized);
    // Large queries visit retained entries instead of expanding an unbounded grid.
    const ids =
      cells === null ? new Set(this.#entries.keys()) : new Set(this.#oversized);
    for (const key of cells ?? []) {
      for (const id of this.#buckets.get(key) ?? []) {
        ids.add(id);
      }
    }
    return [...ids]
      .map((id) => this.#entries.get(id))
      .filter(
        (entry): entry is SpatialEntry<T> =>
          entry !== undefined && intersects(entry.bounds, normalized),
      )
      .sort((left, right) => left.order - right.order)
      .map((entry) => entry.value);
  }

  /** Unordered query with a strict candidate-visit budget, including coarse false positives. */
  queryLimited(
    bounds: SpatialBounds,
    maximumVisits: number,
  ): LimitedSpatialQuery<T> {
    if (!Number.isSafeInteger(maximumVisits) || maximumVisits < 0)
      throw new RangeError(
        "Spatial visit budget must be a nonnegative safe integer.",
      );
    const normalized = normalizedBounds(bounds);
    const cells = this.#cells(normalized);
    const groups =
      cells === null
        ? [this.#entries.keys()]
        : [
            this.#oversized.values(),
            ...cells.map(
              (key) =>
                this.#buckets.get(key)?.values() ?? [][Symbol.iterator](),
            ),
          ];
    const seen = new Set<string>();
    const values: T[] = [];
    let visits = 0;
    for (const group of groups)
      for (const id of group) {
        if (visits >= maximumVisits) return { values, complete: false };
        visits += 1;
        if (seen.has(id)) continue;
        seen.add(id);
        const entry = this.#entries.get(id)!;
        if (intersects(entry.bounds, normalized)) values.push(entry.value);
      }
    return { values, complete: true };
  }

  #require(id: string): SpatialEntry<T> {
    const entry = this.#entries.get(id);
    if (!entry) throw new Error(`Spatial index has no '${id}'.`);
    return entry;
  }

  #attach(id: string, entry: SpatialEntry<T>): void {
    if (entry.cells === null) {
      this.#oversized.add(id);
      return;
    }
    for (const key of entry.cells) {
      let bucket = this.#buckets.get(key);
      if (!bucket) {
        bucket = new Set();
        this.#buckets.set(key, bucket);
      }
      bucket.add(id);
    }
  }

  #detach(id: string, entry: SpatialEntry<T>): void {
    if (entry.cells === null) {
      this.#oversized.delete(id);
      return;
    }
    for (const key of entry.cells) {
      const bucket = this.#buckets.get(key);
      if (bucket?.delete(id) && bucket.size === 0) this.#buckets.delete(key);
    }
  }

  #cells(bounds: SpatialBounds): string[] | null {
    const firstX = Math.floor(bounds.left / this.#cellSize);
    const lastX = Math.floor(bounds.right / this.#cellSize);
    const firstY = Math.floor(bounds.top / this.#cellSize);
    const lastY = Math.floor(bounds.bottom / this.#cellSize);
    if (
      ![firstX, lastX, firstY, lastY].every(Number.isSafeInteger) ||
      (lastX - firstX + 1) * (lastY - firstY + 1) > MAXIMUM_CELLS
    )
      return null;
    const cells: string[] = [];
    for (let y = firstY; y <= lastY; y += 1) {
      for (let x = firstX; x <= lastX; x += 1) {
        cells.push(`${x}:${y}`);
      }
    }
    return cells;
  }
}
