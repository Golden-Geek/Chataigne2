<script lang="ts">
	import { showPanel } from '../../golden_ui/store/ui-panels';
	import { sendSetParamIntent } from '../../golden_ui/store/ui-intents';
	import { appState } from '../../golden_ui/store/workbench.svelte';
	import type { GraphState } from '../../golden_ui/store/graph.svelte';
	import type { NodeId, UiNodeDto } from '../../golden_ui/types';
	import connectedIcon from '../../assets/icons/module/connected.svg';
	import disconnectedIcon from '../../assets/icons/module/disconnected.svg';
	import incomingIcon from '../../assets/icons/module/incoming.svg';
	import outgoingIcon from '../../assets/icons/module/outgoing.svg';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	const TRAFFIC_FLASH_MS = 340;
	const MODULE_INCOMING_TRAFFIC_EVENT_TOPIC = 'chataigne.module.traffic.incoming';
	const MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC = 'chataigne.module.traffic.outgoing';
	const CONNECTION_DECL_IDS = ['connection', 'infos'] as const;
	const CONNECTED_PARAM_DECL_ID = 'connected';
	const CAN_RECEIVE_PARAM_DECL_ID = 'can_receive';
	const CAN_SEND_PARAM_DECL_ID = 'can_send';
	const LOG_INCOMING_PARAM_DECL_ID = 'log_incoming';
	const LOG_OUTGOING_PARAM_DECL_ID = 'log_outgoing';
	const CONNECTED_CLIENTS_PARAM_DECL_ID = 'connected_clients';

	const lastPathSegment = (value: string): string => value.split('/').pop() ?? value;

	const nodeMatchesPathSegment = (candidate: UiNodeDto, segment: string): boolean =>
		candidate.decl_id === segment ||
		lastPathSegment(candidate.decl_id) === segment ||
		candidate.meta.short_name === segment ||
		candidate.meta.label === segment;

	const findDescendantByDeclPath = (
		graph: GraphState | null,
		rootNodeId: NodeId,
		path: readonly string[]
	): UiNodeDto | null => {
		if (!graph) {
			return null;
		}

		let currentNodeId: NodeId = rootNodeId;
		for (const segment of path) {
			const childIds = graph.childrenById.get(currentNodeId) ?? [];
			let nextNodeId: NodeId | null = null;
			for (const childId of childIds) {
				const childNode = graph.nodesById.get(childId);
				if (childNode && nodeMatchesPathSegment(childNode, segment)) {
					nextNodeId = childId;
					break;
				}
			}
			if (nextNodeId === null) {
				return null;
			}
			currentNodeId = nextNodeId;
		}

		return graph.nodesById.get(currentNodeId) ?? null;
	};

	const findConnectionParameter = (
		graph: GraphState | null,
		rootNodeId: NodeId,
		paramDeclId: string
	): UiNodeDto | null => {
		for (const connectionDeclId of CONNECTION_DECL_IDS) {
			const candidate = findDescendantByDeclPath(graph, rootNodeId, [
				connectionDeclId,
				paramDeclId
			]);
			if (candidate !== null) {
				return candidate;
			}
		}
		return null;
	};

	const readBoolParamValue = (candidate: UiNodeDto | null): boolean | null => {
		if (candidate?.data.kind !== 'parameter') {
			return null;
		}
		const { value } = candidate.data.param;
		return value.kind === 'bool' ? value.value : null;
	};

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(graph?.nodesById.get(node.node_id) ?? node);
	let connectedParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, CONNECTED_PARAM_DECL_ID)
	);
	let canReceiveParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, CAN_RECEIVE_PARAM_DECL_ID)
	);
	let canSendParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, CAN_SEND_PARAM_DECL_ID)
	);
	let logIncomingParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, LOG_INCOMING_PARAM_DECL_ID)
	);
	let logOutgoingParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, LOG_OUTGOING_PARAM_DECL_ID)
	);
	let connectionState = $derived(readBoolParamValue(connectedParamNode));
	let canReceive = $derived(readBoolParamValue(canReceiveParamNode) === true);
	let canSend = $derived(readBoolParamValue(canSendParamNode) === true);
	let logIncomingEnabled = $derived(readBoolParamValue(logIncomingParamNode) === true);
	let logOutgoingEnabled = $derived(readBoolParamValue(logOutgoingParamNode) === true);
	let incomingTrafficSequence = $derived(
		session?.getCustomEventSequence(MODULE_INCOMING_TRAFFIC_EVENT_TOPIC, liveNode.node_id) ?? 0
	);
	let outgoingTrafficSequence = $derived(
		session?.getCustomEventSequence(MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC, liveNode.node_id) ?? 0
	);
	let connectionLabel = $derived.by(() => {
		if (connectionState === true) {
			return 'Connected';
		}
		if (connectionState === false) {
			return 'Disconnected';
		}
		return 'Status';
	});
	let connectionClassName = $derived.by(() => {
		if (connectionState === true) {
			return 'connected';
		}
		if (connectionState === false) {
			return 'disconnected';
		}
		return 'unknown';
	});
	let connectionIcon = $derived(connectionState === true ? connectedIcon : disconnectedIcon);
	let connectionFeedbackLabel = $derived(
		connectedParamNode
			? `${connectionLabel}. Inspect ${connectedParamNode.meta.label}`
			: 'Connection status unavailable'
	);
	let incomingLoggingLabel = $derived(
		logIncomingEnabled ? 'Disable incoming traffic logging' : 'Enable incoming traffic logging'
	);
	let outgoingLoggingLabel = $derived(
		logOutgoingEnabled ? 'Disable outgoing traffic logging' : 'Enable outgoing traffic logging'
	);

	let connectedClientsParamNode = $derived(
		findConnectionParameter(graph, liveNode.node_id, CONNECTED_CLIENTS_PARAM_DECL_ID)
	);

	let connectedClientsCount = $derived.by(() => {
		if (connectedClientsParamNode?.data.kind !== 'parameter') {
			return null;
		}
		const { value } = connectedClientsParamNode.data.param;
		return value.kind === 'int' ? value.value : null;
	});

	let incomingFlashActive = $state(false);
	let incomingFlashKey = $state(0);
	let outgoingFlashActive = $state(false);
	let outgoingFlashKey = $state(0);
	let previousIncomingTrafficSequence: number | null = null;
	let previousOutgoingTrafficSequence: number | null = null;

	const revealConnectedParameter = (): void => {
		if (!connectedParamNode) {
			return;
		}

		session?.selectNode(connectedParamNode.node_id, 'REPLACE');
		showPanel({
			panelType: 'inspector',
			panelId: 'inspector'
		});
	};

	const toggleBoolParameter = async (paramNode: UiNodeDto | null): Promise<void> => {
		if (paramNode?.data.kind !== 'parameter') {
			return;
		}
		const { param } = paramNode.data;
		if (param.read_only || param.value.kind !== 'bool') {
			return;
		}
		await sendSetParamIntent(
			paramNode.node_id,
			{ kind: 'bool', value: !param.value.value },
			param.event_behaviour
		);
	};

	const toggleIncomingLogging = (event: MouseEvent): void => {
		event.stopPropagation();
		void toggleBoolParameter(logIncomingParamNode);
	};

	const toggleOutgoingLogging = (event: MouseEvent): void => {
		event.stopPropagation();
		void toggleBoolParameter(logOutgoingParamNode);
	};

	$effect(() => {
		const sequence = incomingTrafficSequence;
		if (previousIncomingTrafficSequence === null) {
			previousIncomingTrafficSequence = sequence;
			return;
		}
		if (sequence === previousIncomingTrafficSequence) {
			return;
		}
		previousIncomingTrafficSequence = sequence;
		if (sequence <= 0) {
			return;
		}

		incomingFlashKey += 1;
		incomingFlashActive = true;
		const timer = setTimeout(() => {
			incomingFlashActive = false;
		}, TRAFFIC_FLASH_MS);
		return () => {
			clearTimeout(timer);
		};
	});

	$effect(() => {
		const sequence = outgoingTrafficSequence;
		if (previousOutgoingTrafficSequence === null) {
			previousOutgoingTrafficSequence = sequence;
			return;
		}
		if (sequence === previousOutgoingTrafficSequence) {
			return;
		}
		previousOutgoingTrafficSequence = sequence;
		if (sequence <= 0) {
			return;
		}

		outgoingFlashKey += 1;
		outgoingFlashActive = true;
		const timer = setTimeout(() => {
			outgoingFlashActive = false;
		}, TRAFFIC_FLASH_MS);
		return () => {
			clearTimeout(timer);
		};
	});
