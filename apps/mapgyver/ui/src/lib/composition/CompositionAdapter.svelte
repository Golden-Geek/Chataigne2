<script lang="ts">
	import { CanvasToolbar } from 'golden_graph_ui';
	import {
		appState,
		ContextMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		IconButton
	} from 'golden_ui';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/panels/close.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { mediaIcons } from '../icons/tree';
	import { createCompositionEditor } from './editor.svelte';
	import { SPATIAL_PAGE_SIZE } from '../spatial/targets';
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import { useDocumentSelection } from '../selection.svelte';
	import CompositionPreview from './CompositionPreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { pickCompositionSource } from '../media/source-hit';
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
		selected: appState.session?.getSelectedNodes()[0]?.uuid ?? null,
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
		if (instance) selectedInstance = instance.reference.instance;
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
	function dropPoint(event: MouseEvent) {
		const canvas = (event.currentTarget as HTMLElement).querySelector(
			'svg[aria-label="Composition transform controls"]'
		);
		const rect = canvas?.getBoundingClientRect();
		if (!composition || !rect || rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: ((event.clientX - rect.left) / rect.width) * composition.canvas.width,
			y: ((event.clientY - rect.top) / rect.height) * composition.canvas.height
		};
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
	async function duplicate() {
		if (!instance) return;
		const selectCreated = selection.capture();
		const result = await controller.edit({
			kind: 'DuplicateInstances',
			instances: [instance.reference.instance]
		});
		if (result?.kind === 'InstancesCreated' && result.instances[0])
			selectCreated(result.instances[0].instance);
	}
	async function remove() {
		if (!instance) return;
		const selectParent = selection.capture(),
			parent = channel?.reference.channel;
		const result = await controller.edit({
			kind: 'RemoveInstance',
			instance: instance.reference.instance
		});
		if (result?.kind === 'Changed' && parent) selectParent(parent);
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
			icon={copyIcon}
			label="Copy instance"
			disabled={busy || !instance}
			onclick={duplicate} />
		<IconButton
			icon={deleteIcon}
			label="Remove instance"
			class="mapgyver-remove-action"
			disabled={busy || !instance}
			onclick={remove} />
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
				selected={instance?.reference.instance ?? null}
				disabled={busy}
				active={gestureState.active}
				select={(id) => {
					selectedInstance = id;
					if (id) selection.select(id);
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
	:global(.canvas-toolbar:not(:hover):not(:focus-within) .mapgyver-remove-action) {
		opacity: 0;
		pointer-events: none;
	}
	:global(.previous img) {
		transform: rotate(180deg);
	}
</style>
