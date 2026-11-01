<script lang="ts">
	import { IconButton, type UiNodeDto } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { useScreenAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	const selection = useDocumentSelection();
	let { node }: { node: UiNodeDto } = $props();
	const owner = useScreenAuthoring();
</script>

{#if node.node_type === 'mapgyver.screen'}<IconButton
		icon={addIcon}
		label="Add Surface"
		disabled={owner.controller.busy}
		onclick={async (event) => {
			event.stopPropagation();
			const result = await owner.controller.edit({
				kind: 'AddSurface',
				screen: node.uuid,
				label: null
			});
			if (result?.kind === 'SurfaceCreated') selection.select(result.reference.surface);
		}} />{/if}
