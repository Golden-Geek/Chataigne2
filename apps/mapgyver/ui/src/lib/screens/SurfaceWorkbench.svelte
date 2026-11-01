<script lang="ts">
	import { onMount } from 'svelte';
	import { appState, IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import refreshIcon from 'golden_ui/style/icons/reset.svg';
	import { useScreenAuthoring } from './context.svelte';
	import ScreenPreview from './ScreenPreview.svelte';
	import SurfaceSourcePreview from './SurfaceSourcePreview.svelte';
	let {
		panelId,
		panelType,
		title,
		panel,
		editor
	}: PanelProps & { panel: PanelState | null; editor: 'screen' | 'source' } = $props();
	const owner = useScreenAuthoring();
	onMount(() => owner.attach());
	const { view, controller, gestureState, gesture } = owner;
	const screen = $derived(controller.screen),
		surface = $derived(controller.surface);
	function select(uuid: string) {
		const session = appState.session;
		if (!session) return;
		const node = [...session.graph.state.nodesById.values()].find((node) => node.uuid === uuid);
		if (node) session.selectNode(node.node_id, 'REPLACE');
	}
</script>

<section
	class="workbench-editor"
	aria-label={panel?.title ?? title ?? (editor === 'screen' ? 'Screen Warping' : 'Surface Source')}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<div class="editor-toolbar">
		<span>{editor === 'source' ? surface?.label : screen?.label}</span>
		<IconButton
			icon={refreshIcon}
			label="Refresh preview"
			disabled={controller.busy || gestureState.active}
			onclick={() => controller.refresh()} />
	</div>
	<div class="editor-canvas">
		{#if view.error || gestureState.error}<p class="editor-error" role="alert">
				{view.error || gestureState.error}
			</p>{/if}
		{#if editor === 'screen' && screen}
			{#key screen.id}<ScreenPreview
					{screen}
					selected={view.selectedSurface}
					disabled={controller.busy}
					select={(uuid) => {
						if (!gestureState.active) {
							view.selectedSurface = uuid;
							select(uuid);
						}
					}}
					start={(reference, corner) => gesture.start(reference, { kind: 'QuadCorner', corner })}
					move={(point) => gesture.move(point)}
					finish={() => gesture.finish()}
					stop={() => gesture.stop()} />{/key}
		{:else if editor === 'source' && surface}
			{#key surface.reference.surface}<SurfaceSourcePreview
					{surface}
					disabled={controller.busy}
					start={(handle) => gesture.start(surface.reference, handle)}
					move={(point) => gesture.move(point)}
					finish={() => gesture.finish()}
					stop={() => gesture.stop()} />{/key}
		{:else}<p class="editor-empty">
				Select {editor === 'screen' ? 'a Screen' : 'a Surface'} in Screen Manager.
			</p>{/if}
	</div>
</section>

<style>
	/* Error visibility must not move the mapping coordinate space during a gesture. */
	.editor-error {
		position: absolute;
		inset: 0 0 auto;
		z-index: 5;
		background: var(--gc-dock-panel-background, #202329);
		pointer-events: none;
	}
</style>
