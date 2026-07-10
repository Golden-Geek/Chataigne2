<script lang="ts">
	import { appState } from '../../store/workbench.svelte';
	import type { UiNodeDto } from '../../types';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	let session = $derived(appState.session);
</script>

<button
	class="select-node-btn"
	title="Select Node in Outliner"
	onclick={(e) => {
		session?.selectNode(node.node_id, 'REPLACE');
		e.stopPropagation(); // Prevents the node from collapsing
		// Implement the logic to select the node in the outliner here
	}}>
</button>

<style>
	.select-node-btn {
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
		width: 1rem;
		height: 1rem;
		background-image: url('../../style/icons/select-node.svg'); /* Replace with your icon path */
		background-size: contain;
		background-repeat: no-repeat;
		background-position: center;
	}
</style>
