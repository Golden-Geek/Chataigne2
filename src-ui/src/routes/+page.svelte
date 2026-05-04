<script lang="ts">
	import {
		MainWindow,
		registerNodeInspector,
		registerOutlinerRowSupplement,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { appIcons } from '$lib/assets/icons/node-icons.svelte';
	import ModuleCommandInspector from '$lib/inspectors/ModuleCommandInspector.svelte';
	import ModuleInspectorPanelHeader from '$lib/inspectors/ModuleInspectorPanelHeader.svelte';
	import ModuleNodeInspector from '$lib/inspectors/ModuleNodeInspector.svelte';
	import ModuleItem from '$lib/panels/modules/ModuleItem.svelte';
	import ModulePanel from '$lib/panels/modules/ModulePanel.svelte';

	registerNodeInspector('module_command', {
		component: ModuleCommandInspector
	});

	registerNodeInspector('module', {
		component: ModuleNodeInspector,
		panelHeaderComponent: ModuleInspectorPanelHeader
	});

	// registerOutlinerRowSupplement('module', {
	// 	component: ModuleItem
	// });

	const userPanels: UserPanelDefinitionMap = {
		modules: {
			title: 'Modules',
			component: ModulePanel,
			description: 'Filtered outliner view for modules'
		}
	};
</script>

<MainWindow {userPanels} nodeIcons={appIcons} />
