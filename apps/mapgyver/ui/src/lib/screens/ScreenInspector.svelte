<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import { useScreenAuthoring } from './context.svelte';
	import ScreenTargetForm from './ScreenTargetForm.svelte';
	import SurfaceSourceForm from './SurfaceSourceForm.svelte';
	let { node, defaultHeader }: { node: UiNodeDto; defaultHeader?: Snippet } = $props();
	const owner = useScreenAuthoring();
	onMount(() => owner.attach());
	const screen = $derived(
		owner.view.catalog?.screens.find(
			(screen) =>
				screen.id === node.uuid ||
				screen.surfaces.some((surface) => surface.reference.surface === node.uuid)
		)
	);
	const surface = $derived(
		screen?.surfaces.find((surface) => surface.reference.surface === node.uuid)
	);
	const busy = $derived(
		owner.controller.busy ||
			owner.gestureState.active ||
			owner.session?.hasActiveEditSession === true
	);
</script>

{@render defaultHeader?.()}
<div class="properties">
	{#if owner.view.error}<p role="alert">{owner.view.error}</p>{/if}
	{#if screen}
		{#if node.node_type === 'mapgyver.screen'}
			{#key screen.id}<ScreenTargetForm
					{screen}
					disabled={busy}
					configure={async (definition) => {
						await owner.controller.edit({ kind: 'ConfigureScreen', screen: screen.id, definition });
					}} />{/key}
		{:else if surface}
			{#key surface.reference.surface}<SurfaceSourceForm
					{surface}
					media={owner.view.media}
					disabled={busy}
					configure={async (definition) => {
						await owner.controller.edit({
							kind: 'ConfigureSurface',
							reference: surface.reference,
							definition
						});
					}} />{/key}
		{/if}
	{/if}
</div>

<style>
	.properties {
		padding: 0.5rem;
		min-width: 0;
	}
</style>
