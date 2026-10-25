import type { NodeId } from '../../../../types';
import type { FreeLayoutRect } from './contracts';
import { rectsIntersect } from './geometry';

export interface FreeSurfaceGroup {
	readonly rect: FreeLayoutRect;
	/** Authored members of this bound, including members outside the viewport. */
	readonly count: number;
}

interface Branch {
	rect: FreeLayoutRect;
	readonly count: number;
	parent: Branch | null;
	children: readonly [Branch, Branch] | null;
}

const union = (a: FreeLayoutRect, b: FreeLayoutRect): FreeLayoutRect => {
	const left = Math.min(a.left, b.left),
		top = Math.min(a.top, b.top);
	return {
		left,
		top,
		width: Math.max(a.left + a.width, b.left + b.width) - left,
		height: Math.max(a.top + a.height, b.top + b.height) - top
	};
};

/** Spatially balanced retained bounds. Overview traversal never enumerates dense members. */
export class FreeSurfaceGroups {
	#root: Branch | null = null;
	readonly #leaves = new Map<NodeId, Branch>();

	clear(): void {
		this.#root = null;
		this.#leaves.clear();
	}

	rebuild(entries: Iterable<readonly [NodeId, FreeLayoutRect]>): void {
		this.clear();
		const build = (items: (readonly [NodeId, FreeLayoutRect])[], depth: number): Branch => {
			if (items.length === 1) {
				const [id, rect] = items[0];
				const leaf: Branch = { rect, count: 1, parent: null, children: null };
				this.#leaves.set(id, leaf);
				return leaf;
			}
			// Only membership changes repartition. Live geometry updates repair ancestor bounds.
			const axis = depth % 2 === 0 ? 'left' : 'top';
			const size = axis === 'left' ? 'width' : 'height';
			items.sort((a, b) => a[1][axis] + a[1][size] / 2 - b[1][axis] - b[1][size] / 2);
			const middle = Math.floor(items.length / 2);
			const left = build(items.slice(0, middle), depth + 1);
			const right = build(items.slice(middle), depth + 1);
			const branch: Branch = {
				rect: union(left.rect, right.rect),
				count: left.count + right.count,
				parent: null,
				children: [left, right]
			};
			left.parent = branch;
			right.parent = branch;
			return branch;
		};
		const items = [...entries];
		if (items.length) this.#root = build(items, 0);
	}

	update(id: NodeId, rect: FreeLayoutRect): void {
		const leaf = this.#leaves.get(id);
		if (!leaf) return;
		leaf.rect = rect;
		for (let branch = leaf.parent; branch; branch = branch.parent) {
			const [left, right] = branch.children!;
			branch.rect = union(left.rect, right.rect);
		}
	}

	/** Conservative groups cover every intersecting member with at most 2 * limit visits. */
	query(rect: FreeLayoutRect, limit: number): FreeSurfaceGroup[] {
		if (!Number.isSafeInteger(limit) || limit < 1)
			throw new RangeError('Overview group limit must be a positive safe integer.');
		if (!this.#root || !rectsIntersect(rect, this.#root.rect)) return [];
		const pending: Branch[] = [this.#root];
		const groups: FreeSurfaceGroup[] = [];
		let visits = 0;
		while (pending.length) {
			const branch = pending.pop()!;
			if (
				branch.children &&
				groups.length + pending.length + 1 < limit &&
				visits + 2 <= limit * 2
			) {
				for (const child of branch.children) {
					visits++;
					if (rectsIntersect(rect, child.rect)) pending.push(child);
				}
			} else groups.push({ rect: branch.rect, count: branch.count });
		}
		return groups;
	}
}
