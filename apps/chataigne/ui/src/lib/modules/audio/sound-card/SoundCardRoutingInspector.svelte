<script lang="ts">
	import { AudioRoutingPatchBay, type AudioRoutingPatchBinding } from 'golden_audio_ui';
	import {
		type NodeId,
		type NodeInspectorChildFilter,
		type NodeInspectorComponentProps,
		type UiNodeDto
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { SOUND_CARD_TELEMETRY_TOPIC, type SoundCardUiTelemetryDto } from './generated';
	import { createSoundCardRoutingBinding } from './sound-card-routing-adapter.svelte';
	import {
		projectSoundCardRouting,
		soundCardAncestorByType,
		soundCardDirectionConfigured
	} from './sound-card-routing-model';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let nodes = $derived(session?.graph.state.nodesById ?? new Map<NodeId, UiNodeDto>());
	let parentById = $derived(session?.graph.state.parentById ?? new Map<NodeId, NodeId>());
	let liveNode = $derived(
		nodes.get(node.node_id) ??
			[...nodes.values()].find((candidate) => candidate.uuid === node.uuid) ??
			null
	);
	let module = $derived(
		liveNode ? soundCardAncestorByType(nodes, parentById, liveNode, 'sound_card_module') : null
	);
	let telemetry = $derived(
		module
			? (session?.getCustomEventPayload<SoundCardUiTelemetryDto>(
					SOUND_CARD_TELEMETRY_TOPIC,
					module.node_id
				) ?? null)
			: null
	);
	let projection = $derived(
		liveNode
			? projectSoundCardRouting(nodes, parentById, liveNode, telemetry?.device ?? null)
			: null
	);
	let binding = $derived<AudioRoutingPatchBinding | null>(
		projection ? createSoundCardRoutingBinding(projection) : null
	);
	let inputDirection = $derived(projection?.direction === 'input');
	let directionActive = $derived(
		projection?.direction === 'input'
			? soundCardDirectionConfigured(nodes, module, 'input')
			: projection?.direction === 'output'
				? soundCardDirectionConfigured(nodes, module, 'output')
				: false
	);
	let sourceLabel = $derived(inputDirection ? 'Device Inputs' : 'Output Channels');
	let destinationLabel = $derived(inputDirection ? 'Input Channels' : 'Device Outputs');

	const renderManagedChild: NodeInspectorChildFilter = (child) =>
		(child.decl_id.split('/').at(-1) ?? child.decl_id) !== 'routes' &&
		child.meta.short_name !== 'routes';
</script>

{#if directionActive}
	{@render defaultHeader?.()}

	{#snippet routingContent()}
		{@render defaultChildren?.('', renderManagedChild)}
		{#if projection && binding}
			<AudioRoutingPatchBay
				sources={projection.sources}
				destinations={projection.destinations}
				connections={projection.connections}
				{binding}
				{sourceLabel}
				{destinationLabel}
				emptyLabel="No channels are connected." />
		{:else}
			<p class="unavailable">Routing is not available in the current module snapshot.</p>
		{/if}
	{/snippet}

	{@render defaultContent?.(routingContent, 'sound-card-routing-inspector')}
{/if}

<style>
	.unavailable {
		margin: 0.55rem;
		padding: 0.55rem;
		border-radius: 0.3rem;
		background: color-mix(in srgb, var(--gc-color-bg-lighter) 65%, transparent);
		color: var(--gc-color-text-muted);
		font-size: 0.76rem;
	}

	:global(.sound-card-routing-inspector) {
		min-inline-size: 0;
	}
</style>
