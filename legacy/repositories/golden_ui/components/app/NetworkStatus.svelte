<script lang="ts">
	import { onMount } from 'svelte';

	type ConnectionInfo = {
		openAccess: boolean;
		listeningAddress: string;
		advertisedName: string | null;
		websocketPath: string;
		metrics?: TransportMetrics;
	};

	type TransportMetrics = {
		activeConnections: number;
		activeWebsockets: number;
		droppedOutboundMessages: number;
		resyncRequests: number;
		protocolErrors: number;
	};

	let info = $state<ConnectionInfo | null>(null);
	let metrics = $state<TransportMetrics | null>(null);
	let copied = $state(false);
	const { httpBaseUrl } = $props<{ httpBaseUrl: string }>();
	const endpoint = (path: string): string => `${httpBaseUrl.replace(/\/+$/, '')}/${path}`;

	let isOpenLan = $derived.by(() => {
		if (!info?.openAccess) return false;
		const address = info.listeningAddress;
		const host = (
			address.startsWith('[')
				? address.slice(1, address.indexOf(']'))
				: address.slice(0, address.lastIndexOf(':'))
		).toLowerCase();
		return host !== 'localhost' && host !== '127.0.0.1' && host !== '::1';
	});

	const refresh = async (): Promise<void> => {
		try {
			const response = await fetch(endpoint('connection-info'));
			if (!response.ok) return;
			const nextInfo = (await response.json()) as ConnectionInfo;
			info = nextInfo;
			metrics = nextInfo.metrics ?? null;
		} catch {
			// The normal connection badge already owns disconnected-state presentation.
		}
	};

	const copyConnectionInfo = async (): Promise<void> => {
		if (!info) return;
		const scheme = window.location.protocol === 'https:' ? 'https' : 'http';
		const text = `${scheme}://${info.listeningAddress}`;
		await navigator.clipboard.writeText(text);
		copied = true;
		window.setTimeout(() => (copied = false), 1500);
	};

	onMount(() => {
		void refresh();
		const interval = window.setInterval(() => void refresh(), 2000);
		return () => window.clearInterval(interval);
	});
</script>

{#if isOpenLan && info}
	<details class="network-status" data-no-drag>
		<summary title="Any client that can reach this address may control the application">
			<span class="network-dot" aria-hidden="true"></span>
			Open network control
		</summary>
		<div class="network-popover">
			<strong>{info.advertisedName || 'Golden Runtime'}</strong>
			<span>{info.listeningAddress}</span>
			<span>{metrics?.activeWebsockets ?? 0} WebSocket client(s)</span>
			<span
				>{metrics?.droppedOutboundMessages ?? 0} dropped · {metrics?.resyncRequests ?? 0} resync</span>
			<span>{metrics?.protocolErrors ?? 0} recent protocol error(s)</span>
			<button type="button" onclick={() => void copyConnectionInfo()}>
				{copied ? 'Copied' : 'Copy connection info'}
			</button>
		</div>
	</details>
{/if}

<style>
	.network-status {
		position: relative;
		font-size: 0.72rem;
		color: var(--gc-color-warning);
	}

	summary {
		display: flex;
		align-items: center;
		gap: 0.4em;
		padding: 0.35em 0.65em;
		border-radius: 0.45em;
		background: rgb(from var(--gc-color-warning) r g b / 12%);
		cursor: pointer;
		list-style: none;
		white-space: nowrap;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.network-dot {
		width: 0.55em;
		height: 0.55em;
		border-radius: 50%;
		background: currentColor;
	}

	.network-popover {
		position: absolute;
		top: calc(100% + 0.4rem);
		right: 0;
		z-index: 20;
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-width: 17rem;
		padding: 0.8rem;
		color: var(--gc-color-text, #eee);
		background: var(--gc-color-surface-2, #1e1e1e);
		border: 0.08rem solid var(--gc-color-border, #444);
		border-radius: 0.5rem;
		box-shadow: 0 0.5rem 1.5rem rgb(0 0 0 / 28%);
	}

	button {
		align-self: flex-start;
		padding: 0.35em 0.65em;
		border: 0.08rem solid var(--gc-color-border, #444);
		border-radius: 0.4em;
		color: inherit;
		background: var(--gc-color-surface-1, #141414);
		cursor: pointer;
	}
</style>
