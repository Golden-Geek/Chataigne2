<script lang="ts">
	import NodeContextMenu from '../common/NodeContextMenu.svelte';
	import GlobalConfirmationDialog from '../common/GlobalConfirmationDialog.svelte';
	import NodePickerModalHost from '../common/NodePickerModalHost.svelte';
	import WindowExitConfirmDialog from './WindowExitConfirmDialog.svelte';
</script>

<NodePickerModalHost />
<NodeContextMenu />
<GlobalConfirmationDialog />
<WindowExitConfirmDialog />
