<script lang="ts">
	import { showPanel } from '../../golden_ui/store/ui-panels';
	import { appState } from '../../golden_ui/store/workbench.svelte';
	import type { GraphState } from '../../golden_ui/store/graph.svelte';
	import type { NodeId, UiNodeDto } from '../../golden_ui/types';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	const CONNECTED_PARAM_PATH = ['infos', 'connected'] as const;

	const findDescendantByDeclPath = (
		graph: GraphState | null,
		rootNodeId: NodeId,
		path: readonly string[]
	): UiNodeDto | null => {
		if (!graph) {
			return null;
		}

		let currentNodeId: NodeId = rootNodeId;
		for (const segment of path) {
			const childIds = graph.childrenById.get(currentNodeId) ?? [];
			let nextNodeId: NodeId | null = null;
			for (const childId of childIds) {
				const childNode = graph.nodesById.get(childId);
				if (childNode?.decl_id === segment) {
					nextNodeId = childId;
					break;
				}
			}
			if (nextNodeId === null) {
				return null;
			}
			currentNodeId = nextNodeId;
		}

		return graph.nodesById.get(currentNodeId) ?? null;
	};

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(graph?.nodesById.get(node.node_id) ?? node);
	let connectedParamNode = $derived(
		findDescendantByDeclPath(graph, liveNode.node_id, CONNECTED_PARAM_PATH)
	);
	let connectionState = $derived.by((): boolean | null => {
		if (connectedParamNode?.data.kind !== 'parameter') {
			return null;
		}
		const { value } = connectedParamNode.data.param;
		if (value.kind !== 'bool') {
			return null;
		}
		return value.value;
	});
	let connectionLabel = $derived.by(() => {
		if (connectionState === true) {
			return 'Connected';
		}
		if (connectionState === false) {
			return 'Disconnected';
		}
		return 'Status';
	});
	let connectionClassName = $derived.by(() => {
		if (connectionState === true) {
			return 'connected';
		}
		if (connectionState === false) {
			return 'disconnected';
		}
		return 'unknown';
	});

	const revealConnectedParameter = (): void => {
		if (!connectedParamNode) {
			return;
		}

		session?.selectNode(connectedParamNode.node_id, 'REPLACE');
		showPanel({
			panelType: 'inspector',
			panelId: 'inspector'
		});
	};
</script>

{#if connectedParamNode}
	<button
		type="button"
		class={`module-connection-pill ${connectionClassName}`}
		title={`Inspect ${connectedParamNode.meta.label}`}
		aria-label={`Inspect ${connectedParamNode.meta.label}`}
		onclick={(event) => {
			event.stopPropagation();
			revealConnectedParameter();
		}}>
		<span class="module-connection-indicator" aria-hidden="true"></span>
		<span class="module-connection-label">{connectionLabel}</span>
	</button>
{/if}

<style>
	.module-connection-pill {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.1rem 0.45rem;
		border: 0.06rem solid rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		border-radius: 999rem;
		background: rgb(from var(--gc-color-background) r g b / 0.28);
		color: rgb(from var(--gc-color-text) r g b / 0.8);
		font: inherit;
		font-size: 0.7rem;
		line-height: 1;
		transition:
			background-color 0.12s ease,
			border-color 0.12s ease,
			color 0.12s ease;
	}

	.module-connection-pill:hover {
		background: rgb(from var(--gc-color-text) r g b / 0.08);
	}

	.module-connection-pill.connected {
		border-color: rgb(from var(--gc-color-success) r g b / 0.42);
		background: rgb(from var(--gc-color-success) r g b / 0.12);
		color: rgb(from var(--gc-color-success) r g b / 0.95);
	}

	.module-connection-pill.disconnected {
		border-color: rgb(from var(--gc-color-error) r g b / 0.42);
		background: rgb(from var(--gc-color-error) r g b / 0.12);
		color: rgb(from var(--gc-color-error) r g b / 0.95);
	}

	.module-connection-pill.unknown {
		border-color: rgb(from var(--gc-color-panel-outline) r g b / 0.42);
		background: rgb(from var(--gc-color-background) r g b / 0.2);
		color: rgb(from var(--gc-color-text) r g b / 0.68);
	}

	.module-connection-indicator {
		width: 0.45rem;
		height: 0.45rem;
		border-radius: 999rem;
		background: currentColor;
		box-shadow: 0 0 0.3rem rgb(from currentColor r g b / 0.3);
	}

	.module-connection-label {
		white-space: nowrap;
	}
</style>
