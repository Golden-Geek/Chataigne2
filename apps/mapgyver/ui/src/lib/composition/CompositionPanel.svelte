<script lang="ts">
	import { IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import resetIcon from 'golden_ui/style/icons/reset.svg';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import { useCompositionAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import CompositionPreview from './CompositionPreview.svelte';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import { pickCompositionSource } from '../media/source-hit';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const owner = useCompositionAuthoring();
	const { view, controller, gestureState, gesture } = owner;
	const channel = $derived(owner.channel),
		busy = $derived(owner.busy);
	const selection = useDocumentSelection();
	const preview = usePreview();
	function dropPoint(event: DragEvent) {
		const composition = view.composition;
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
		const point = dropPoint(event),
			composition = view.composition;
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
		if (!view.instance) return;
		const selectCreated = selection.capture();
		const result = await controller.edit({
			kind: 'DuplicateInstances',
			instances: [view.instance]
		});
		if (result?.kind === 'InstancesCreated' && result.instances[0])
			selectCreated(result.instances[0].instance);
	}
	async function remove() {
		if (!view.instance) return;
		const selectParent = selection.capture(),
			parent = view.selected?.channel;
		const result = await controller.edit({ kind: 'RemoveInstance', instance: view.instance });
		if (result?.kind === 'Changed' && parent) selectParent(parent);
	}
</script>

<section
	class="workbench-editor"
	aria-label={props.title ?? 'Composition'}
	data-panel-id={props.panelId}>
	<div class="editor-toolbar">
		<span>{channel?.label ?? 'Composition'}</span>
		<IconButton
			icon={addIcon}
			label="Add Composition"
			disabled={busy}
			onclick={async () => {
				const selectCreated = selection.capture();
				const result = await controller.create();
				if (result?.kind === 'Created') selectCreated(result.reference.channel);
			}} />
		<IconButton
			icon={resetIcon}
			label="Refresh Composition"
			disabled={busy}
			onclick={() => controller.refresh()} />
		<IconButton
			icon={copyIcon}
			label="Copy instance"
			disabled={busy || !view.instance}
			onclick={duplicate} />
		<IconButton
			icon={deleteIcon}
			label="Remove instance"
			disabled={busy || !view.instance}
			onclick={remove} />
	</div>
	<SourceDropZone
		class="editor-canvas"
		data-composition-drop-target=""
		addLabel="Add instance"
		hitTest={pickDropTarget}
		disabled={busy || gestureState.active || !view.composition}
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
		{#if view.error || gestureState.error}<p class="editor-error" role="alert">
				{view.error || gestureState.error}
			</p>{/if}
		{#if channel && view.composition}
			<CompositionPreview
				{channel}
				composition={view.composition}
				selected={view.instance}
				disabled={busy}
				active={gestureState.active}
				select={(id) => {
					view.instance = id;
					if (id) selection.select(id);
				}}
				start={(reference, handle) => gesture.start(reference, handle)}
				move={(point) => gesture.move(point)}
				finish={() => gesture.finish()}
				stop={() => gesture.stop()} />
		{:else}<p class="editor-empty">Select a Composition channel in Media Manager.</p>{/if}
	</SourceDropZone>
</section>
