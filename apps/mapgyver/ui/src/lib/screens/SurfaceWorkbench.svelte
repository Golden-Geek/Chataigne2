<script lang="ts">
	import { onMount } from 'svelte';
	import { IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import fillIcon from './icons/fill.svg';
	import fitIcon from './icons/fit.svg';
	import type { SurfaceFitMode } from './generated/SurfaceFitMode';
	import { useScreenAuthoring } from './context.svelte';
	import ScreenPreview from './ScreenPreview.svelte';
	import SurfaceSourcePreview from './SurfaceSourcePreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import { pickSurfaceSource } from '../media/source-hit';
	let {
		panelId,
		panelType,
		title,
		panel,
		editor
	}: PanelProps & { panel: PanelState | null; editor: 'screen' | 'source' } = $props();
	const owner = useScreenAuthoring();
	const selection = useDocumentSelection();
	onMount(() => owner.attach());
	const { view, controller, gestureState, gesture } = owner;
	const screen = $derived(controller.screen),
		surface = $derived(controller.surface);
	function pickDropTarget(event: DragEvent) {
		const controls = (event.currentTarget as HTMLElement).querySelector(
			'svg[aria-label="Surface mapping controls"]'
		);
		const rect = controls?.getBoundingClientRect();
		if (!screen || !rect || rect.width <= 0 || rect.height <= 0) return null;
		return pickSurfaceSource(
			{ x: (event.clientX - rect.left) / rect.width, y: (event.clientY - rect.top) / rect.height },
			screen.surfaces
		);
	}
	function selectSurface(uuid: string | null) {
		if (gestureState.active || !screen) return;
		if (uuid) controller.selectSurface({ screen: screen.id, surface: uuid });
		else controller.selectScreen(screen.id);
		selection.select(uuid ?? screen.id);
	}
	function fitSurface(mode: SurfaceFitMode) {
		if (!surface || gestureState.active) return;
		void controller.edit({ kind: 'FitSurface', reference: surface.reference, mode });
	}
</script>

<section
	class="workbench-editor"
	aria-label={panel?.title ?? title ?? (editor === 'screen' ? 'Screen Warping' : 'Surface Source')}
	data-panel-id={panelId}
	data-panel-type={panelType}
	aria-busy={controller.busy || gestureState.active}>
	{#snippet actions()}
		{#if editor === 'screen'}
			<IconButton
				icon={fillIcon}
				label="Fill Screen"
				title="Stretch Surface to Screen bounds"
				disabled={controller.busy || gestureState.active || !surface}
				onclick={() => fitSurface('Fill')} />
			<IconButton
				icon={fitIcon}
				label="Fit to Screen"
				title="Center Surface with cropped-source aspect preserved"
				disabled={controller.busy || gestureState.active || !surface}
				onclick={() => fitSurface('Fit')} />
		{/if}
	{/snippet}
	<SourceDropZone
		class="editor-canvas"
		data-screen-drop-target={editor === 'screen' ? '' : undefined}
		addLabel="Add Surface"
		replaceOnly={editor === 'source' ? surface?.reference.surface : null}
		hitTest={editor === 'screen' ? pickDropTarget : undefined}
		disabled={controller.busy ||
			gestureState.active ||
			!screen ||
			(editor === 'source' && !surface)}
		onsource={async (source, target) => {
			if (!screen) return;
			const selectDropped = selection.capture();
			const result = await controller.edit({
				kind: 'DropSource',
				screen: screen.id,
				surface: target,
				source
			});
			if (result?.kind === 'SurfaceCreated') selectDropped(result.reference.surface);
			else if (result?.kind === 'SurfaceChanged') selectDropped(result.snapshot.reference.surface);
		}}>
		{#if view.error || gestureState.error}<p class="editor-error" role="alert">
				{view.error || gestureState.error}
			</p>{/if}
		{#if editor === 'screen' && screen}
			<ScreenPreview
				{actions}
				{screen}
				selected={view.selectedSurface}
				disabled={controller.busy}
				select={selectSurface}
				start={(reference, handle) => gesture.start(reference, handle)}
				pixelsActive={gesture.pixelsActive}
				move={(point) => gesture.move(point)}
				finish={() => gesture.finish()}
				stop={() => gesture.stop()} />
		{:else if editor === 'source' && surface}
			<SurfaceSourcePreview
				{surface}
				disabled={controller.busy}
				start={(handle) => gesture.start(surface.reference, handle)}
				move={(point) => gesture.move(point)}
				finish={() => gesture.finish()}
				stop={() => gesture.stop()} />
		{:else}<p class="editor-empty">
				Select {editor === 'screen' ? 'a Screen' : 'a Surface'} in Screen Manager.
			</p>{/if}
	</SourceDropZone>
</section>
