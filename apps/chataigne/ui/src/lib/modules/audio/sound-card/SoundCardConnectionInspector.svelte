<script lang="ts">
	import type { NodeId, NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { SOUND_CARD_TELEMETRY_TOPIC, type SoundCardUiTelemetryDto } from './generated';
	import { soundCardAncestorByType, soundCardChildByKey } from './sound-card-routing-model';
	import { soundCardConnectionHint } from './sound-card-connection-status';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let nodes = $derived(session?.graph.state.nodesById ?? new Map<NodeId, UiNodeDto>());
	let parentById = $derived(session?.graph.state.parentById ?? new Map<NodeId, NodeId>());
	let liveNode = $derived(nodes.get(node.node_id) ?? node);
	let module = $derived(soundCardAncestorByType(nodes, parentById, liveNode, 'sound_card_module'));
	let connectedNode = $derived(soundCardChildByKey(nodes, liveNode, 'connected'));
	let connected = $derived.by((): boolean | null => {
		if (
			connectedNode?.data.kind !== 'parameter' ||
			connectedNode.data.param.value.kind !== 'bool'
		) {
			return null;
		}
		return connectedNode.data.param.value.value;
	});
	let telemetry = $derived(
		module
			? (session?.getCustomEventPayload<SoundCardUiTelemetryDto>(
					SOUND_CARD_TELEMETRY_TOPIC,
					module.node_id
				) ?? null)
			: null
	);
	let hint = $derived(soundCardConnectionHint(connected, telemetry?.device ?? null));
</script>

{@render defaultHeader?.()}

{#snippet connectionContent()}
	{@render defaultChildren?.()}
	<p class="connection-configuration-hint {hint.tone}" role="status">
		<span class="connection-configuration-dot" aria-hidden="true"></span>
		<span>{hint.message}</span>
	</p>
{/snippet}

{@render defaultContent?.(connectionContent, 'sound-card-connection-inspector')}

<style>
	.connection-configuration-hint {
		display: flex;
		align-items: flex-start;
		gap: 0.38rem;
		margin: 0.5rem 0.35rem 0.1rem;
		padding: 0.42rem 0.5rem;
		border: 0.0625rem solid currentColor;
		border-radius: 0.35rem;
		background: rgb(from currentColor r g b / 8%);
		color: var(--gc-color-text-muted);
		font-size: 0.68rem;
		line-height: 1.35;
	}

	.connection-configuration-hint.success {
		color: var(--gc-color-success);
	}

	.connection-configuration-hint.error {
		color: var(--gc-color-error);
	}

	.connection-configuration-hint.pending {
		color: var(--gc-color-warning);
	}

	.connection-configuration-dot {
		flex: 0 0 auto;
		inline-size: 0.42rem;
		block-size: 0.42rem;
		margin-block-start: 0.24rem;
		border-radius: 50%;
		background: currentColor;
		box-shadow: 0 0 0 0.12rem rgb(from currentColor r g b / 15%);
	}

	:global(.sound-card-connection-inspector) {
		min-inline-size: 0;
	}
</style>
