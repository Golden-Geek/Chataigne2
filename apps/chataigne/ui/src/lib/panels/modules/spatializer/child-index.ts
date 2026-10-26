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
			snapshot.nodesById.forEachChange(previous.nodesById, (id, next, before) => {
				if (next?.children !== before?.children || next?.uuid !== before?.uuid)
					this.#folders.delete(id);
				if (
					next?.decl_id !== before?.decl_id ||
					next?.meta.label !== before?.meta.label ||
					next?.uuid !== before?.uuid
				) {
					const oldParent = previous.parentById.get(id),
						parent = snapshot.parentById.get(id);
					if (oldParent !== undefined) this.#folders.delete(oldParent);
					if (parent !== undefined) this.#folders.delete(parent);
				}
			});
		}
		// Published GraphState objects may be reused; retain their immutable map versions explicitly.
		this.#snapshot = snapshot
			? { nodesById: snapshot.nodesById, parentById: snapshot.parentById }
			: null;
		this.#epoch = epoch;
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
				for (const name of [
					child.decl_id,
					child.decl_id.slice(child.decl_id.lastIndexOf('/') + 1),
					child.meta.label
				])
					if (!index.has(name)) index.set(name, id);
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
