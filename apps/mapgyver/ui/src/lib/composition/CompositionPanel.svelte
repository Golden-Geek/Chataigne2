<script lang="ts">
	import { IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import resetIcon from 'golden_ui/style/icons/reset.svg';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import { useCompositionAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import CompositionPreview from './CompositionPreview.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const owner = useCompositionAuthoring();
	const { view, controller, gestureState, gesture } = owner;
	const channel = $derived(owner.channel),
		busy = $derived(owner.busy);
	const selection = useDocumentSelection();
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
	<div class="editor-canvas">
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
	</div>
</section>
