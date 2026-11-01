<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { IconButton, type UiNodeDto } from 'golden_ui';
	import { useScreenAuthoring } from './context.svelte';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import NameField from '../properties/NameField.svelte';
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
	const screenIndex = $derived(
		owner.view.catalog?.screens.findIndex((item) => item.id === screen?.id) ?? -1
	);
	const surfaceIndex = $derived(
		screen?.surfaces.findIndex((item) => item.reference.surface === node.uuid) ?? -1
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
			<NameField
				label="Screen name"
				value={screen.label}
				disabled={busy}
				rename={(label) =>
					owner.controller.edit({ kind: 'RenameScreen', screen: screen.id, label })} />
			<div class="actions">
				<IconButton
					icon={arrowIcon}
					class="earlier"
					label="Move screen earlier"
					disabled={busy || screenIndex <= 0}
					onclick={() =>
						owner.controller.edit({
							kind: 'ReorderScreen',
							screen: screen.id,
							after: owner.view.catalog?.screens[screenIndex - 2]?.id ?? null
						})} />
				<IconButton
					icon={arrowIcon}
					class="later"
					label="Move screen later"
					disabled={busy ||
						!owner.view.catalog ||
						screenIndex >= owner.view.catalog.screens.length - 1}
					onclick={() =>
						owner.controller.edit({
							kind: 'ReorderScreen',
							screen: screen.id,
							after: owner.view.catalog!.screens[screenIndex + 1].id
						})} />
				<IconButton
					icon={deleteIcon}
					label="Delete screen"
					disabled={busy}
					onclick={() => owner.controller.edit({ kind: 'RemoveScreen', screen: screen.id })} />
			</div>
			{#key screen.id}<ScreenTargetForm
					{screen}
					disabled={busy}
					configure={async (definition) => {
						await owner.controller.edit({ kind: 'ConfigureScreen', screen: screen.id, definition });
					}} />{/key}
		{:else if surface}
			<NameField
				label="Surface name"
				value={surface.label}
				disabled={busy}
				rename={(label) =>
					owner.controller.edit({ kind: 'RenameSurface', reference: surface.reference, label })} />
			<div class="actions">
				<IconButton
					icon={arrowIcon}
					class="earlier"
					label="Move surface earlier"
					disabled={busy || surfaceIndex <= 0}
					onclick={() =>
						owner.controller.edit({
							kind: 'ReorderSurface',
							reference: surface.reference,
							after: screen.surfaces[surfaceIndex - 2]?.reference.surface ?? null
						})} />
				<IconButton
					icon={arrowIcon}
					class="later"
					label="Move surface later"
					disabled={busy || surfaceIndex >= screen.surfaces.length - 1}
					onclick={() =>
						owner.controller.edit({
							kind: 'ReorderSurface',
							reference: surface.reference,
							after: screen.surfaces[surfaceIndex + 1].reference.surface
						})} />
				<IconButton
					icon={deleteIcon}
					label="Delete surface"
					disabled={busy}
					onclick={() =>
						owner.controller.edit({ kind: 'RemoveSurface', reference: surface.reference })} />
			</div>
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
	.actions {
		display: flex;
		gap: 0.25rem;
	}
	:global(.earlier img) {
		transform: rotate(-90deg);
	}
	:global(.later img) {
		transform: rotate(90deg);
	}
	.properties {
		padding: 0.5rem;
		min-width: 0;
	}
</style>
