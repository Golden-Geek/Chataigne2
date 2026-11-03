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
	import lockIcon from 'golden_ui/style/icons/lock.svg';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { createCompositionEditor } from './editor.svelte';
	import {
		compositionPanelState,
		COMPOSITION_PAGE_SIZE,
		type CompositionPanelState
	} from './panel-state';
	import { useDocumentSelection } from '../selection.svelte';
	import CompositionPreview from './CompositionPreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { pickCompositionSource } from '../media/source-hit';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	let props: PanelProps = $props();
	let choice = $state(
		untrack(() =>
			compositionPanelState(readPanelPersistedState<CompositionPanelState>(props.params))
		)
	);
	let title = $state(untrack(() => props.title));
	let selectedInstance = $state<string | null>(null);
	export const setPanelState = (next: PanelState) => {
		owner.gesture.stop();
		title = next.title;
		choice = compositionPanelState(readPanelPersistedState<CompositionPanelState>(next.params));
		selectedInstance = null;
	};
	const owner = createCompositionEditor(() => ({
		selected: appState.session?.getSelectedNodes()[0]?.uuid ?? null,
		document: choice.locked ? choice.document : null,
		locked: choice.locked,
		instance: selectedInstance,
		start: choice.page * COMPOSITION_PAGE_SIZE,
		count: COMPOSITION_PAGE_SIZE
	}));
	const { view, controller, gestureState, gesture } = owner;
	const channel = $derived(owner.channel),
		composition = $derived(owner.composition),
		instance = $derived(owner.instance),
		busy = $derived(owner.busy);
	const pages = $derived(Math.max(1, Math.ceil((view.page?.total ?? 0) / COMPOSITION_PAGE_SIZE)));
	const selection = useDocumentSelection(),
		preview = usePreview();
	function persist() {
		writePanelPersistedState(props.panelApi, { ...choice });
	}
	$effect(() => {
		if (instance) selectedInstance = instance.reference.instance;
	});
	$effect(() => {
		if (view.page?.composition && choice.page >= pages) {
			choice.page = pages - 1;
			untrack(persist);
		}
	});
	function dropPoint(event: DragEvent) {
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
	aria-label={title ?? 'Composition'}
	data-panel-id={props.panelId}
	data-composition-channel={channel?.reference.channel ?? ''}
	data-composition-selected={instance?.reference.instance ?? ''}
	data-composition-locked={!!choice.locked}
	data-composition-loading={view.refreshing}
	data-composition-busy={busy}>
	<div class="editor-toolbar">
		<IconButton
			icon={lockIcon}
			label={choice.locked ? 'Unlock Composition' : 'Lock Composition'}
			disabled={busy || (!choice.locked && !channel)}
			onclick={() => {
				gesture.stop();
				if (choice.locked) {
					choice.locked = null;
					choice.document = null;
				} else if (channel && view.page) {
					choice.locked = channel.reference;
					choice.document = view.page.document;
				}
				persist();
			}} />
		<IconButton
			icon={copyIcon}
			label="Copy instance"
			disabled={busy || !instance}
			onclick={duplicate} />
		<IconButton
			icon={deleteIcon}
			label="Remove instance"
			disabled={busy || !instance}
			onclick={remove} />
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous instances"
				class="previous"
				disabled={busy || choice.page === 0}
				onclick={() => {
					choice.page--;
					persist();
				}} />
			<span>Page {choice.page + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next instances"
				disabled={busy || choice.page >= pages - 1}
				onclick={() => {
					choice.page++;
					persist();
				}} />
		{/if}
	</div>
	<SourceDropZone
		class="editor-canvas"
		data-composition-drop-target=""
		addLabel="Add instance"
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
		{#if channel && composition}
			<CompositionPreview
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
		{:else if view.refreshing}<p class="editor-empty" role="status">Loading Composition…</p>
		{:else if choice.locked}<p class="editor-empty">
				Locked Composition unavailable in this project.
			</p>
		{:else}<p class="editor-empty">Select a Composition in Media Manager.</p>{/if}
	</SourceDropZone>
</section>

<style>
	:global(.previous img) {
		transform: rotate(180deg);
	}
</style>
