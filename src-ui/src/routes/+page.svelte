<script lang="ts">
	import { MainWindow, WorkbenchOverlayHost, type UserPanelDefinitionMap } from 'golden_ui';
	import { appIcons } from '$lib/assets/icons/node-icons.svelte';

	const userPanels: UserPanelDefinitionMap = {};
</script>

<MainWindow {userPanels} nodeIcons={appIcons} />
<WorkbenchOverlayHost />
