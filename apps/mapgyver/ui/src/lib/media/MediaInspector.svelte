<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { MediaEdit } from './generated/MediaEdit';
	import { invokeMediaEdit } from './operations';
	import { ASSET_NODE_TYPE, MEDIA_NAMESPACE } from './generated/constants';
	import AssetLocatorForm from './AssetLocatorForm.svelte';
	import reloadIcon from 'golden_ui/style/icons/reset.svg';
	import stepPreviousIcon from '../icons/step-previous.svg';
	import stepNextIcon from '../icons/step-next.svg';
	import { useDocumentSelection } from '../selection.svelte';
	import ChannelsSection from './ChannelsSection.svelte';
	import ReferenceChannelsSection from './ReferenceChannelsSection.svelte';
	import MediaScriptsSection from './MediaScriptsSection.svelte';
	import MediaCommandsSection from './MediaCommandsSection.svelte';
	import VideoSourceStatus from './VideoSourceStatus.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren,
		collapsed = false
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet<[Snippet?]>;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
		collapsed?: boolean;
	} = $props();
	const selection = useDocumentSelection();
	const parametersOnly = (child: UiNodeDto) =>
		child.decl_id === 'parameters' || child.node_type === 'mapgyver.composition.placement.v2';
	const session = $derived(appState.session);
	let snapshot = $state<ChannelSnapshot | null>(null),
		pending = $state(false),
		error = $state('');
	let owner = $state.raw<{ session: typeof session; generation: number } | null>(null);
	let read = 0,
		disposed = false;
	const channel = $derived(
		session?.status === 'connected' &&
			!session.loading.visible &&
			owner?.session === session &&
			owner.generation === session.snapshotGeneration &&
			snapshot?.reference.channel === node.uuid &&
			!collapsed
			? snapshot
			: null
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
		if (
			!current ||
			current.status !== 'connected' ||
			current.loading.visible ||
			collapsed ||
			node.node_type !== 'mapgyver.media.channel.v2'
		) {
			snapshot = null;
			owner = null;
			return;
		}
		if (current.hasActiveEditSession) return;
		const parentId = current.graph.state.parentById.get(node.node_id);
		const parent = parentId == null ? null : current.getNodeData(parentId);
		if (parent?.node_type !== 'mapgyver.media.v11') return;
		const reference = { media: parent.uuid, channel: node.uuid };
		const generation = current.snapshotGeneration;
		try {
			const result = await invokeMediaEdit(current, { kind: 'InspectChannel', reference });
			if (
				disposed ||
				id !== read ||
				session !== current ||
				current.snapshotGeneration !== generation ||
				node.uuid !== reference.channel ||
				current.loading.visible
			)
				return;
			if (result.kind === 'Channel') {
				snapshot = result.snapshot;
				owner = result.snapshot ? { session: current, generation } : null;
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
		void node.uuid;
		void collapsed;
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

{#snippet channelActions()}
	{#if channel?.definition.source.kind === 'Video' && channel.definition.source.file}
		{@const reference = channel.reference}
		<IconButton
			icon={stepPreviousIcon}
			label="Previous Video frame"
			disabled={busy}
			onclick={() =>
				edit({
					kind: 'Video',
					media: reference.media,
					edit: { kind: 'Step', channel: reference.channel, direction: 'Previous' }
				})} />
		<IconButton
			icon={stepNextIcon}
			label="Next Video frame"
			disabled={busy}
			onclick={() =>
				edit({
					kind: 'Video',
					media: reference.media,
					edit: { kind: 'Step', channel: reference.channel, direction: 'Next' }
				})} />
	{/if}
	{#if channel?.definition.source.kind === 'Picture' && channel.definition.source.file}
		{@const reference = channel.reference}
		<IconButton
			icon={reloadIcon}
			label="Reload Picture"
			disabled={busy}
			onclick={() => edit({ kind: 'ReloadPicture', reference })} />
	{/if}
{/snippet}
{@render defaultHeader?.(channelActions)}
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
	{#if node.node_type === 'mapgyver.media.v11'}
		{#key node.uuid}<ChannelsSection {node} {defaultChildren} />{/key}
	{:else if node.node_type === 'mapgyver.media.reference.v4' && !collapsed}
		{#key node.uuid}<ReferenceChannelsSection {node} />{/key}
	{/if}
	<div class="properties" class:channel-properties={node.node_type === 'mapgyver.media.channel.v2'}>
		{#if channel?.definition.source.kind === 'Video'}
			<VideoSourceStatus reference={channel.reference} file={channel.definition.source.file} />
		{/if}
		{#if error}<p role="alert">{error}</p>{/if}
		{#if node.node_type === 'mapgyver.media.manager'}
			<MediaCommandsSection {node} {defaultChildren} disabled={busy} />
			<AssetLocatorForm
				disabled={busy}
				submitLabel="Import Picture"
				submit={(locator) => edit({ kind: 'ImportPicture', locator, target: null, label: null })} />
			<MediaScriptsSection
				{node}
				{defaultChildren}
				disabled={busy}
				oncreate={async () => {
					const select = selection.capture();
					const result = await edit({ kind: 'CreateScript', label: null });
					if (result?.kind === 'ScriptCreated') select(result.script);
				}} />
		{/if}
		{#if node.node_type === ASSET_NODE_TYPE}
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
	.properties:empty {
		display: none;
	}
	.channel-properties {
		padding: 0.15rem 0.3rem;
		gap: 0.25rem;
	}
</style>
