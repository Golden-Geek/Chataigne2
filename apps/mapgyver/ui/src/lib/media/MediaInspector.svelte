<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import type { MediaManagerSnapshot } from './generated/MediaManagerSnapshot';
	import type { MediaEdit } from './generated/MediaEdit';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import { invokeMediaEdit } from './operations';
	import { MEDIA_NAMESPACE } from './generated/constants';
	import ChannelPreview from './ChannelPreview.svelte';
	import SolidChannelEditor from './SolidChannelEditor.svelte';
	import PictureChannelEditor from './PictureChannelEditor.svelte';
	import SharedTextureChannelEditor from './SharedTextureChannelEditor.svelte';
	import CompositionProperties from '../composition/CompositionProperties.svelte';
	import MatrixProperties from '../matrix/MatrixProperties.svelte';
	import AssetLocatorForm from './AssetLocatorForm.svelte';
	import ChannelSettingsForm from './ChannelSettingsForm.svelte';
	let { node, defaultHeader }: { node: UiNodeDto; defaultHeader?: Snippet } = $props();
	const session = $derived(appState.session);
	let catalog = $state<MediaManagerSnapshot | null>(null),
		pending = $state(false),
		error = $state('');
	let owner = $state.raw<{ session: typeof session; generation: number } | null>(null);
	let read = 0,
		disposed = false;
	const currentCatalog = $derived(
		session?.status === 'connected' &&
			!session.loading.visible &&
			owner?.session === session &&
			owner.generation === session.snapshotGeneration
			? catalog
			: null
	);
	const media = $derived(
		currentCatalog?.media.find(
			(media) =>
				media.id === node.uuid ||
				media.channels.some((channel) => channel.reference.channel === node.uuid)
		)
	);
	const channel = $derived(
		media?.channels.find((channel) => channel.reference.channel === node.uuid)
	);
	const busy = $derived(
		pending ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
	);
	async function refresh() {
		const current = session,
			id = ++read;
		if (!current || current.status !== 'connected' || current.loading.visible) {
			catalog = null;
			owner = null;
			return;
		}
		if (current.hasActiveEditSession) return;
		const generation = current.snapshotGeneration;
		try {
			const result = await invokeMediaEdit(current, { kind: 'InspectManager' });
			if (
				disposed ||
				id !== read ||
				session !== current ||
				current.snapshotGeneration !== generation ||
				current.loading.visible
			)
				return;
			if (result.kind === 'Manager') {
				catalog = result.snapshot;
				owner = { session: current, generation };
				error = '';
			}
		} catch (cause) {
			if (
				!disposed &&
				id === read &&
				session === current &&
				current.snapshotGeneration === generation
			)
				error = String(cause);
		}
	}
	async function edit(intent: MediaEdit) {
		const current = session;
		if (!current || busy) return;
		const generation = current.snapshotGeneration,
			uuid = node.uuid;
		pending = true;
		error = '';
		try {
			const result = await invokeMediaEdit(current, intent);
			if (
				disposed ||
				session !== current ||
				generation !== current.snapshotGeneration ||
				node.uuid !== uuid
			)
				return;
			await refresh();
			return result;
		} catch (cause) {
			if (
				!disposed &&
				session === current &&
				generation === current.snapshotGeneration &&
				node.uuid === uuid
			)
				error = String(cause);
		} finally {
			pending = false;
		}
	}
	async function configure(definition: ChannelDefinition) {
		if (channel) await edit({ kind: 'ConfigureChannel', reference: channel.reference, definition });
	}
	$effect(() => {
		const current = session;
		if (current) {
			void current.currentHistoryStateId;
			void current.snapshotGeneration;
			void current.status;
			void current.loading.visible;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			void refresh();
		});
	});
	onDestroy(() => {
		disposed = true;
		++read;
	});
</script>

{@render defaultHeader?.()}
<div class="properties">
	{#if error}<p role="alert">{error}</p>{/if}
	{#if media}
		{#if node.node_type === 'mapgyver.media'}
			<div class="summary">
				<span>{media.media_kind}</span><span>{media.channels.length} channels</span>
				<IconButton
					icon={addIcon}
					label="Add channel"
					disabled={busy || media.media_kind === 'Matrix'}
					onclick={() => edit({ kind: 'AddChannel', media: media.id, label: null })} />
				<IconButton
					icon={deleteIcon}
					label="Delete media"
					disabled={busy}
					onclick={() => edit({ kind: 'RemoveMedia', media: media.id })} />
			</div>
		{/if}
		{#if channel}
			{#key channel.reference.channel}
				<ChannelPreview {channel} />
				{#if channel.definition.source.kind === 'SolidColor'}<SolidChannelEditor
						{channel}
						disabled={busy}
						{configure} />
				{:else if channel.definition.source.kind === 'Picture'}<PictureChannelEditor
						{channel}
						assets={currentCatalog?.assets ?? []}
						disabled={busy}
						{configure}
						{edit} />
				{:else if channel.definition.source.kind === 'Spout'}<SharedTextureChannelEditor
						{channel}
						disabled={busy}
						{configure} />
				{:else if channel.definition.source.kind === 'Composition'}<CompositionProperties {node} />
				{:else if channel.definition.source.kind === 'Matrix'}
					<ChannelSettingsForm
						{channel}
						source={channel.definition.source}
						title="Output channel"
						disabled={busy}
						{configure}>{#snippet children()}{/snippet}</ChannelSettingsForm>
					<MatrixProperties />
				{/if}
				<IconButton
					icon={deleteIcon}
					label="Delete channel"
					disabled={busy || media.channels.length === 1 || media.media_kind === 'Matrix'}
					onclick={() => edit({ kind: 'RemoveChannel', reference: channel.reference })} />
			{/key}
		{/if}
	{:else if node.node_type === 'mapgyver.media.asset'}
		<AssetLocatorForm
			disabled={busy}
			submitLabel="Relink asset"
			submit={async (locator) => {
				await edit({ kind: 'RelinkAsset', asset: node.uuid, locator });
			}} />
	{/if}
</div>

<style>
	.properties {
		padding: 0.5rem;
		display: grid;
		gap: 0.5rem;
		min-width: 0;
	}
	.summary {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.85em;
	}
</style>
