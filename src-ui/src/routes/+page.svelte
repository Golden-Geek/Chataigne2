<script lang="ts">
	import { MainWindow, registerNodeInspector, type UserPanelDefinitionMap } from 'golden_ui';
	import { appIcons } from '$lib/assets/icons/node-icons.svelte';
	import ModuleCommandInspector from '$lib/inspectors/ModuleCommandInspector.svelte';
	import ModulePanel from '$lib/panels/modules/ModulePanel.svelte';

	registerNodeInspector('module_command', {
		component: ModuleCommandInspector
	});

	const userPanels: UserPanelDefinitionMap = {
		modules: {
			title: 'Modules',
			component: ModulePanel,
			description: 'Filtered outliner view for modules'
		}
	};
</script>

<MainWindow {userPanels} nodeIcons={appIcons} />
