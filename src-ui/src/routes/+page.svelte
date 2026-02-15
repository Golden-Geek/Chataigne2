<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Workbench,
		createGraphStore,
		createHttpUiClient,
		type UiEditIntent,
		wholeGraphScope
	} from '$gc-ui';

	const graph = createGraphStore();
	const client = createHttpUiClient({
		baseUrl: import.meta.env.VITE_GC_UI_API_BASE ?? 'http://127.0.0.1:7010/api/ui',
		pollIntervalMs: 120
	});

	let status = $state('Connecting to engine...');

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

		status = ack.earliest_event_time
			? `Applied at ${ack.earliest_event_time.tick}:${ack.earliest_event_time.micro}:${ack.earliest_event_time.seq}`
			: 'Applied.';

		if ($graph.requiresResync) {
			const snapshot = await client.snapshot(wholeGraphScope);
			graph.loadSnapshot(snapshot);
			status = 'Snapshot resynced.';
		}
	};

	onMount(() => {
		let unsubscribe = () => {};

		void (async () => {
			try {
				const snapshot = await client.snapshot(wholeGraphScope);
				graph.loadSnapshot(snapshot);
				status = 'Connected.';

				unsubscribe = client.subscribe(wholeGraphScope, snapshot.at, (batch) => {
					graph.applyBatch(batch);
				});
			} catch (error) {
				const message = error instanceof Error ? error.message : 'unknown connection error';
				status = `Connection failed: ${message}`;
			}
		})();

		return () => {
			unsubscribe();
		};
	});
</script>

<Workbench
	state={$graph}
	{status}
	onSelect={(nodeId) => graph.selectNode(nodeId)}
	onIntent={(intent) => void handleIntent(intent)}
/>
