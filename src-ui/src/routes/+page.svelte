<script lang="ts">
	import { ConnectedWorkbench } from '$gc-ui';

	const wsUrl = import.meta.env.VITE_GC_UI_WS_URL ?? 'ws://127.0.0.1:7010/api/ui/ws';
	const httpBaseUrl = import.meta.env.VITE_GC_UI_API_BASE ?? 'http://127.0.0.1:7010/api/ui';
</script>

<ConnectedWorkbench {wsUrl} {httpBaseUrl} pollIntervalMs={120} />
