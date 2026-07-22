export interface SpatialBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

interface SpatialEntry<T> {
  value: T;
  order: number;
}

const normalizedBounds = (bounds: SpatialBounds): SpatialBounds => ({
  left: Math.min(bounds.left, bounds.right),
  top: Math.min(bounds.top, bounds.bottom),
  right: Math.max(bounds.left, bounds.right),
  bottom: Math.max(bounds.top, bounds.bottom),
});

/** Uniform-grid spatial index optimized for immutable, revision-keyed graph views. */
export class SpatialIndex<T> {
  readonly #cellSize: number;
  readonly #entries = new Map<string, SpatialEntry<T>>();
  readonly #buckets = new Map<string, string[]>();
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

  insert(id: string, bounds: SpatialBounds, value: T): void {
    if (this.#entries.has(id)) {
      throw new Error(`Spatial index already contains '${id}'.`);
    }
    const normalized = normalizedBounds(bounds);
    this.#entries.set(id, { value, order: this.#nextOrder++ });
    this.#forEachCell(normalized, (key) => {
      const bucket = this.#buckets.get(key);
      if (bucket) {
        bucket.push(id);
      } else {
        this.#buckets.set(key, [id]);
      }
    });
  }

  query(bounds: SpatialBounds): T[] {
    const ids = new Set<string>();
    this.#forEachCell(normalizedBounds(bounds), (key) => {
      for (const id of this.#buckets.get(key) ?? []) {
        ids.add(id);
      }
    });
    return [...ids]
      .map((id) => this.#entries.get(id))
      .filter((entry): entry is SpatialEntry<T> => entry !== undefined)
      .sort((left, right) => left.order - right.order)
      .map((entry) => entry.value);
  }

  #forEachCell(bounds: SpatialBounds, visit: (key: string) => void): void {
    const firstX = Math.floor(bounds.left / this.#cellSize);
    const lastX = Math.floor(bounds.right / this.#cellSize);
    const firstY = Math.floor(bounds.top / this.#cellSize);
    const lastY = Math.floor(bounds.bottom / this.#cellSize);
    for (let y = firstY; y <= lastY; y += 1) {
      for (let x = firstX; x <= lastX; x += 1) {
        visit(`${x}:${y}`);
      }
    }
  }
}
