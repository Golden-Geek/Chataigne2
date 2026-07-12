import type { UiNodeDto, UiSnapshot } from '../../types';

export interface WorkbenchDescriptionStore {
	applySnapshotSchema(schema: UiSnapshot['schema']): void;
	getNodeDescription(node: UiNodeDto | null | undefined): string | null;
	reset(): void;
}

const normalizeDescription = (value: string | null | undefined): string | null => {
	const text = typeof value === 'string' ? value.trim() : '';
	return text.length > 0 ? text : null;
};

export const createWorkbenchDescriptionStore = (): WorkbenchDescriptionStore => {
	let nodeTypeDescriptions = $state<Map<string, string>>(new Map());
	let declaredDescriptions = $state<Map<string, string>>(new Map());

	const rebuildNodeTypeDescriptions = (descriptors: UiSnapshot['schema']['node_types']): void => {
		const nextDescriptions = new Map<string, string>();
		for (const descriptor of descriptors) {
			const nodeType = descriptor.node_type.trim();
			const description = normalizeDescription(descriptor.description);
			if (nodeType.length === 0 || description === null) {
				continue;
			}
			nextDescriptions.set(nodeType, description);
		}
		nodeTypeDescriptions = nextDescriptions;
	};

	const rebuildDeclaredDescriptions = (
		descriptors: UiSnapshot['schema']['declared_descriptions']
	): void => {
		const nextDescriptions = new Map<string, string>();
		for (const descriptor of descriptors) {
			const key = descriptor.key.trim();
			const description = normalizeDescription(descriptor.description);
			if (key.length === 0 || description === null) {
				continue;
			}
			nextDescriptions.set(key, description);
		}
		declaredDescriptions = nextDescriptions;
	};

	const applySnapshotSchema = (schema: UiSnapshot['schema']): void => {
		rebuildNodeTypeDescriptions(schema.node_types);
		rebuildDeclaredDescriptions(schema.declared_descriptions);
	};

	const getNodeDescription = (node: UiNodeDto | null | undefined): string | null => {
		if (!node) {
			return null;
		}
		const declaredDescriptionKey =
			typeof node.meta.declared_description_key === 'string'
				? node.meta.declared_description_key.trim()
				: '';
		if (declaredDescriptionKey.length > 0) {
			if (node.meta.description_overridden) {
				return normalizeDescription(node.meta.description);
			}
			return declaredDescriptions.get(declaredDescriptionKey) ?? null;
		}
		return (
			normalizeDescription(node.meta.description) ??
			nodeTypeDescriptions.get(node.node_type) ??
			null
		);
	};

	const reset = (): void => {
		nodeTypeDescriptions = new Map();
		declaredDescriptions = new Map();
	};

	return {
		applySnapshotSchema,
		getNodeDescription,
		reset
	};
};
