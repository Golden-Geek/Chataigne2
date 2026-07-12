import type { NodeId, UiNodeDto } from '../../types';
import type { GraphStore } from '../graph.svelte';
import type { FooterHoverInfo } from './types';

interface FooterHoverEntry {
	token: symbol;
	nodeId: NodeId;
}

interface WorkbenchFooterHoverStoreOptions {
	getNodeDescription(node: UiNodeDto | null | undefined): string | null;
}

export interface WorkbenchFooterHoverStore {
	clearFooterHover(token: symbol): void;
	setFooterHover(token: symbol, nodeId: NodeId): void;
	prune(): void;
	getFooterHoverInfo(): FooterHoverInfo | null;
	reset(): void;
}

export const createWorkbenchFooterHoverStore = (
	graph: GraphStore,
	options: WorkbenchFooterHoverStoreOptions
): WorkbenchFooterHoverStore => {
	let footerHoverStack = $state<FooterHoverEntry[]>([]);

	const clearFooterHover = (token: symbol): void => {
		const nextStack = footerHoverStack.filter((entry) => entry.token !== token);
		if (nextStack.length !== footerHoverStack.length) {
			footerHoverStack = nextStack;
		}
	};

	const setFooterHover = (token: symbol, nodeId: NodeId): void => {
		if (!graph.state.nodesById.has(nodeId)) {
			clearFooterHover(token);
			return;
		}
		footerHoverStack = [
			...footerHoverStack.filter((entry) => entry.token !== token),
			{ token, nodeId }
		];
	};

	const prune = (): void => {
		if (footerHoverStack.length === 0) {
			return;
		}
		const nextStack = footerHoverStack.filter((entry) => graph.state.nodesById.has(entry.nodeId));
		if (nextStack.length !== footerHoverStack.length) {
			footerHoverStack = nextStack;
		}
	};

	const getFooterHoverInfo = (): FooterHoverInfo | null => {
		for (let index = footerHoverStack.length - 1; index >= 0; index -= 1) {
			const node = graph.state.nodesById.get(footerHoverStack[index]?.nodeId ?? -1);
			if (!node) {
				continue;
			}
			const description = options.getNodeDescription(node);
			if (description === null) {
				return null;
			}
			return {
				node_id: node.node_id,
				label: node.meta.label,
				node_type: node.node_type,
				description
			};
		}
		return null;
	};

	const reset = (): void => {
		footerHoverStack = [];
	};

	return {
		clearFooterHover,
		setFooterHover,
		prune,
		getFooterHoverInfo,
		reset
	};
};
