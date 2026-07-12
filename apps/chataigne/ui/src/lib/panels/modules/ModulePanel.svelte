<script lang="ts">
	import { ManagerListPanel, type PanelProps, type PanelState, type UiNodeDto } from 'golden_ui';
	import ModuleItem from './ModuleItem.svelte';

	let _props: PanelProps = $props();

	const MODULE_MANAGER_NODE_TYPE = 'module_manager';
	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'module_folder';

	const isModuleFolderNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		Boolean(candidate && candidate.node_type === MODULE_FOLDER_NODE_TYPE);

	const isModuleLeafNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		Boolean(
			candidate &&
			candidate.user_item_kind === MODULE_USER_ITEM_KIND &&
			candidate.node_type !== MODULE_FOLDER_NODE_TYPE
		);

	const isModuleTreeNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		isModuleLeafNode(candidate) || isModuleFolderNode(candidate);

	const canRenderModuleChildren = (candidate: UiNodeDto): boolean => !isModuleLeafNode(candidate);

	export const setPanelState = (_next: PanelState): void => {};
</script>

<ManagerListPanel
	managerNodeType={MODULE_MANAGER_NODE_TYPE}
	searchPlaceholder="Search modules..."
	missingMessage="No module manager found in the current graph."
	emptyMessage="No modules available."
	rootDropMessage="Drop here to move into Module Manager."
	addButtonTitle="Add item to Module Manager"
	rowSupplementComponent={ModuleItem}
	isTreeNode={isModuleTreeNode}
	canRenderNodeChildren={canRenderModuleChildren} />
