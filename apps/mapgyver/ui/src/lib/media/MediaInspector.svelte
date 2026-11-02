<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import type { MediaManagerSnapshot } from './generated/MediaManagerSnapshot';
	import type { MediaEdit } from './generated/MediaEdit';
	import { invokeMediaEdit } from './operations';
	import { MEDIA_NAMESPACE } from './generated/constants';
	import ChannelPreview from './ChannelPreview.svelte';
	import CompositionProperties from '../composition/CompositionProperties.svelte';
	import MatrixProperties from '../matrix/MatrixProperties.svelte';
	import AssetLocatorForm from './AssetLocatorForm.svelte';
	import reloadIcon from 'golden_ui/style/icons/reset.svg';
	import referenceIcon from 'golden_ui/style/icons/parameter/reference.svg';
	import SpoutSenderPicker from './SpoutSenderPicker.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const selection = useDocumentSelection();
	const parametersOnly = (child: UiNodeDto) => child.decl_id === 'parameters';
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
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
	<div class="properties">
		{#if error}<p role="alert">{error}</p>{/if}
		{#if node.node_type === 'mapgyver.media.manager'}
			<AssetLocatorForm
				disabled={busy}
				submitLabel="Import Picture"
				submit={(locator) => edit({ kind: 'ImportPicture', locator, target: null, label: null })} />
		{/if}
		{#if channel && media}
			<ChannelPreview {channel} />
			{#if channel.definition.source.kind === 'Picture'}
				<AssetLocatorForm
					disabled={busy}
					submitLabel="Import and assign"
					submit={(locator) =>
						edit({ kind: 'ImportPicture', locator, target: channel.reference, label: null })} />
				{#if channel.definition.source.asset}
					{@const asset = channel.definition.source.asset}
					<div class="summary">
						<IconButton
							icon={reloadIcon}
							label="Reload Picture"
							disabled={busy}
							onclick={() => edit({ kind: 'ReloadAsset', asset })} />
						<IconButton
							icon={referenceIcon}
							label="Inspect Picture asset"
							disabled={busy}
							onclick={() => selection.select(asset)} />
					</div>
				{/if}
			{:else if channel.definition.source.kind === 'Spout'}
				<SpoutSenderPicker
					sender={channel.definition.source.sender ?? ''}
					disabled={busy}
					onSelect={async (sender) => {
						const folder = node.children
							.map((id) => session?.getNodeData(id))
							.find((child) => child?.decl_id === 'parameters');
						const parameter = folder?.children
							.map((id) => session?.getNodeData(id))
							.find((child) => child?.decl_id === 'parameters/sender');
						if (parameter)
							await session?.sendIntent({
								kind: 'setParam',
								node: parameter.node_id,
								value: { kind: 'str', value: sender },
								behaviour: 'Append'
							});
					}} />
			{:else if channel.definition.source.kind === 'Composition'}<CompositionProperties {node} />
			{:else if channel.definition.source.kind === 'Matrix'}<MatrixProperties />{/if}
		{/if}
		{#if node.node_type === 'mapgyver.media.asset'}
			<IconButton
				icon={reloadIcon}
				label="Reload asset"
				disabled={busy}
				onclick={() => edit({ kind: 'ReloadAsset', asset: node.uuid })} />
		{/if}
	</div>
{/snippet}
{@render defaultContent?.(content)}

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
