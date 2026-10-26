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
	hash: number;
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
 * size. Hash branches exist only where keys share a prefix, rather than allocating seven levels
 * per key. Full hash collisions share a leaf keyed by complete node identity.
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
		// Both maps must copy shared paths, including when the original is edited after forking.
		this.#owner = Symbol('graph-index-version');
		return fork;
	}

	get(key: NodeId): Value | undefined {
		return this.#leaf(key)?.entries.get(key);
	}

	#leaf(key: NodeId): TrieLeaf<Value> | undefined {
		let branch = this.#root;
		const hash = hashNodeId(key);
		for (let level = 0; level < HASH_LEVELS; level += 1) {
			const child = branch.children.get(trieSlot(hash, level));
			if (!child) return undefined;
			if (isLeaf(child)) return child.hash === hash ? child : undefined;
			branch = child;
		}
		throw new Error('graph index trie exceeds hash depth');
	}

	has(key: NodeId): boolean {
		return this.#leaf(key)?.entries.has(key) ?? false;
	}

	set(key: NodeId, value: Value): this {
		if (!Number.isSafeInteger(key) || key < 0) {
			throw new RangeError(`graph index keys must be non-negative safe integers, got ${key}`);
		}
		const existed = this.has(key);
		const hash = hashNodeId(key);
		this.#root = this.#ownedBranch(this.#root);
		this.#insert(this.#root, key, value, hash, 0);
		if (!existed) {
			this.#size += 1;
		}
		return this;
	}

	#insert(branch: TrieBranch<Value>, key: NodeId, value: Value, hash: number, level: number): void {
		const slot = trieSlot(hash, level);
		const child = branch.children.get(slot);
		if (!child) {
			branch.children.set(slot, { owner: this.#owner, hash, entries: new Map([[key, value]]) });
			this.#copiedNodeCount += 1;
			return;
		}
		if (isLeaf(child) && child.hash === hash) {
			const leaf = this.#ownedLeaf(child);
			leaf.entries.set(key, value);
			branch.children.set(slot, leaf);
			return;
		}
		if (level + 1 >= HASH_LEVELS) throw new Error('graph index trie exceeds hash depth');
		let next: TrieBranch<Value>;
		if (isLeaf(child)) {
			// Keep a key in a leaf until another hash needs the same prefix.
			next = { owner: this.#owner, children: new Map([[trieSlot(child.hash, level + 1), child]]) };
			this.#copiedNodeCount += 1;
		} else next = this.#ownedBranch(child);
		branch.children.set(slot, next);
		this.#insert(next, key, value, hash, level + 1);
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
			if (!child) {
				throw new Error('graph index trie path disappeared during delete');
			}
			if (isLeaf(child)) {
				const leaf = this.#ownedLeaf(child);
				leaf.entries.delete(key);
				if (leaf.entries.size) branch.children.set(slot, leaf);
				else branch.children.delete(slot);
				break;
			}
			const next = this.#ownedBranch(child);
			branch.children.set(slot, next);
			path.push({ branch, slot });
			branch = next;
		}
		for (let index = path.length - 1; index >= 0; index -= 1) {
			const entry = path[index];
			if (!entry || branch.children.size > 0) {
				break;
			}
			entry.branch.children.delete(entry.slot);
			branch = entry.branch;
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

	/** Visit changed keys only, skipping shared trie branches between published versions. */
	forEachChange(
		previous: VersionedNodeMap<Value>,
		visit: (key: NodeId, value: Value | undefined, previousValue: Value | undefined) => void
	): void {
		const walk = (
			before: TrieBranch<Value> | TrieLeaf<Value> | undefined,
			after: TrieBranch<Value> | TrieLeaf<Value> | undefined
		): void => {
			if (before === after) return;
			if ((before && isLeaf(before)) || (after && isLeaf(after))) {
				const oldEntries = before
					? isLeaf(before)
						? before.entries
						: new Map(this.#walk(before))
					: new Map<NodeId, Value>();
				const entries = after
					? isLeaf(after)
						? after.entries
						: new Map(this.#walk(after))
					: new Map<NodeId, Value>();
				const keys = new Set([...oldEntries.keys(), ...entries.keys()]);
				for (const key of keys) {
					const oldValue = oldEntries.get(key);
					const value = entries.get(key);
					if (oldEntries.has(key) !== entries.has(key) || !Object.is(oldValue, value))
						visit(key, value, oldValue);
				}
				return;
			}
			const keys = new Set([...(before?.children.keys() ?? []), ...(after?.children.keys() ?? [])]);
			for (const key of keys) walk(before?.children.get(key), after?.children.get(key));
		};
		walk(previous.#root, this.#root);
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
			hash: leaf.hash,
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
