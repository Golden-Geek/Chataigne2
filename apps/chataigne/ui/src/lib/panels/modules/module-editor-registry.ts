import type {
	PanelComponent,
	PanelSpawnRequest,
	UiNodeDto,
	UserPanelDefinitionMap
} from 'golden_ui';

export interface ModuleEditorDescriptor {
	readonly nodeType: string;
	readonly panelType: string;
	readonly panelIdPrefix: string;
	readonly panelComponent: PanelComponent;
	readonly description: string;
	readonly actionLabel: string;
	readonly iconUrl: string;
	readonly category?: string;
	title(node: UiNodeDto): string;
}

const descriptors = new Map<string, ModuleEditorDescriptor>();

const normalized = (value: string): string => value.trim();

export const registerModuleEditor = (descriptor: ModuleEditorDescriptor): void => {
	const nodeType = normalized(descriptor.nodeType);
	const panelType = normalized(descriptor.panelType);
	const panelIdPrefix = normalized(descriptor.panelIdPrefix);
	if (!nodeType || !panelType || !panelIdPrefix) {
		throw new Error(
			'Module editor registration requires node type, panel type, and panel ID prefix.'
		);
	}
	descriptors.set(nodeType, {
		...descriptor,
		nodeType,
		panelType,
		panelIdPrefix
	});
};

export const unregisterModuleEditor = (nodeType: string): void => {
	descriptors.delete(normalized(nodeType));
};

export const resolveModuleEditor = (
	nodeOrType: UiNodeDto | string
): ModuleEditorDescriptor | null => {
	const nodeType = typeof nodeOrType === 'string' ? nodeOrType : nodeOrType.node_type;
	return descriptors.get(normalized(nodeType)) ?? null;
};

export const moduleEditorPanelDefinitions = (): UserPanelDefinitionMap =>
	Object.fromEntries(
		[...descriptors.values()].map((descriptor) => [
			descriptor.panelType,
			{
				title: descriptor.actionLabel,
				component: descriptor.panelComponent,
				description: descriptor.description,
				category: descriptor.category ?? 'Module Editors'
			}
		])
	);

export const moduleEditorPanelRequest = (
	descriptor: ModuleEditorDescriptor,
	node: UiNodeDto
): PanelSpawnRequest => ({
	panelId: `${descriptor.panelIdPrefix}-${node.node_id}`,
	panelType: descriptor.panelType,
	title: descriptor.title(node),
	params: { moduleNodeId: node.node_id },
	position: {
		referencePanelId: 'state-machine',
		direction: 'within'
	}
});

export const resetModuleEditorsForTests = (): void => {
	descriptors.clear();
};
