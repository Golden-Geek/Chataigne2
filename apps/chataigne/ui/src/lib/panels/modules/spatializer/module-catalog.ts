import type { UiNodeDto, WorkbenchSession } from 'golden_ui';
import { SPATIALIZER_MODULE_TYPE } from './constants';

type Nodes = WorkbenchSession['graph']['state']['nodesById'];

/** Keep module selection independent of unrelated matrix value/structure publications. */
export class SpatializerModuleCatalog {
	#nodes: Nodes | null = null;
	#epoch: unknown;
	#modules = new Map<number, UiNodeDto>();
	#ordered: UiNodeDto[] = [];

	update(nodes: Nodes | null, epoch: unknown): UiNodeDto[] {
		let changed = false;
		const visit = (id: number, node: UiNodeDto | undefined) => {
			if (node?.node_type === SPATIALIZER_MODULE_TYPE) {
				if (this.#modules.get(id) !== node) {
					this.#modules.set(id, node);
					changed = true;
				}
			} else if (this.#modules.delete(id)) changed = true;
		};
		if (!nodes || !this.#nodes || epoch !== this.#epoch) {
			changed = this.#modules.size > 0;
			this.#modules.clear();
			if (nodes) for (const node of nodes.values()) visit(node.node_id, node);
		} else if (nodes !== this.#nodes)
			nodes.forEachChange(this.#nodes, (id, node) => visit(id, node));
		this.#nodes = nodes;
		this.#epoch = epoch;
		if (changed)
			this.#ordered = [...this.#modules.values()].sort((a, b) =>
				a.meta.label.localeCompare(b.meta.label)
			);
		return this.#ordered;
	}
}
