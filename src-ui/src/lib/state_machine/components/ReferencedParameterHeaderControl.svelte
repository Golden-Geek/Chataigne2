<script lang="ts">
	import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import ANodeSocketDefaultEditor from './ANodeSocketDefaultEditor.svelte';

	let { owner, referenceDeclId = 'source' }: { owner: UiNodeDto; referenceDeclId?: string } =
		$props();

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let liveOwner: UiNodeDto = $derived(graphNodesById?.get(owner.node_id) ?? owner);
	let graphNodesByUuid = $derived.by((): ReadonlyMap<string, NodeId> => {
		if (!graphNodesById) return new Map();
		return new Map(Array.from(graphNodesById.values()).map((candidate) => [candidate.uuid, candidate.node_id]));
	});

	const childByDeclId = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphNodesById) return null;
		for (const childId of parent.children) {
			const child = graphNodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	const referencedNodeId = (value: ParamValue | null): NodeId | null => {
		if (!graphNodesById || value?.kind !== 'reference') return null;
		return value.cached_id ?? graphNodesByUuid.get(value.uuid) ?? null;
	};

	let referenceNode = $derived(childByDeclId(liveOwner, referenceDeclId));
	let referencedParameter = $derived.by((): UiNodeDto | null => {
		if (!graphNodesById || referenceNode?.data.kind !== 'parameter') return null;
		const nodeId = referencedNodeId(referenceNode.data.param.value);
		const candidate = nodeId === null ? null : (graphNodesById.get(nodeId) ?? null);
		return candidate?.data.kind === 'parameter' ? candidate : null;
	});
</script>

{#if referencedParameter}
	<span
		class="referenced-parameter-header-control"
		title={referencedParameter.meta.label}
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<ANodeSocketDefaultEditor parameter={referencedParameter} />
	</span>
{/if}

<style>
	.referenced-parameter-header-control {
		display: inline-flex;
		align-items: center;
		inline-size: min(12rem, 42vw);
		min-inline-size: 5rem;
		block-size: 1.4rem;
		overflow: hidden;
	}

	.referenced-parameter-header-control :global(.socket-default-editor) {
		inline-size: 100%;
		min-inline-size: 0;
	}
</style>
