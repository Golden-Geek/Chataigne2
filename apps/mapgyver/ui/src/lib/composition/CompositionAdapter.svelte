<script lang="ts">
	import { CanvasToolbar } from 'golden_graph_ui';
	import {
		appState,
		ContextMenu,
		openNodeContextMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		IconButton
	} from 'golden_ui';
	import fillIcon from '../screens/icons/fill.svg';
	import fitIcon from '../screens/icons/fit.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { mediaIcons } from '../icons/tree';
	import { createCompositionEditor } from './editor.svelte';
	import { SPATIAL_PAGE_SIZE } from '../spatial/targets';
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import { useDocumentSelection } from '../selection.svelte';
	import CompositionPreview from './CompositionPreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { pickCompositionSource } from '../media/source-hit';
	import { mediaFileDrop } from '../media/file-drop';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	let props: {
		panelId: string;
		title: string;
		reference: ChannelReference;
		document: string;
		present: boolean;
		locked: boolean;
		page: number;
		onpagechange(page: number): void;
	} = $props();
	let selectedInstance = $state<string | null>(null);
	const owner = createCompositionEditor(() => ({
		selected: appState.session?.selectedNodeUuid ?? null,
		document: props.document,
		locked: props.reference,
		retained: props.reference,
		instance: selectedInstance,
		start: props.page * SPATIAL_PAGE_SIZE,
		count: SPATIAL_PAGE_SIZE
	}));
	const { view, controller, gestureState, gesture } = owner;
	const channel = $derived(owner.channel),
		composition = $derived(owner.composition),
		instance = $derived(owner.instance),
		busy = $derived(owner.busy || !props.present);
	const pages = $derived(Math.max(1, Math.ceil((view.page?.total ?? 0) / SPATIAL_PAGE_SIZE)));
	const selection = useDocumentSelection(),
		preview = usePreview();
	$effect(() => {
		selectedInstance = view.page?.selected?.reference.instance ?? null;
	});
	$effect(() => {
		if (view.page?.composition && props.page >= pages) {
			props.onpagechange(pages - 1);
		}
	});
	let menuAnchor = $state<ContextMenuAnchor | null>(null);
	let menuItems = $state.raw<ContextMenuItem[]>([]);
	const menuChannel = $derived(channel?.reference.channel ?? null);
	$effect(() => {
		void menuChannel;
		void appState.session?.snapshotGeneration;
		menuAnchor = null;
	});
	function creationMenu(event: MouseEvent) {
		if (busy || gestureState.active || !channel) return;
		const target =
			event.target instanceof Element
				? (event.target
						.closest('[data-instance-drop-target]')
						?.getAttribute('data-instance-drop-target') ??
					(event.target.closest('[data-instance-handle]') ? instance?.reference.instance : null))
				: null;
		if (target) {
			const node = appState.session?.graph.state.nodesById.getByUuid(target);
			if (!node) return;
			event.preventDefault();
			event.stopPropagation();
			selection.select(target);
			openNodeContextMenu(node.node_id, event.clientX, event.clientY);
			return;
		}
		const position = dropPoint(event);
		if (!position) return;
		event.preventDefault();
		event.stopPropagation();
		const reference = channel.reference;
		const session = appState.session,
			generation = session?.snapshotGeneration;
		menuItems = (view.page?.media_types ?? []).map((type) => ({
			label: type.label,
			icon: mediaIcons[type.media_kind],
			action: async () => {
				if (
					appState.session !== session ||
					session?.snapshotGeneration !== generation ||
					channel?.reference.channel !== reference.channel ||
					channel.reference.media !== reference.media
				)
					return;
				const selectCreated = selection.capture();
				const result = await controller.edit({
					kind: 'CreateMedia',
					media_kind: type.media_kind,
					label: null,
					position
				});
				if (result?.kind === 'InstancesCreated' && result.instances[0])
					selectCreated(result.instances[0].instance);
			}
		}));
		menuAnchor = { kind: 'point', x: event.clientX, y: event.clientY };
	}
	function dropPoint(event: MouseEvent, area = event.currentTarget as HTMLElement) {
		const canvas = area.querySelector('svg[aria-label="Composition transform controls"]');
		const rect = canvas?.getBoundingClientRect();
		if (!composition || !rect || rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: ((event.clientX - rect.left) / rect.width) * composition.canvas.width,
			y: ((event.clientY - rect.top) / rect.height) * composition.canvas.height
		};
	}
	function fileDestination(area: HTMLDivElement, event: DragEvent) {
		const position = dropPoint(event, area);
		const reference = channel?.reference;
		if (!position || !reference) return null;
		return mediaFileDrop(
			{ bounds: area, label: 'Add Media to Composition', action: 'add' },
			async (path) => {
				if (
					channel?.reference.media !== reference.media ||
					channel.reference.channel !== reference.channel
				)
					return;
				const selectCreated = selection.capture();
				const result = await controller.edit({ kind: 'DropMediaFile', path, position });
				if (result?.kind === 'InstancesCreated' && result.instances[0])
					selectCreated(result.instances[0].instance);
			}
		);
	}
	function pickDropTarget(event: DragEvent) {
		const point = dropPoint(event);
		return point && composition
			? pickCompositionSource(
					point,
					composition,
					(reference) =>
						preview.statuses.get(previewKey({ kind: 'Channel', reference }))?.extent ?? null
				)
			: null;
	}
