import type { NodeId } from '../types';

const HASH_BITS = 5;
const HASH_BRANCHES = 1 << HASH_BITS;
const HASH_MASK = HASH_BRANCHES - 1;
const HASH_LEVELS = Math.ceil(32 / HASH_BITS);

interface TrieBranch<Value> {
	owner: symbol;
	children: Map<number, TrieBranch<Value> | TrieLeaf<Value>>;
}

interface TrieLeaf<Value> {
	owner: symbol;
	entries: Map<NodeId, Value>;
}

const hashNodeId = (key: NodeId): number => {
	const low = key >>> 0;
	const high = Math.floor(key / 0x1_0000_0000) >>> 0;
	let hash = (low ^ Math.imul(high, 0x9e37_79b1)) >>> 0;
	hash ^= hash >>> 16;
	hash = Math.imul(hash, 0x85eb_ca6b) >>> 0;
	hash ^= hash >>> 13;
	hash = Math.imul(hash, 0xc2b2_ae35) >>> 0;
	return (hash ^ (hash >>> 16)) >>> 0;
};

const trieSlot = (hash: number, level: number): number =>
	(hash >>> (level * HASH_BITS)) & HASH_MASK;

const isLeaf = <Value>(node: TrieBranch<Value> | TrieLeaf<Value>): node is TrieLeaf<Value> =>
	'entries' in node;

/**
 * Numeric-key persistent map used by published graph indexes.
 *
 * A fork shares the complete trie. Mutations use a fork-local ownership token and clone only the
 * branches on touched hash paths, so work is proportional to changed keys rather than prior map
 * size. Published versions are never mutated again by the graph store.
 */
export class VersionedNodeMap<Value> implements Map<NodeId, Value> {
	readonly [Symbol.toStringTag] = 'Map';
	#root: TrieBranch<Value>;
	#owner: symbol;
	#size: number;
	#copiedNodeCount = 0;

	constructor(entries?: Iterable<readonly [NodeId, Value]> | null) {
		this.#owner = Symbol('graph-index-version');
		this.#root = { owner: this.#owner, children: new Map() };
		this.#size = 0;
		if (entries) {
			for (const [key, value] of entries) {
				this.set(key, value);
			}
		}
	}

	get size(): number {
		return this.#size;
	}

	fork(): VersionedNodeMap<Value> {
		const fork = new VersionedNodeMap<Value>();
		fork.#root = this.#root;
		fork.#size = this.#size;
		return fork;
	}

	get(key: NodeId): Value | undefined {
		let branch = this.#root;
		const hash = hashNodeId(key);
		for (let level = 0; level < HASH_LEVELS; level += 1) {
			const child = branch.children.get(trieSlot(hash, level));
			if (!child || isLeaf(child)) {
				return undefined;
			}
			branch = child;
		}
		const leaf = branch.children.get(hash);
		return leaf && isLeaf(leaf) ? leaf.entries.get(key) : undefined;
	}

	has(key: NodeId): boolean {
		let branch = this.#root;
		const hash = hashNodeId(key);
		for (let level = 0; level < HASH_LEVELS; level += 1) {
			const child = branch.children.get(trieSlot(hash, level));
			if (!child || isLeaf(child)) {
				return false;
			}
			branch = child;
		}
		const leaf = branch.children.get(hash);
		return Boolean(leaf && isLeaf(leaf) && leaf.entries.has(key));
	}

