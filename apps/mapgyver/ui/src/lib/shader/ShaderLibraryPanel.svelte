<script lang="ts">
	import { FileExplorerPanel, type PanelProps, type PanelState } from 'golden_ui';
	import { SHADER_LIBRARY_SUFFIXES } from '../media/generated/constants';

	let props: PanelProps = $props();
	let explorer = $state<{ setPanelState(state: PanelState): void }>();
	export const setPanelState = (state: PanelState) => explorer?.setPanelState(state);
</script>

<FileExplorerPanel {...props} fileSuffixes={SHADER_LIBRARY_SUFFIXES} bind:this={explorer} />
