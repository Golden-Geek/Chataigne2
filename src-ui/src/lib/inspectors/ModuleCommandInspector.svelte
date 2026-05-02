<script lang="ts">
	import type { NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from '$lib/golden_ui/store/workbench.svelte';
	import TriggerEditor from '$lib/golden_ui/components/panels/inspector/parameters/TriggerEditor.svelte';

	let { node, defaultHeader, defaultChildren, collapsed }: NodeInspectorComponentProps = $props();

	let session = $derived(appState.session);
	let liveNode: UiNodeDto = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);

	const isCommandTrigger = (candidate: UiNodeDto): boolean =>
		candidate.data.kind === 'parameter' &&
		candidate.data.param.value.kind === 'trigger' &&
		(candidate.decl_id === 'trigger' ||
			candidate.meta.short_name === 'trigger' ||
			candidate.meta.label === 'Trigger');

	let triggerNode = $derived.by((): UiNodeDto | null => {
		if (!graphNodesById) {
			return null;
		}

		for (const childId of liveNode.children) {
			const child = graphNodesById.get(childId);
			if (child && isCommandTrigger(child)) {
				return child;
			}
		}

		return null;
	});
</script>

{#snippet commandHeaderExtra()}
	{#if triggerNode}
		<span
			class="command-header-trigger"
			role="presentation"
			onclick={(event) => {
				event.stopPropagation();
			}}
			onkeydown={(event) => {
				event.stopPropagation();
			}}>
			<TriggerEditor node={triggerNode} layoutMode="widget" insideLabel={triggerNode.meta.label} />
		</span>
	{/if}
{/snippet}

{@render defaultHeader?.(commandHeaderExtra)}

{#if collapsed !== true}
	<div class="node-inspector-content module-command-inspector">
		{@render defaultChildren?.()}
	</div>
{/if}

<style>
	.command-header-trigger {
		display: inline-flex;
		align-items: center;
		inline-size: 4.8rem;
		block-size: 1.15rem;
		margin-left: 0.15rem;
	}

	.command-header-trigger :global(.trigger) {
		inline-size: 100%;
		block-size: 100%;
		min-block-size: 0;
		border-radius: 0.35rem;
		gap: 0.25rem;
		padding-inline: 0.35rem;
	}

	.command-header-trigger :global(.trigger img) {
		inline-size: 0.75rem;
		block-size: 0.75rem;
		padding: 0;
	}

	.command-header-trigger :global(.trigger-label) {
		font-size: 0.68rem;
	}

	.module-command-inspector {
		min-inline-size: 0;
	}
</style>
