<script lang="ts">
	import type { UiNodeDto } from '../../../types';
	import { appState } from '../../../store/workbench.svelte';
	import EnableButton from '../../common/EnableButton.svelte';
	import NodeInspector from '../inspector/NodeInspector.svelte';

	let {
		targetNode,
		widgetNode = null,
		insideLabel = null,
		maxChildLevel = 2,
		showEnableButton = true
	} = $props<{
		targetNode: UiNodeDto;
		widgetNode?: UiNodeDto | null;
		insideLabel?: string | null;
		maxChildLevel?: number;
		showEnableButton?: boolean;
		editMode?: boolean;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveTargetNode = $derived(graph?.nodesById.get(targetNode.node_id) ?? targetNode);
	let effectiveMaxChildLevel = $derived(
		Number.isFinite(maxChildLevel) ? Math.max(0, Math.round(maxChildLevel)) : 2
	);
	let insideLabelText = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsEnableButton = $derived(showEnableButton && liveTargetNode.meta.can_be_disabled);
	let showsToolbar = $derived(insideLabelText.length > 0 || showsEnableButton);
</script>

<div class="dashboard-node-widget-inspector-scroll">
	<div class="dashboard-node-widget-inspector-natural">
		{#if showsToolbar}
			<div class="dashboard-node-widget-inspector-toolbar">
				{#if insideLabelText.length > 0}
					<div class="dashboard-node-widget-inspector-label">{insideLabelText}</div>
				{/if}
				{#if showsEnableButton}
					<div class="dashboard-node-widget-inspector-enable">
						<EnableButton node={liveTargetNode} />
					</div>
				{/if}
			</div>
		{/if}
		<div class="dashboard-node-widget-inspector-root">
			<NodeInspector
				nodes={[liveTargetNode]}
				level={0}
				layoutMode="dashboard"
				maxChildLevel={effectiveMaxChildLevel} />
		</div>
	</div>
</div>

<style>
	.dashboard-node-widget-inspector-scroll {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: auto;
		background-color: var(--gc-color-panel);
	}

	.dashboard-node-widget-inspector-natural {
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		min-block-size: 100%;
	}

	.dashboard-node-widget-inspector-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
		padding: 0.75rem 0.75rem 0 0.75rem;
	}

	.dashboard-node-widget-inspector-label {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.78rem;
		font-weight: 600;
	}

	.dashboard-node-widget-inspector-enable {
		display: flex;
		align-items: center;
		flex: 0 0 auto;
	}

	.dashboard-node-widget-inspector-root {
		display: flex;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-inspector-natural :global(.node-inspector.root) {
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		flex: 1 1 auto;
	}
</style>
