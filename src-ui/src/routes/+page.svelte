<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Workbench,
		createGraphStore,
		createWebSocketUiClient,
		type UiEditIntent,
		wholeGraphScope
	} from '$gc-ui';

	const graph = createGraphStore();
	const client = createWebSocketUiClient({
		wsUrl: import.meta.env.VITE_GC_UI_WS_URL ?? 'ws://127.0.0.1:7010/api/ui/ws',
		httpBaseUrl: import.meta.env.VITE_GC_UI_API_BASE ?? 'http://127.0.0.1:7010/api/ui',
		pollIntervalMs: 120,
		onConnectionStateChange: (state) => {
			if (state === 'connecting') {
				status = 'Connecting to engine...';
			} else if (state === 'connected') {
				status = 'Connected.';
			} else if (state === 'disconnected') {
				status = 'Disconnected from engine.';
			} else if (state === 'reconnecting') {
				status = 'Disconnected, reconnecting...';
			} else if (state === 'fallbackPolling') {
				status = 'WebSocket lost. Using HTTP fallback while reconnecting...';
			}
		}
	});

	let status = $state('Connecting to engine...');
	let resyncInFlight = false;

	const handleIntent = async (intent: UiEditIntent): Promise<void> => {
		const ack = await client.sendIntent(intent);
		if (!ack.success) {
			status = `Error: ${ack.error_message ?? ack.error_code ?? 'unknown error'}`;
			return;
		}

		if (ack.status === 'staged') {
			status = 'Intent accepted and staged.';
			return;
		}

		if (intent.kind !== 'setParam') {
			status = ack.earliest_event_time
				? `Applied at ${ack.earliest_event_time.tick}:${ack.earliest_event_time.micro}:${ack.earliest_event_time.seq}`
				: 'Applied.';
		}

		if ($graph.requiresResync) {
			const snapshot = await client.snapshot(wholeGraphScope);
			graph.loadSnapshot(snapshot);
			status = 'Snapshot resynced.';
		}
	};

	onMount(() => {
		let unsubscribe = () => {};
		let stopped = false;
		let subscribed = false;
		let bootstrapInFlight = false;
		let bootstrapRetryTimer: ReturnType<typeof setTimeout> | null = null;

		const clearBootstrapRetry = (): void => {
			if (bootstrapRetryTimer !== null) {
				clearTimeout(bootstrapRetryTimer);
				bootstrapRetryTimer = null;
			}
		};

		const scheduleBootstrapRetry = (delayMs: number): void => {
			if (stopped || bootstrapRetryTimer !== null || subscribed) {
				return;
			}
			bootstrapRetryTimer = setTimeout(() => {
				bootstrapRetryTimer = null;
				void bootstrap();
			}, delayMs);
		};

		const onBatch = (batch: Parameters<typeof graph.applyBatch>[0]): void => {
			graph.applyBatch(batch);
			if ($graph.requiresResync && !resyncInFlight) {
				resyncInFlight = true;
				void (async () => {
					try {
						const latest = await client.snapshot(wholeGraphScope);
						graph.loadSnapshot(latest);
						status = 'Snapshot resynced.';
					} catch (error) {
						const message = error instanceof Error ? error.message : 'unknown resync error';
						status = `Resync failed: ${message}`;
					} finally {
						resyncInFlight = false;
					}
				})();
			}
		};

		const bootstrap = async (): Promise<void> => {
			if (stopped || subscribed || bootstrapInFlight) {
				return;
			}

			bootstrapInFlight = true;
			try {
				const snapshot = await client.snapshot(wholeGraphScope);
				if (stopped) {
					return;
				}
				graph.loadSnapshot(snapshot);
				status = 'Connected.';
				unsubscribe = client.subscribe(wholeGraphScope, snapshot.at, onBatch);
				subscribed = true;
				clearBootstrapRetry();
			} catch (error) {
				if (stopped) {
					return;
				}
				const message = error instanceof Error ? error.message : 'unknown connection error';
				status = `Connection failed: ${message} (retrying...)`;
				scheduleBootstrapRetry(1000);
			} finally {
				bootstrapInFlight = false;
			}
		};

		void bootstrap();

		return () => {
			stopped = true;
			clearBootstrapRetry();
			unsubscribe();
		};
	});
</script>

<Workbench
	state={$graph}
	{client}
	{status}
	onSelect={(nodeId) => graph.selectNode(nodeId)}
	onIntent={(intent) => void handleIntent(intent)}
/>
