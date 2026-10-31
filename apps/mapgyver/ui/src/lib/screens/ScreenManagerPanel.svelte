<script lang="ts">
	import type { PanelProps, PanelState } from 'golden_ui';
	import SurfaceWorkbench from './SurfaceWorkbench.svelte';
	let props: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
</script>

<SurfaceWorkbench {...props} {panel} editor="screen" />
