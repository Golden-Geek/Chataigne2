import type { UiNodeDto, WorkbenchSession } from 'golden_ui';

type Snapshot = Pick<WorkbenchSession['graph']['state'], 'nodesById' | 'parentById'>;

/** Index only visited folders; value publications retain their declaration lookup. */
export class SpatializerChildIndex {
	#snapshot: Snapshot | null = null;
	#epoch: unknown;
	#folders = new Map<number, Map<string, number>>();
	#builds = 0;

	update(snapshot: Snapshot | null, epoch: unknown) {
		const previous = this.#snapshot;
		if (!snapshot || !previous || epoch !== this.#epoch) this.#folders.clear();
		else if (snapshot.nodesById !== previous.nodesById) {
			const extended = new Set<number>();
			const extend = (parent: number): boolean => {
				if (extended.has(parent)) return true;
				const index = this.#folders.get(parent),
					before = previous.nodesById.get(parent),
					next = snapshot.nodesById.get(parent);
				if (!index || !before || !next || before.uuid !== next.uuid) return false;
				const added = next.children.length - before.children.length;
				if (added <= 0) return false;
				const append = before.children.every((id, i) => next.children[i] === id);
				if (!append && !before.children.every((id, i) => next.children[added + i] === id))
					return false;
				const ids = append
					? next.children.slice(before.children.length)
					: next.children.slice(0, added);
				const nodes = ids.map((id) => snapshot.nodesById.get(id));
				if (nodes.some((node) => !node)) return false;
				// New aliases at the front supersede old ones; aliases appended
				// at the end keep the existing first-child lookup semantics.
				for (const node of append ? nodes : nodes.reverse()) this.#insert(index, node!, !append);
				extended.add(parent);
				return true;
			};
			snapshot.nodesById.forEachChange(previous.nodesById, (id, next, before) => {
				if ((next?.children !== before?.children || next?.uuid !== before?.uuid) && !extend(id))
					this.#folders.delete(id);
				if (
					next?.decl_id !== before?.decl_id ||
					next?.meta.label !== before?.meta.label ||
					next?.uuid !== before?.uuid
				) {
					const oldParent = previous.parentById.get(id),
						parent = snapshot.parentById.get(id);
					if (oldParent !== undefined) this.#folders.delete(oldParent);
					if (parent !== undefined && (before || !extend(parent))) this.#folders.delete(parent);
				}
			});
		}
		// Published GraphState objects may be reused; retain their immutable map versions explicitly.
		this.#snapshot = snapshot
			? { nodesById: snapshot.nodesById, parentById: snapshot.parentById }
			: null;
		this.#epoch = epoch;
	}

	#insert(index: Map<string, number>, child: UiNodeDto, replace = false) {
		for (const name of [
			child.decl_id,
			child.decl_id.slice(child.decl_id.lastIndexOf('/') + 1),
			child.meta.label
		])
			if (replace || !index.has(name)) index.set(name, child.node_id);
	}

	child(owner: UiNodeDto | null, key: string): UiNodeDto | null {
		const snapshot = this.#snapshot;
		if (!snapshot || !owner) return null;
		const current = snapshot.nodesById.get(owner.node_id);
		if (!current || current.uuid !== owner.uuid) return null;
		let index = this.#folders.get(current.node_id);
		if (!index) {
			index = new Map();
			for (const id of current.children) {
				const child = snapshot.nodesById.get(id);
				if (!child) continue;
				this.#insert(index, child);
			}
			this.#folders.set(current.node_id, index);
			this.#builds++;
		}
		const id = index.get(key);
		return id === undefined ? null : (snapshot.nodesById.get(id) ?? null);
	}

	get builds() {
		return this.#builds;
	}
}
