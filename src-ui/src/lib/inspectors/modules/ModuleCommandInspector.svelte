<script lang="ts">
	import type { NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from '$lib/golden_ui/store/workbench.svelte';
	import CheckboxEditor from '$lib/golden_ui/components/panels/inspector/parameters/CheckboxEditor.svelte';
	import TriggerEditor from '$lib/golden_ui/components/panels/inspector/parameters/TriggerEditor.svelte';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let liveNode: UiNodeDto = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);

	const isCommandTrigger = (candidate: UiNodeDto): boolean =>
		candidate.data.kind === 'parameter' &&
		candidate.data.param.value.kind === 'trigger' &&
		(candidate.decl_id === 'trigger' ||
			candidate.meta.short_name === 'trigger' ||
			candidate.meta.label === 'Trigger');

	const isAutoTrigger = (candidate: UiNodeDto): boolean =>
		candidate.data.kind === 'parameter' &&
		candidate.data.param.value.kind === 'bool' &&
		(candidate.decl_id === 'auto_trigger' ||
			candidate.meta.short_name === 'auto_trigger' ||
			candidate.meta.label === 'Auto Trigger');

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

	let autoTriggerNode = $derived.by((): UiNodeDto | null => {
		if (!graphNodesById) {
			return null;
		}

		for (const childId of liveNode.children) {
			const child = graphNodesById.get(childId);
			if (child && isAutoTrigger(child)) {
				return child;
			}
		}

		return null;
	});
</script>

{#snippet commandHeaderExtra()}
	{#if autoTriggerNode || triggerNode}
		<span
			class="command-header-controls"
			role="presentation"
			onclick={(event) => {
				event.stopPropagation();
			}}
			onkeydown={(event) => {
				event.stopPropagation();
			}}>
			{#if autoTriggerNode}
				<span class="command-header-auto-trigger" title={autoTriggerNode.meta.label}>
					<CheckboxEditor node={autoTriggerNode} layoutMode="widget" insideLabel="Auto" />
				</span>
			{/if}
			{#if triggerNode}
				<span class="command-header-trigger">
					<TriggerEditor
						node={triggerNode}
						layoutMode="widget"
						insideLabel={triggerNode.meta.label} />
				</span>
			{/if}
		</span>
	{/if}
{/snippet}

{@render defaultHeader?.(commandHeaderExtra)}

{#snippet commandContent()}
	{@render defaultChildren?.()}
{/snippet}

{@render defaultContent?.(commandContent, 'module-command-inspector')}

<style>
	.command-header-controls {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
		margin-left: 0.15rem;
	}

	.command-header-auto-trigger {
		display: inline-flex;
		align-items: center;
		inline-size: 3rem;
		block-size: 1.15rem;
	}

	.command-header-trigger {
		display: inline-flex;
		align-items: center;
		inline-size: 4.8rem;
		block-size: 1.15rem;
	}

	.command-header-auto-trigger :global(.widget-checkbox-button) {
		border-radius: 0.35rem;
		padding: 0;
	}

	.command-header-auto-trigger
		:global(.widget-checkbox-button.with-inline-label .widget-checkbox-mark) {
		padding-inline: 0.25rem;
		font-size: 0.62rem;
		font-weight: 600;
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

	:global(.module-command-inspector) {
		min-inline-size: 0;
	}
</style>
