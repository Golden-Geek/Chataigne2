<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState, type WorkbenchSession } from 'golden_ui';
	import { createCompositionAuthoringState, CompositionAuthoringController } from './authoring';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { channelKey, channelReferenceFromKey } from '../media/identity';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import CompositionChannelEditor from '../media/CompositionChannelEditor.svelte';
	import SourcePicker from './SourcePicker.svelte';
	import InstanceForm from './InstanceForm.svelte';
	let { panelId, panelType, title }: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
	const session = $derived(appState.session);
	const view = $state(createCompositionAuthoringState());
	const controller = new CompositionAuthoringController<WorkbenchSession>(
		() => session,
		view,
		invokeMediaEdit
	);
	const channel = $derived(controller.channel);
	const instance = $derived(controller.instance);
	const busy = $derived(controller.busy);
	const pageSize = 50;
	let page = $state(0);
	const instances = $derived(view.composition?.instances ?? []);
	const pages = $derived(Math.max(1, Math.ceil(instances.length / pageSize)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const start = $derived(currentPage * pageSize);
	const selectedIndex = $derived(
		instances.findIndex((item) => item.reference.instance === view.instance)
	);
	$effect(() => {
		if (selectedIndex >= 0) page = Math.floor(selectedIndex / pageSize);
	});
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			void controller.refresh();
		});
	});
	onDestroy(() => controller.dispose());

	function add(event: SubmitEvent & { currentTarget: HTMLFormElement }) {
		event.preventDefault();
		const key = String(new FormData(event.currentTarget).get('source'));
		if (key)
			void controller.edit({ kind: 'AddInstances', sources: [channelReferenceFromKey(key)] });
	}
</script>

<section
	class="composition"
	aria-label={panel?.title ?? title ?? 'Composition'}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<header>
		<div>
			<h2>Composition</h2>
			<p>Arrange independent instances of reusable media channels.</p>
		</div>
		<button disabled={busy} onclick={() => controller.create()}>Add Composition</button>
		<button
			disabled={view.pending ||
				view.refreshing ||
				session?.status !== 'connected' ||
				session.loading.visible}
			onclick={() => {
				view.error = '';
				void controller.refresh();
			}}>Refresh</button>
	</header>
	{#if view.error}<p class="error" role="alert">{view.error}</p>{/if}
	{#if session?.status !== 'connected'}
		<p>Connect to MapGyver to edit Compositions.</p>
	{:else if !view.catalog}
		<p>Loading media…</p>
	{:else if controller.channels.length === 0}
		<p>Add a Composition, then choose source channels for its instances.</p>
	{:else}
		<label
			>Composition channel
			<select
				disabled={view.pending}
				value={view.selected ? channelKey(view.selected) : ''}
				onchange={(event) => {
					const channel = controller.channels.find(
						(channel) => channelKey(channel.reference) === event.currentTarget.value
					);
					if (channel) controller.select(channel.reference);
				}}>
				{#each view.catalog.media as media (media.id)}
					{#each media.channels.filter((channel) => channel.definition.source.kind === 'Composition') as channel (channel.reference.channel)}
						<option value={channelKey(channel.reference)}>{media.label} / {channel.label}</option>
					{/each}
				{/each}
			</select>
		</label>
		{#if channel}
			<div class="workspace">
				<div class="canvas">
					<ChannelPreview {channel} />
					<CompositionChannelEditor
						{channel}
						disabled={busy}
						configure={async (definition) => {
							await controller.configure(definition);
						}} />
				</div>
				<div class="instances">
					<form onsubmit={add}>
						<fieldset disabled={busy || !view.composition}>
							<legend>Add source instance</legend>
							<SourcePicker catalog={view.catalog} required emptyLabel="Choose a channel" />
							<button>Add instance</button>
						</fieldset>
					</form>
					{#if view.composition}
						<nav aria-label="Ordered instances">
							<p>Draw order: the last instance appears on top.</p>
							{#if pages > 1}
								<div class="actions">
									<button
										disabled={busy || currentPage === 0}
										onclick={() => {
											page = currentPage - 1;
										}}>Previous instances</button>
									<span>Page {currentPage + 1} / {pages}</span>
									<button
										disabled={busy || currentPage === pages - 1}
										onclick={() => {
											page = currentPage + 1;
										}}>Next instances</button>
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
										}}>{item.label}</button>
									<button
										title="Move earlier"
										aria-label={`Move ${item.label} earlier`}
										disabled={busy || index === 0}
										onclick={() =>
											controller.edit({
												kind: 'ReorderInstance',
												instance: item.reference.instance,
												after: view.composition?.instances[index - 2]?.reference.instance ?? null
											})}>↑</button>
									<button
										title="Move later"
										aria-label={`Move ${item.label} later`}
										disabled={busy || index === view.composition.instances.length - 1}
										onclick={() =>
											controller.edit({
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
							<button
								disabled={busy}
								onclick={() =>
									controller.edit({
										kind: 'DuplicateInstances',
										instances: [instance.reference.instance]
									})}>Duplicate</button>
							<button
								disabled={busy}
								onclick={() =>
									controller.edit({
										kind: 'RemoveInstance',
										instance: instance.reference.instance
									})}>Remove</button>
						</div>
						{#key instance.reference.instance}
							<InstanceForm
								{instance}
								catalog={view.catalog}
								disabled={busy}
								configure={(definition) =>
									controller.edit({
										kind: 'ConfigureInstance',
										instance: instance.reference.instance,
										definition
									})}
								rename={(label) =>
									controller.edit({
										kind: 'RenameInstance',
										instance: instance.reference.instance,
										label
									})} />
						{/key}
					{/if}
				</div>
			</div>
		{/if}
	{/if}
</section>

<style>
	.composition {
		height: 100%;
		overflow: auto;
		padding: 1rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		background: var(--mapgyver-panel-background, #171a20);
	}
	header,
	.actions {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.6rem;
	}
	header > div {
		flex: 1;
	}
	h2,
	p {
		margin: 0 0 0.4em;
	}
	.workspace {
		display: grid;
		grid-template-columns: minmax(0, 2fr) minmax(15rem, 1fr);
		gap: 1rem;
	}
	.canvas,
	.instances {
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	fieldset,
	label {
		display: grid;
		gap: 0.5rem;
		min-width: 0;
	}
	fieldset {
		border: 0.0625rem solid #444b57;
		border-radius: 0.4rem;
		padding: 0.75rem;
	}
	.instance-row {
		display: flex;
		gap: 0.3rem;
		margin-bottom: 0.3rem;
	}
	.name {
		flex: 1;
		overflow-wrap: anywhere;
		text-align: left;
	}
	[aria-pressed='true'] {
		outline: 0.12rem solid #72a9ef;
	}
	.error {
		color: #ffb3b3;
	}
	@media (max-width: 55rem) {
		.workspace {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