	set(key: NodeId, value: Value): this {
		if (!Number.isSafeInteger(key) || key < 0) {
			throw new RangeError(`graph index keys must be non-negative safe integers, got ${key}`);
		}
		const existed = this.has(key);
		const hash = hashNodeId(key);
		this.#root = this.#ownedBranch(this.#root);
		let branch = this.#root;
		for (let level = 0; level < HASH_LEVELS; level += 1) {
			const slot = trieSlot(hash, level);
			const child = branch.children.get(slot);
			let next: TrieBranch<Value>;
			if (!child) {
				next = { owner: this.#owner, children: new Map() };
				this.#copiedNodeCount += 1;
			} else if (isLeaf(child)) {
				throw new Error('graph index trie shape is invalid');
			} else {
				next = this.#ownedBranch(child);
			}
			branch.children.set(slot, next);
			branch = next;
		}
		const existingLeaf = branch.children.get(hash);
		let leaf: TrieLeaf<Value>;
		if (!existingLeaf) {
			leaf = { owner: this.#owner, entries: new Map() };
			this.#copiedNodeCount += 1;
		} else if (!isLeaf(existingLeaf)) {
			throw new Error('graph index trie leaf shape is invalid');
		} else {
			leaf = this.#ownedLeaf(existingLeaf);
		}
		leaf.entries.set(key, value);
		branch.children.set(hash, leaf);
		if (!existed) {
			this.#size += 1;
		}
		return this;
	}

	getOrInsert(key: NodeId, defaultValue: Value): Value {
		if (this.has(key)) {
			return this.get(key) as Value;
		}
		this.set(key, defaultValue);
		return defaultValue;
	}

	getOrInsertComputed(key: NodeId, callback: (key: NodeId) => Value): Value {
		if (this.has(key)) {
			return this.get(key) as Value;
		}
		const value = callback(key);
		this.set(key, value);
		return value;
	}

	delete(key: NodeId): boolean {
		if (!this.has(key)) {
			return false;
		}
		const hash = hashNodeId(key);
		this.#root = this.#ownedBranch(this.#root);
		const path: Array<{ branch: TrieBranch<Value>; slot: number }> = [];
		let branch = this.#root;
		for (let level = 0; level < HASH_LEVELS; level += 1) {
			const slot = trieSlot(hash, level);
			const child = branch.children.get(slot);
			if (!child || isLeaf(child)) {
				throw new Error('graph index trie path disappeared during delete');
			}
			const next = this.#ownedBranch(child);
			branch.children.set(slot, next);
			path.push({ branch, slot });
			branch = next;
		}
		const existingLeaf = branch.children.get(hash);
		if (!existingLeaf || !isLeaf(existingLeaf)) {
			throw new Error('graph index trie leaf disappeared during delete');
		}
		const leaf = this.#ownedLeaf(existingLeaf);
		leaf.entries.delete(key);
		if (leaf.entries.size > 0) {
			branch.children.set(hash, leaf);
		} else {
			branch.children.delete(hash);
			for (let index = path.length - 1; index >= 0; index -= 1) {
				const entry = path[index];
				if (!entry || branch.children.size > 0) {
					break;
				}
				entry.branch.children.delete(entry.slot);
				branch = entry.branch;
			}
		}
		this.#size -= 1;
		return true;
	}

	clear(): void {
		if (this.#size === 0) {
			return;
		}
		this.#root = { owner: this.#owner, children: new Map() };
		this.#size = 0;
		this.#copiedNodeCount += 1;
	}

	*entries(): MapIterator<[NodeId, Value]> {
		for (const entry of this.#walk(this.#root)) {
			yield entry;
		}
	}

	*keys(): MapIterator<NodeId> {
		for (const [key] of this.#walk(this.#root)) {
			yield key;
		}
	}

	*values(): MapIterator<Value> {
		for (const [, value] of this.#walk(this.#root)) {
			yield value;
		}
	}

	[Symbol.iterator](): MapIterator<[NodeId, Value]> {
		return this.entries();
	}

	forEach(
		callbackfn: (value: Value, key: NodeId, map: Map<NodeId, Value>) => void,
		thisArg?: unknown
	): void {
		for (const [key, value] of this.#walk(this.#root)) {
			callbackfn.call(thisArg, value, key, this);
		}
	}

	mutationCopies(): number {
		return this.#copiedNodeCount;
	}

	#ownedBranch(branch: TrieBranch<Value>): TrieBranch<Value> {
		if (branch.owner === this.#owner) {
			return branch;
		}
		this.#copiedNodeCount += 1;
		return {
			owner: this.#owner,
			children: new Map(branch.children)
		};
	}

	#ownedLeaf(leaf: TrieLeaf<Value>): TrieLeaf<Value> {
		if (leaf.owner === this.#owner) {
			return leaf;
		}
		this.#copiedNodeCount += 1;
		return {
			owner: this.#owner,
			entries: new Map(leaf.entries)
		};
	}

	*#walk(branch: TrieBranch<Value>): Generator<[NodeId, Value]> {
		for (const child of branch.children.values()) {
			if (isLeaf(child)) {
				yield* child.entries;
			} else {
				yield* this.#walk(child);
			}
		}
	}
}

export const graphIndexMutationCopies = <Value>(
	index: ReadonlyMap<NodeId, Value>
): number | null => (index instanceof VersionedNodeMap ? index.mutationCopies() : null);
