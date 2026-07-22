<script lang="ts">
	import type { ParamValue, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import ANodeSocketDefaultEditor from './ANodeSocketDefaultEditor.svelte';

	let { owner, referenceDeclId = 'source' }: { owner: UiNodeDto; referenceDeclId?: string } =
		$props();

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let liveOwner: UiNodeDto = $derived(graphState?.nodesById.get(owner.node_id) ?? owner);

	const childByDeclId = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphState) return null;
		for (const childId of parent.children) {
			const child = graphState.nodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	const referencedNode = (value: ParamValue | null): UiNodeDto | null => {
		if (!graphState || value?.kind !== 'reference') return null;
		if (value.cached_id !== undefined) {
			const byId = graphState.nodesById.get(value.cached_id);
			if (byId) return byId;
		}
		for (const candidate of graphState.nodesById.values()) {
			if (candidate.uuid === value.uuid) return candidate;
		}
		return null;
	};

	let referenceNode = $derived(childByDeclId(liveOwner, referenceDeclId));
	let referencedParameter = $derived.by((): UiNodeDto | null => {
		if (referenceNode?.data.kind !== 'parameter') return null;
		const candidate = referencedNode(referenceNode.data.param.value);
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
