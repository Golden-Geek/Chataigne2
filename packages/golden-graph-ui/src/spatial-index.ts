import type { GraphNodeId, Rect } from "./types";

export class SpatialIndex {
  readonly #cells = new Map<string, Set<GraphNodeId>>();
  readonly #itemCells = new Map<GraphNodeId, readonly string[]>();
  readonly #rectangles = new Map<GraphNodeId, Rect>();

  constructor(readonly cellSize = 256) {
    if (!Number.isFinite(cellSize) || cellSize <= 0) {
      throw new Error("spatial index cell size must be finite and positive");
    }
  }

  upsert(id: GraphNodeId, rectangle: Rect): void {
    this.remove(id);
    assertRect(rectangle);
    const keys = this.#cellKeys(rectangle);
    this.#rectangles.set(id, rectangle);
    this.#itemCells.set(id, keys);
    for (const key of keys) {
      let cell = this.#cells.get(key);
      if (!cell) {
        cell = new Set();
        this.#cells.set(key, cell);
      }
      cell.add(id);
    }
  }

  remove(id: GraphNodeId): void {
    for (const key of this.#itemCells.get(id) ?? []) {
      const cell = this.#cells.get(key);
      cell?.delete(id);
      if (cell?.size === 0) this.#cells.delete(key);
    }
    this.#itemCells.delete(id);
    this.#rectangles.delete(id);
  }

  query(area: Rect): GraphNodeId[] {
    assertRect(area);
    const candidates = new Set<GraphNodeId>();
    for (const key of this.#cellKeys(area)) {
      for (const id of this.#cells.get(key) ?? []) candidates.add(id);
    }
    return [...candidates]
      .filter((id) => intersects(this.#rectangles.get(id)!, area))
      .sort();
  }

  hitTest(x: number, y: number): GraphNodeId[] {
    return this.query({ x, y, width: 0, height: 0 });
  }

  #cellKeys(rectangle: Rect): string[] {
    const left = Math.floor(rectangle.x / this.cellSize);
    const top = Math.floor(rectangle.y / this.cellSize);
    const right = Math.floor((rectangle.x + rectangle.width) / this.cellSize);
    const bottom = Math.floor((rectangle.y + rectangle.height) / this.cellSize);
    const keys: string[] = [];
    for (let y = top; y <= bottom; y += 1) {
      for (let x = left; x <= right; x += 1) keys.push(`${x}:${y}`);
    }
    return keys;
  }
}

function assertRect(rectangle: Rect): void {
  if (
    !Number.isFinite(rectangle.x) ||
    !Number.isFinite(rectangle.y) ||
    !Number.isFinite(rectangle.width) ||
    !Number.isFinite(rectangle.height) ||
    rectangle.width < 0 ||
    rectangle.height < 0
  ) {
    throw new Error("graph geometry must be finite and non-negative");
  }
}

function intersects(left: Rect, right: Rect): boolean {
  return !(
    left.x + left.width < right.x ||
    right.x + right.width < left.x ||
    left.y + left.height < right.y ||
    right.y + right.height < left.y
  );
}
