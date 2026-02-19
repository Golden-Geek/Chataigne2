<script lang="ts">
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';
	import Workbench from './Workbench.svelte';
	import { createWorkbenchSession } from '../store/workbench.svelte';

	const props = $props<{
		wsUrl?: string;
		httpBaseUrl?: string;
		pollIntervalMs?: number;
		bootstrapRetryMs?: number;
		children?: Snippet;
	}>();

	const session = createWorkbenchSession({
		wsUrl: props.wsUrl ?? 'ws://localhost:7010/api/ui/ws',
		httpBaseUrl: props.httpBaseUrl ?? 'http://localhost:7010/api/ui',
		pollIntervalMs: props.pollIntervalMs ?? 120,
		bootstrapRetryMs: props.bootstrapRetryMs ?? 1000
	});

	onMount(() => session.mount());
</script>

<Workbench {session}>
	{@render props.children?.()}
</Workbench>
