<script lang="ts">
	import { appState } from '../../store/workbench.svelte';
	import { sendPatchMetaIntent } from '../../store/ui-intents';
	import type { UiNodeDto } from '../../types';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let enabled = $derived(liveNode.meta.enabled);
	let canBeDisabled = $derived(liveNode.meta.can_be_disabled);
	let enabledInHierarchy = $derived(session?.isNodeEnabledInHierarchy(liveNode.node_id) ?? enabled);

	const toggleEnabled = (): void => {
		if (!canBeDisabled) {
			return;
		}
		void sendPatchMetaIntent(liveNode.node_id, { enabled: !enabled });
	};
</script>

<button
	type="button"
	class="enable-button"
	role="switch"
	aria-label="Toggle node enabled"
	aria-checked={enabledInHierarchy}
	class:enabled={enabledInHierarchy}
	class:parent-disabled={enabled && !enabledInHierarchy}
	title={enabledInHierarchy ? 'Node enabled' : enabled ? 'Disabled by parent' : 'Node disabled'}
	tabindex="0"
	onmousedown={toggleEnabled}
	onkeydown={(event) => {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			toggleEnabled();
		}
	}}></button>

<style>
	.enable-button {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 2rem;
		display: inline-block;
		background-color: rgba(50, 50, 50, 0.5);
		cursor: pointer;
		vertical-align: middle;
		transition:
			background-color 0.2s ease,
			box-shadow 0.2s ease,
			border 0.2s ease;
		border: solid 1px rgba(200, 200, 200, 0.5);
		padding: 0;
	}

	.enable-button.enabled {
		background-color: var(--gc-color-node-enabled);
		box-shadow: 0 0 0.3rem var(--gc-color-node-enabled);
	}

	.enable-button.parent-disabled {
		background-color: var(--gc-color-node-inactive);
		box-shadow: 0 0 0.3rem var(--gc-color-node-inactive);
	}
</style>
