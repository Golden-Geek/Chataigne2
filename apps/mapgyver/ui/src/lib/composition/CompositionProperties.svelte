<script lang="ts">
	import { IconButton } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import duplicateIcon from 'golden_ui/style/icons/duplicate.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import type { Snippet } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import { useCompositionAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import { channelReferenceFromKey } from '../media/identity';
	import type { CompositionEdit } from '../media/generated/CompositionEdit';
	import SourcePicker from './SourcePicker.svelte';
	import InstanceForm from './InstanceForm.svelte';
	import CompositionChannelEditor from '../media/CompositionChannelEditor.svelte';
	let { node, defaultHeader }: { node?: UiNodeDto; defaultHeader?: Snippet } = $props();
	const owner = useCompositionAuthoring();
	const { view, controller } = owner;
	const busy = $derived(owner.busy),
		instance = $derived(owner.instance),
		instances = $derived(owner.instances),
		channel = $derived(owner.channel);
	const pages = $derived(owner.pages),
		currentPage = $derived(owner.currentPage),
		start = $derived(owner.start),
		pageSize = owner.pageSize;
	function add(event: SubmitEvent & { currentTarget: HTMLFormElement }) {
		event.preventDefault();
		const key = String(new FormData(event.currentTarget).get('source'));
		if (key) void edit({ kind: 'AddInstances', sources: [channelReferenceFromKey(key)] });
	}
	const selection = useDocumentSelection();
	async function edit(intent: CompositionEdit) {
		const selectResult = selection.capture();
		const parent = view.selected?.channel;
		const result = await controller.edit(intent);
		if (result?.kind === 'InstancesCreated' && result.instances[0])
			selectResult(result.instances[0].instance);
		else if (result?.kind === 'Changed' && intent.kind === 'RemoveInstance' && parent)
			selectResult(parent);
		return result;
	}
</script>

{@render defaultHeader?.()}
<div class="properties">
	{#if view.catalog}
		{#if !node || node.node_type === 'mapgyver.media.channel'}
			{#if channel}<CompositionChannelEditor
					{channel}
					disabled={busy}
					configure={async (definition) => {
						await controller.configure(definition);
					}} />{/if}
		{/if}
		<form onsubmit={add}>
			<fieldset disabled={busy || !view.composition}>
				<legend>Add source instance</legend>
				<SourcePicker catalog={view.catalog} required emptyLabel="Choose a channel" />
				<IconButton icon={addIcon} label="Add instance" type="submit" />
			</fieldset>
		</form>
		{#if view.composition}
			<nav aria-label="Ordered instances">
				<p>Draw order: the last instance appears on top.</p>
				{#if pages > 1}
					<div class="actions">
						<IconButton
							icon={arrowIcon}
							label="Previous instances"
							class="previous"
							disabled={busy || currentPage === 0}
							onclick={() => {
								owner.page = currentPage - 1;
							}} />
						<span>Page {currentPage + 1} / {pages}</span>
						<IconButton
							icon={arrowIcon}
							label="Next instances"
							disabled={busy || currentPage === pages - 1}
							onclick={() => {
								owner.page = currentPage + 1;
							}} />
					</div>
				{/if}
				{#each instances.slice(start, start + pageSize) as item, offset (item.reference.instance)}
					{@const index = start + offset}
					<div class="instance-row">
						<button
							class="name"
							aria-pressed={view.instance === item.reference.instance}
							disabled={busy}
							onclick={() => {
								view.instance = item.reference.instance;
								selection.select(item.reference.instance);
							}}>{item.label}</button>
						<button
							title="Move earlier"
							aria-label={`Move ${item.label} earlier`}
							disabled={busy || index === 0}
							onclick={() =>
								edit({
									kind: 'ReorderInstance',
									instance: item.reference.instance,
									after: view.composition?.instances[index - 2]?.reference.instance ?? null
								})}>↑</button>
						<button
							title="Move later"
							aria-label={`Move ${item.label} later`}
							disabled={busy || index === view.composition.instances.length - 1}
							onclick={() =>
								edit({
									kind: 'ReorderInstance',
									instance: item.reference.instance,
									after: view.composition!.instances[index + 1].reference.instance
								})}>↓</button>
					</div>
				{:else}<p>No instances. Choose a source channel to begin.</p>{/each}
			</nav>
		{/if}
		{#if instance}
			<div class="actions">
				<IconButton
					icon={duplicateIcon}
					label="Duplicate"
					disabled={busy}
					onclick={() =>
						edit({
							kind: 'DuplicateInstances',
							instances: [instance.reference.instance]
						})} />
				<IconButton
					icon={deleteIcon}
					label="Remove"
					disabled={busy}
					onclick={() =>
						edit({
							kind: 'RemoveInstance',
							instance: instance.reference.instance
						})} />
			</div>
			{#key instance.reference.instance}
				<InstanceForm
					{instance}
					catalog={view.catalog}
					disabled={busy}
					configure={(definition) =>
						edit({
							kind: 'ConfigureInstance',
							instance: instance.reference.instance,
							definition
						})}
					rename={(label) =>
						edit({
							kind: 'RenameInstance',
							instance: instance.reference.instance,
							label
						})} />
			{/key}
		{/if}
	{/if}
</div>

<style>
	:global(.previous img) {
		transform: rotate(180deg);
	}
	.properties {
		display: grid;
		gap: 0.5rem;
		padding: 0.5rem;
		min-width: 0;
	}
	.actions,
	.instance-row {
		display: flex;
		gap: 0.25rem;
		align-items: center;
	}
	.name {
		flex: 1;
		text-align: left;
	}
	p {
		font-size: 0.8rem;
	}
	nav {
		display: grid;
		gap: 0.2rem;
	}
</style>
