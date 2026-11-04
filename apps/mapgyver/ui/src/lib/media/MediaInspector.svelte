<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { MediaEdit } from './generated/MediaEdit';
	import { invokeMediaEdit } from './operations';
	import { ASSET_NODE_TYPE, MEDIA_NAMESPACE } from './generated/constants';
	import MatrixRuntimeInspector from '../matrix/MatrixRuntimeInspector.svelte';
	import AssetLocatorForm from './AssetLocatorForm.svelte';
	import reloadIcon from 'golden_ui/style/icons/reset.svg';
	import referenceIcon from 'golden_ui/style/icons/parameter/reference.svg';
	import SpoutSenderPicker from './SpoutSenderPicker.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import ChannelsSection from './ChannelsSection.svelte';
	import ReferenceChannelsSection from './ReferenceChannelsSection.svelte';
	import MediaScriptsSection from './MediaScriptsSection.svelte';
	import MediaCommandsSection from './MediaCommandsSection.svelte';
	import PictureChannelFile from './PictureChannelFile.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren,
		collapsed = false
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
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
			node.node_type !== 'mapgyver.media.channel'
		) {
			snapshot = null;
			owner = null;
			return;
		}
		if (current.hasActiveEditSession) return;
		const parentId = current.graph.state.parentById.get(node.node_id);
		const parent = parentId == null ? null : current.getNodeData(parentId);
		if (parent?.node_type !== 'mapgyver.media.v7') return;
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

{@render defaultHeader?.()}
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
	{#if node.node_type === 'mapgyver.media.v7'}
		{#key node.uuid}<ChannelsSection {node} {defaultChildren} />{/key}
	{:else if node.node_type === 'mapgyver.media.reference.v3' && !collapsed}
		{#key node.uuid}<ReferenceChannelsSection {node} />{/key}
	{/if}
	<div class="properties">
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
		{#if channel}
			{#if channel.definition.source.kind === 'Picture'}
				<PictureChannelFile {node} {channel} {busy} {edit} />
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
			{:else if channel.definition.source.kind === 'Matrix'}<MatrixRuntimeInspector />{/if}
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
	.summary {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.85em;
	}
</style>