</script>

<div class="module-indicators">
	{#if connectedClientsCount != null}
		<span class="module-connected-clients" class:has-clients={connectedClientsCount > 0}>
			{`${connectedClientsCount} client${connectedClientsCount !== 1 ? 's' : ''}`}
		</span>
	{/if}
	{#if connectedParamNode}
		<button
			type="button"
			class={`module-status-icon module-connection-icon ${connectionClassName}`}
			title={connectionFeedbackLabel}
			aria-label={connectionFeedbackLabel}
			onclick={(event) => {
				event.stopPropagation();
				revealConnectedParameter();
			}}>
			<img src={connectionIcon} alt="" aria-hidden="true" />
		</button>
	{:else}
		<span
			class={`module-status-icon module-connection-icon ${connectionClassName}`}
			title={connectionFeedbackLabel}
			aria-label={connectionFeedbackLabel}
			role="img">
			<img src={connectionIcon} alt="" aria-hidden="true" />
		</span>
	{/if}

	{#if canReceive && logIncomingParamNode}
		<button
			type="button"
			class="module-status-icon module-traffic-icon traffic-incoming"
			class:logging={logIncomingEnabled}
			class:flashing={incomingFlashActive}
			title={incomingLoggingLabel}
			aria-label={incomingLoggingLabel}
			aria-pressed={logIncomingEnabled}
			onclick={toggleIncomingLogging}>
			{#key incomingFlashKey}
				<img
					class="module-traffic-image"
					class:flashing={incomingFlashActive}
					src={incomingIcon}
					alt=""
					aria-hidden="true" />
			{/key}
		</button>
	{:else}
		<span class="traffic-placeholder"> </span>
	{/if}

	{#if canSend && logOutgoingParamNode}
		<button
			type="button"
			class="module-status-icon module-traffic-icon traffic-outgoing"
			class:logging={logOutgoingEnabled}
			class:flashing={outgoingFlashActive}
			title={outgoingLoggingLabel}
			aria-label={outgoingLoggingLabel}
			aria-pressed={logOutgoingEnabled}
			onclick={toggleOutgoingLogging}>
			{#key outgoingFlashKey}
				<img
					class="module-traffic-image"
					class:flashing={outgoingFlashActive}
					src={outgoingIcon}
					alt=""
					aria-hidden="true" />
			{/key}
		</button>
	{:else}
		<span class="traffic-placeholder"> </span>
	{/if}
</div>

<style>
	.module-indicators {
		display: inline-flex;
		justify-content: end;
		align-items: center;
		gap: 0.15rem;
		flex: 1 0 auto;
	}

	.module-connected-clients {
		font-size: 0.75rem;
		vertical-align: middle;
		margin-right: 0.25rem;
		color: rgb(from var(--gc-color-text) r g b / 0.3);
	}

	.module-connected-clients.has-clients {
		color: rgb(from var(--gc-color-text) r g b / 0.6);
	}

	.module-status-icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.2rem;
		height: 1.2rem;
		background: transparent;
		cursor: pointer;
		border-radius: 0.8rem;
		transition:
			background-color 0.12s ease,
			border-color 0.12s ease,
			box-shadow 0.12s ease,
			opacity 0.12s ease,
			transform 0.12s ease;
	}

	.module-status-icon:hover {
		background: rgb(from var(--gc-color-text) r g b / 0.08);
	}

	.module-status-icon img {
		width: 100%;
		height: 100%;
		object-fit: contain;
		pointer-events: none;
	}

	.module-connection-icon {
		cursor: normal;
		pointer-events: none;
		border: solid 1px;
		padding: 0.15rem;
	}

	.module-connection-icon.connected {
		border-color: rgb(from var(--gc-color-success) r g b / 0.1);
		background: rgb(from var(--gc-color-success) r g b / 0.12);
	}

	.module-connection-icon.disconnected {
		border-color: rgb(from var(--gc-color-error) r g b / 0.2);
		background: rgb(from var(--gc-color-error) r g b / 0.12);
	}

	.module-connection-icon.unknown {
		background: rgb(from var(--gc-color-background) r g b / 0.2);
	}

	.traffic-placeholder {
		width:1.2rem;
		height:1.2rem;
		margin:0;
	}

	.module-traffic-icon {
		padding: 0rem;
		border: solid 1px transparent;
		transition: border-color 0.12s ease;
	}
	

	.module-traffic-icon.logging.traffic-incoming {
		border-color: rgba(0, 150, 255, 1);
	}

	.module-traffic-icon.logging.traffic-outgoing {
		border-color: rgba(255, 192, 50, 1);
	}

	.module-traffic-image {
		filter: grayscale(1) brightness(0.6);
	}

	.module-traffic-image.flashing {
		animation: module-traffic-flash 0.1s ease-out;
	}

	@keyframes module-traffic-flash {
		0% {
			filter: brightness(1) grayscale(0);
		}
		100% {
			filter: brightness(0.8) grayscale(1);
		}
	}
</style>
