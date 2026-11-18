<script lang="ts">
	import { untrack } from 'svelte';
	import {
		appState,
		IconButton,
		readPanelPersistedState,
		writePanelPersistedState,
		type PanelProps,
		type PanelState
	} from 'golden_ui';
	import fillIcon from './icons/fill.svg';
	import fitIcon from './icons/fit.svg';
	import relatedIcon from './icons/related.svg';
	import previousIcon from './icons/previous.svg';
	import nextIcon from './icons/next.svg';
	import magnetIcon from './icons/magnet.svg';
	import { createScreenHandles } from './handle-reader.svelte';
	import { createRelatedSurfaces } from './related.svelte';
	import type { SurfaceFitMode } from './generated/SurfaceFitMode';
	import { createScreenEditor } from './editor.svelte';
	import { screenPanelState, type ScreenPanelState } from './panel-state';
	import ScreenPreview from './ScreenPreview.svelte';
	import SurfaceSourcePreview from './SurfaceSourcePreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import { pickSurfaceSource } from '../media/source-hit';
	import { followPanelTitle } from '../panel-title.svelte';
	let {
		panelId,
		panelType,
		panelApi,
		title,
		params,
		panel,
		editor
	}: PanelProps & { panel: PanelState | null; editor: 'screen' | 'source' } = $props();
	let choice = $state(
		untrack(() =>
			screenPanelState(
				readPanelPersistedState<ScreenPanelState>(params),
				editor === 'screen' ? 'Screen' : 'Surface'
			)
		)
	);
	$effect(() => {
		if (panel) {
			const next = screenPanelState(
				readPanelPersistedState<ScreenPanelState>(panel.params),
				editor === 'screen' ? 'Screen' : 'Surface'
			);
			untrack(() => {
				if (
					choice.document !== next.document ||
					JSON.stringify(choice.target) !== JSON.stringify(next.target)
				)
					owner.gesture.stop();
				choice = next;
			});
		}
	});
	const owner = createScreenEditor(() => ({
		editor: editor === 'screen' ? 'Screen' : 'Surface',
		selected: appState.session?.selectedNodeUuid ?? null,
		document: choice.document,
		locked: choice.locked ? choice.target : null,
		retained: choice.target,
		surface: choice.surface
	}));
	const selection = useDocumentSelection();
	const { view, controller, gestureState, gesture } = owner;
	const screen = $derived(owner.screen),
		surface = $derived(owner.surface);
	const inputAvailable = $derived(
		appState.session?.status === 'connected' && !appState.session.loading.visible
	);
	const cropParameters = $derived(
		controller.page?.parameters.find((binding) => binding.surface === surface?.reference.surface)
	);
	const cropSelected = $derived(
		!!cropParameters &&
			[cropParameters.crop_origin, cropParameters.crop_size].includes(
				appState.session?.selectedNodeUuid ?? ''
			)
	);
	const screenHandles = createScreenHandles(
		() =>
			editor === 'screen' && surface && controller.page
				? {
						document: controller.page.document,
						reference: surface.reference,
						include_candidates: choice.snap,
						cursor: null
					}
				: null,
		() => gestureState.active
	);
	let cursors = $state<(string | null)[]>([null]);
	const relatedKey = $derived(
		JSON.stringify([
			appState.session?.projectGeneration,
			appState.session?.currentHistoryStateId,
			controller.page?.document,
			surface?.reference,
			surface?.definition.source,
			choice.related
		])
	);
	$effect(() => {
		void relatedKey;
		untrack(() => {
			cursors = [null];
		});
	});
	const related = createRelatedSurfaces(() =>
		editor === 'source' && choice.related && surface && controller.page
			? {
					document: controller.page.document,
					reference: surface.reference,
					cursor: cursors.at(-1) ?? null
				}
			: null
	);
	let canonical = $state('');
	$effect(() => {
		const source = related.page?.source;
		if (!related.page) return;
		const next = JSON.stringify(source);
		untrack(() => {
			if (canonical && canonical !== next) cursors = [null];
			canonical = next;
		});
	});
	function persist() {
		writePanelPersistedState(panelApi, { ...choice });
	}
	$effect(() => {
		const page = controller.page;
		if (!page?.screen || !page.target || !page.document_matches) return;
		const selected = page.selected_surface;
		if (
			choice.document === page.document &&
			JSON.stringify(choice.target) === JSON.stringify(page.target) &&
			choice.surface === selected
		)
			return;
		choice.document = page.document;
		choice.target = page.target;
		choice.surface = selected;
		untrack(persist);
	});
	$effect(() => {
		panelApi.setContentBinding({
			locked: choice.locked,
			canLock: !!screen && (editor === 'screen' || !!surface),
			toggle: () => {
				gesture.stop();
				choice.locked = !choice.locked;
				persist();
			}
		});
	});
	followPanelTitle(
		() => panelApi,
		() => (editor === 'screen' ? 'Screen Warping' : 'Surface Source'),
		() => (editor === 'screen' ? screen?.label : surface?.label)
	);
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
		selection.select(uuid ?? screen.id);
	}
	function fitSurface(mode: SurfaceFitMode) {
		if (!surface || gestureState.active) return;
		void controller.edit({ kind: 'FitSurface', reference: surface.reference, mode });
	}
	function selectCrop(handle: 'origin' | 'size') {
		if (gestureState.active || !cropParameters) return false;
		return selection.select(
			handle === 'origin' ? cropParameters.crop_origin : cropParameters.crop_size
		);
	}
	function clearCrop() {
		if (!gestureState.active && surface) selection.select(surface.reference.surface);
	}
