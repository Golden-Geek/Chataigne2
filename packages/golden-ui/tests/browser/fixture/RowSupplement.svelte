<script lang="ts">
	import type { UiNodeDto } from '../../../types';
	let { node }: { node: UiNodeDto } = $props();
</script>

{#if node.node_id % 13 === 0}<span data-tall-supplement style:block-size="4rem">Tall row</span>{/if}
