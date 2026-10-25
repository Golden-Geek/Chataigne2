/** Bounded DOM membership changes; existing identities receive payload updates immediately. */
export class RetainedMounts<T> {
  readonly #key: (value: T) => string;
  #target: readonly T[] = [];
  #wanted = new Map<string, T>();
  readonly #mounted = new Map<string, T>();
  #values: readonly T[] = [];
  #pending = 0;
  #dirty = false;

  constructor(key: (value: T) => string) {
    this.#key = key;
  }
  get pending(): number {
    return this.#pending;
  }
  get values(): readonly T[] {
    return this.#values;
  }

  setTarget(values: readonly T[]): void {
    const wanted = new Map<string, T>();
    let shared = 0;
    for (const value of values) {
      const key = this.#key(value);
      if (wanted.has(key))
        throw new Error(`Duplicate mounted identity '${key}'.`);
      wanted.set(key, value);
      if (this.#mounted.has(key)) {
        shared += 1;
        if (this.#mounted.get(key) !== value) {
          this.#mounted.set(key, value);
          this.#dirty = true;
        }
      }
    }
    this.#dirty ||=
      values.length !== this.#target.length ||
      values.some(
        (value, index) => this.#key(value) !== this.#key(this.#target[index]),
      );
    this.#target = values;
    this.#wanted = wanted;
    this.#pending = wanted.size + this.#mounted.size - 2 * shared;
  }

  step(limit: number): void {
    if (!Number.isSafeInteger(limit) || limit < 0)
      throw new RangeError(
        "Mount work limit must be a nonnegative safe integer.",
      );
    let work = 0;
    for (const key of this.#mounted.keys()) {
      if (work >= limit) break;
      if (this.#wanted.has(key)) continue;
      this.#mounted.delete(key);
      work += 1;
    }
    for (const [key, value] of this.#wanted) {
      if (work >= limit) break;
      if (this.#mounted.has(key)) continue;
      this.#mounted.set(key, value);
      work += 1;
    }
    this.#pending -= work;
    if (work || this.#dirty) {
      const ordered = this.#target.filter((value) =>
        this.#mounted.has(this.#key(value)),
      );
      for (const [key, value] of this.#mounted)
        if (!this.#wanted.has(key)) ordered.push(value);
      this.#values = ordered;
      this.#dirty = false;
    }
  }
}