</script>

<section
	class="workbench-editor"
	aria-label={panel?.title ?? title ?? (editor === 'screen' ? 'Screen Warping' : 'Surface Source')}
	data-panel-id={panelId}
	data-panel-type={panelType}
	data-screen-target={screen?.id ?? ''}
	data-surface-target={surface?.reference.surface ?? ''}
	data-content-locked={choice.locked}
	aria-busy={controller.busy ||
		gestureState.active ||
		(editor === 'screen' && choice.snap && screenHandles.loading)}>
	{#snippet actions()}
		{#if editor === 'screen'}
			<IconButton
				class="screen-snap-toggle"
				icon={magnetIcon}
				label="Snap Surface handles"
				aria-pressed={choice.snap}
				title={screenHandles.error ||
					'Snap to other Surfaces in this Screen and link corners; existing links stay active when off'}
				disabled={!surface || gestureState.active}
				onclick={() => {
					choice.snap = !choice.snap;
					persist();
				}} />
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
		{:else}
			<IconButton
				icon={relatedIcon}
				label="Show related Surfaces"
				aria-pressed={choice.related}
				title={related.error || 'Show Surfaces using the same source'}
				disabled={!surface || gestureState.active}
				onclick={() => {
					choice.related = !choice.related;
					persist();
				}} />
			{#if choice.related}
				<IconButton
					icon={previousIcon}
					label="Previous related Surfaces"
					disabled={cursors.length < 2 || related.loading}
					onclick={() => {
						cursors = cursors.slice(0, -1);
					}} />
				<IconButton
					icon={nextIcon}
					label="Next related Surfaces"
					disabled={!related.page?.next || related.loading}
					onclick={() => {
						if (related.page?.next) cursors = [...cursors, related.page.next];
					}} />
			{/if}
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
		onsource={async (source, target, event) => {
			if (!screen) return;
			const selectDropped = selection.capture();
			const controls =
				event.currentTarget instanceof HTMLElement
					? event.currentTarget.querySelector('svg[aria-label="Surface mapping controls"]')
					: null;
			const rect = controls?.getBoundingClientRect();
			const explicit =
				event.target instanceof Element && event.target.closest('button[data-source-drop-object]');
			const result = await controller.edit(
				editor === 'screen' && !explicit && rect && rect.width > 0 && rect.height > 0
					? {
							kind: 'DropSourceAt',
							screen: screen.id,
							point: {
								x: (event.clientX - rect.left) / rect.width,
								y: (event.clientY - rect.top) / rect.height
							},
							source
						}
					: { kind: 'DropSource', screen: screen.id, surface: target, source }
			);
			if (result?.kind === 'SurfaceCreated') selectDropped(result.reference.surface);
			else if (result?.kind === 'SurfaceChanged') selectDropped(result.snapshot.reference.surface);
		}}>
		{#if view.error || view.readError || gestureState.error}<p class="editor-error" role="alert">
				{view.error || view.readError || gestureState.error}
			</p>{/if}
		{#if editor === 'screen' && screen}
			<ScreenPreview
				snapping={choice.snap}
				candidates={screenHandles.candidates}
				groups={screenHandles.page?.groups ?? []}
				unlink={(corner) => {
					if (surface)
						void controller.edit({
							kind: 'UnlinkScreenCorners',
							member: { reference: surface.reference, corner }
						});
				}}
				{inputAvailable}
				presentation={owner.presentation}
				{actions}
				{screen}
				selected={surface?.reference.surface ?? null}
				disabled={controller.busy}
				select={selectSurface}
				start={(reference, handle) => gesture.start(reference, handle)}
				pixelsActive={() => gesture.pixelsActive}
				gestureActive={gestureState.active}
				move={(point) => gesture.move(point)}
				finish={(link) => owner.finishWarp(link ?? null)}
				stop={() => gesture.stop()} />
		{:else if editor === 'source' && surface}
			<SurfaceSourcePreview
				preview={controller.page?.source_preview ?? null}
				{actions}
				related={related.surfaces}
				selectRelated={(uuid) => {
					if (inputAvailable && !gestureState.active) selection.select(uuid);
				}}
				{inputAvailable}
				presentation={owner.presentation}
				{surface}
				selected={cropSelected}
				select={selectCrop}
				onselectionclear={clearCrop}
				disabled={controller.busy}
				start={(handle) => gesture.start(surface.reference, handle)}
				move={(point) => gesture.move(point)}
				finish={() => gesture.finish()}
				stop={() => gesture.stop()} />
		{:else}<p class="editor-empty">
				{choice.locked
					? 'Locked target unavailable in this project.'
					: `Select ${editor === 'screen' ? 'a Screen' : 'a Surface'} in Screens.`}
			</p>{/if}
	</SourceDropZone>
</section>

<style>
	:global(.screen-snap-toggle[aria-pressed='false'] img) {
		filter: grayscale(1);
		opacity: 0.45;
	}
	:global(.screen-snap-toggle[aria-pressed='true'] img) {
		filter: none;
		opacity: 1;
	}
</style>