</script>

<section
	class="workbench-editor"
	aria-label={props.title}
	data-panel-id={props.panelId}
	data-spatial-kind="Composition"
	data-spatial-media={props.present ? (channel?.reference.media ?? '') : ''}
	data-composition-channel={props.present ? (channel?.reference.channel ?? '') : ''}
	data-composition-selected={instance?.reference.instance ?? ''}
	data-composition-locked={props.locked}
	data-composition-loading={view.refreshing}
	data-composition-busy={busy}>
	{#snippet actions()}
		<IconButton
			icon={fillIcon}
			label="Fill Composition"
			title="Stretch media to Composition bounds"
			disabled={busy || !instance}
			onclick={() =>
				instance &&
				controller.edit({
					kind: 'TransformInstance',
					instance: instance.reference.instance,
					action: 'Fill'
				})} />
		<IconButton
			icon={fitIcon}
			label="Fit to Composition"
			title="Center media with its source aspect preserved"
			disabled={busy || !instance}
			onclick={() =>
				instance &&
				controller.edit({
					kind: 'TransformInstance',
					instance: instance.reference.instance,
					action: 'Fit'
				})} />
		<IconButton
			icon={arrowIcon}
			label="To Front"
			title="Place media in front of every other item"
			class="composition-to-front"
			disabled={busy || !instance}
			onclick={() =>
				instance &&
				controller.edit({
					kind: 'ArrangeInstance',
					instance: instance.reference.instance,
					action: 'Front'
				})} />
		<IconButton
			icon={arrowIcon}
			label="To Back"
			title="Place media behind every other item"
			class="composition-to-back"
			disabled={busy || !instance}
			onclick={() =>
				instance &&
				controller.edit({
					kind: 'ArrangeInstance',
					instance: instance.reference.instance,
					action: 'Back'
				})} />
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous instances"
				class="previous"
				disabled={busy || props.page === 0}
				onclick={() => props.onpagechange(props.page - 1)} />
			<span>Page {props.page + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next instances"
				disabled={busy || props.page >= pages - 1}
				onclick={() => props.onpagechange(props.page + 1)} />
		{/if}
	{/snippet}
	<SourceDropZone
		class="editor-canvas"
		data-composition-drop-target=""
		data-local-context-menu
		addLabel="Add instance"
		oncontextmenu={creationMenu}
		hitTest={pickDropTarget}
		{fileDestination}
		disabled={busy || gestureState.active || !composition}
		onsource={async (source, target, event) => {
			const position = dropPoint(event);
			if (!position) return;
			const selectDropped = selection.capture();
			const result = await controller.edit({
				kind: 'DropSource',
				copy: event.altKey,
				source,
				instance: target,
				position
			});
			if (result?.kind === 'InstancesCreated' && result.instances[0])
				selectDropped(result.instances[0].instance);
			else if (result?.kind === 'InstanceChanged')
				selectDropped(result.snapshot.reference.instance);
		}}>
		{#if view.error || view.readError || gestureState.error}<p class="editor-error" role="alert">
				{view.error || view.readError || gestureState.error}
			</p>{/if}
		{#if props.present && channel && composition}
			<CompositionPreview
				presentation={owner.presentation}
				{actions}
				{channel}
				{composition}
				selected={selectedInstance}
				disabled={busy}
				selectionAvailable={appState.session?.status === 'connected' &&
					!appState.session.loading.visible}
				active={gestureState.active}
				select={(id) => {
					if (selection.select(id)) {
						controller.selectInstance(id);
						selectedInstance = id;
					}
				}}
				onselectionclear={() => {
					if (gestureState.active) return;
					if (selection.select(channel.reference.media)) selectedInstance = null;
				}}
				start={(reference, handle) => owner.start(reference, handle)}
				move={(point) => gesture.move(point)}
				finish={() => gesture.finish()}
				stop={() => gesture.stop()} />
		{:else if !props.present}<p class="editor-empty">
				{props.locked
					? 'Locked Composition unavailable in this project.'
					: 'Composition unavailable in this project.'}
			</p>
			<CanvasToolbar {actions} disabled frame={() => {}} home={() => {}} />
		{:else if view.refreshing}<p class="editor-empty" role="status">Loading Composition…</p>
		{:else if props.locked}<p class="editor-empty">
				Locked Composition unavailable in this project.
			</p>
			<CanvasToolbar {actions} disabled frame={() => {}} home={() => {}} />
		{:else}<p class="editor-empty">Select a Composition in Medias.</p>{/if}
	</SourceDropZone>
</section>
<ContextMenu
	open={menuAnchor !== null}
	anchor={menuAnchor}
	items={menuItems}
	onClose={() => {
		menuAnchor = null;
	}} />

<style>
	:global(.composition-to-front img) {
		transform: rotate(-90deg);
	}
	:global(.composition-to-back img) {
		transform: rotate(90deg);
	}
	:global(.previous img) {
		transform: rotate(180deg);
	}
</style>
