<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import { useScreenAuthoring } from './context.svelte';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import DisplayPicker from './DisplayPicker.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const parametersOnly = (child: UiNodeDto) => child.decl_id === 'parameters';
	const owner = useScreenAuthoring();
	function parameter(name: string) {
		const session = owner.session;
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		return folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === `parameters/${name}`);
	}
	const targetKind = $derived.by(() => {
		const field = parameter('target_kind');
		return field?.data.kind === 'parameter' && field.data.param.value.kind === 'enum'
			? field.data.param.value.value
			: 'Unconfigured';
	});
	const display = $derived.by(() => {
		const field = parameter('display');
		return field?.data.kind === 'parameter' && field.data.param.value.kind === 'str'
			? field.data.param.value.value
			: '';
	});
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
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
	<div class="properties">
		{#if owner.view.error}<p role="alert">{owner.view.error}</p>{/if}
		{#if screen}
			{#if node.node_type === 'mapgyver.screen'}
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
				{#if targetKind === 'Fullscreen'}
					<DisplayPicker
						{display}
						disabled={busy}
						onSelect={async (value) => {
							const field = parameter('display');
							if (field)
								await owner.session?.sendIntent({
									kind: 'setParam',
									node: field.node_id,
									value: { kind: 'str', value },
									behaviour: 'Append'
								});
						}} />
				{/if}
			{:else if surface}
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
			{/if}
		{/if}
	</div>
{/snippet}
{@render defaultContent?.(content)}

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
