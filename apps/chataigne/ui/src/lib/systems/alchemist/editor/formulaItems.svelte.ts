import type { GraphNodePosition } from 'golden_graph_ui';
import type { UiNodeDto, UiCreatableUserItem, UiCreateUserItemInitialParam } from 'golden_ui';
import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
import {
	ANODE_CREATE_PREFIX,
	ANODE_NODE_TYPE,
	PROPERTY_NODE_TYPE,
	PROPERTY_MANAGER_NODE_TYPE,
	FORMULA_NODE_TYPE,
	parameterChild
} from '../alchemistGraph';
import { FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX } from '../formulaSource';
import type { ProcessorUiDto } from '../../state_machine/generated';
import type { FormulaMutations, FormulaEditorSession } from './formulaMutations';

export const PROPERTY_DRAG_TYPE = 'application/x-chataigne-alchemist-property';
const FORMULA_COPY_SOURCE_DECL_ID = 'formula_copy_source';
const MANAGER_ANODE_BY_ROLE: Record<string, string> = {
	condition: `${ANODE_CREATE_PREFIX}chataigne.conditions_manager`,
	filter: `${ANODE_CREATE_PREFIX}chataigne.filters_manager`,
	input: `${ANODE_CREATE_PREFIX}chataigne.inputs_manager`,
	output: `${ANODE_CREATE_PREFIX}chataigne.outputs_manager`
};
export function createFormulaItems(
	mutations: FormulaMutations,
	source: () => {
		formula: UiNodeDto | null;
		readOnly: boolean;
		items: UiCreatableUserItem[];
		formulas: UiNodeDto[];
		library: UiNodeDto | null;
		processor: ProcessorUiDto | null;
		graph: FormulaEditorSession['graph']['state'] | null;
		center(): GraphNodePosition | undefined;
		closeMenu(): void;
		setPreviewTarget(target: { kind: 'formula'; nodeId: number }): void;
	}
) {
	let formula = $derived(source().formula);
	let formulaReadOnly = $derived(source().readOnly);
	let graphState = $derived(source().graph);
	let anodeItems = $derived(source().items);
	let formulaNodes = $derived(source().formulas);
	let formulaLibrary = $derived(source().library);
	let processorUi = $derived(source().processor);
	const initialParam = (
		decl_id: string,
		value: UiCreateUserItemInitialParam['value']
	): UiCreateUserItemInitialParam => ({ decl_id, value });

	const formulaReferenceValue = (target: UiNodeDto): UiCreateUserItemInitialParam['value'] => ({
		kind: 'reference',
		uuid: target.uuid,
		cached_id: target.node_id,
		cached_name: target.meta.label,
		relative_path_from_root: []
	});

	const formulaNodeForBuiltinSource = (sourceKey: string): UiNodeDto | null => {
		const sourceTag = `${FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX}${sourceKey}`;
		return formulaNodes.find((formula) => formula.meta.tags.includes(sourceTag)) ?? null;
	};

	const createNode = (
		item: UiCreatableUserItem,
		position: GraphNodePosition = source().center() ?? { x: 0, y: 0 }
	): void => {
		if (
			!formula ||
			formulaReadOnly ||
			!graphState ||
			!anodeItems.some((candidate) => candidate.node_type === item.node_type)
		)
			return;
		source().closeMenu();
		void mutations
			.run(async (owner) => {
				const session = owner.session;
				const formula = owner.formula;
				if (!formula) throw new Error('Formula owner is unavailable.');
				const result = await sendCreateUserItemByTypeIntent(
					formula.node_id,
					item.node_type,
					item.label,
					{
						select_when_created: true,
						created_node_type: ANODE_NODE_TYPE,
						initial_params: [
							...item.initial_params,
							initialParam('position', {
								kind: 'vec2',
								value: [position.x, position.y]
							})
						]
					}
				);
				if (!result.success) throw new Error(`failed to create ${item.label}`);
				if (owner.current() && result.createdNodeId !== null) {
					session?.selectNode(result.createdNodeId, 'REPLACE');
				}
			})
			.catch(() => undefined);
	};

	const createPropertyItem = (parent: UiNodeDto, item: UiCreatableUserItem): void => {
		if (formulaReadOnly) return;
		void mutations
			.run(async (owner) => {
				const session = owner.session;
				const formula = owner.formula;
				if (!formula) throw new Error('Formula owner is unavailable.');
				const result = await sendCreateUserItemByTypeIntent(
					parent.node_id,
					item.node_type,
					item.label,
					{
						select_when_created: true,
						initial_params: item.initial_params
					}
				);
				if (!result.success) throw new Error(`failed to create ${item.label}`);
				if (owner.current() && result.createdNodeId !== null) {
					session?.selectNode(result.createdNodeId, 'REPLACE');
				}
			})
			.catch(() => undefined);
	};

	const createEditableBuiltInFormulaCopy = (): void => {
		if (
			!processorUi ||
			processorUi.formula_source_kind !== 'builtin' ||
			!processorUi.formula_can_duplicate_to_library ||
			!formulaLibrary ||
			!formulaLibrary.creatable_user_items.some((item) => item.node_type === FORMULA_NODE_TYPE)
		) {
			return;
		}
		const formulaLabel = processorUi.formula_label;
		const sourceKey = processorUi.formula_source_key;
		if (!sourceKey) return;
		const sourceFormula = formulaNodeForBuiltinSource(sourceKey);
		if (!sourceFormula) return;
		const libraryNodeId = formulaLibrary.node_id;
		void mutations
			.run(async (owner) => {
				const session = owner.session;
				const result = await sendCreateUserItemByTypeIntent(
					libraryNodeId,
					FORMULA_NODE_TYPE,
					formulaLabel,
					{
						select_when_created: true,
						created_node_type: FORMULA_NODE_TYPE,
						initial_params: [
							initialParam(FORMULA_COPY_SOURCE_DECL_ID, formulaReferenceValue(sourceFormula))
						]
					}
				);
				if (!result.success) throw new Error(`failed to duplicate ${formulaLabel}`);
				if (owner.current() && result.createdNodeId !== null) {
					session?.selectNode(result.createdNodeId, 'REPLACE');
					source().setPreviewTarget({ kind: 'formula', nodeId: result.createdNodeId });
				}
			}, false)
			.catch(() => undefined);
	};

	const createPropertyGetter = (property: UiNodeDto, position: GraphNodePosition): void => {
		if (!formula || formulaReadOnly || !graphState || property.node_type !== PROPERTY_NODE_TYPE) {
			return;
		}
		void mutations
			.run(async (owner) => {
				const session = owner.session;
				const formula = owner.formula;
				if (!formula) throw new Error('Formula owner is unavailable.');
				const result = await sendCreateUserItemByTypeIntent(
					formula.node_id,
					`${ANODE_CREATE_PREFIX}property`,
					property.meta.label,
					{
						select_when_created: true,
						created_node_type: ANODE_NODE_TYPE,
						initial_params: [
							initialParam('position', {
								kind: 'vec2',
								value: [position.x, position.y]
							}),
							initialParam('config/property_id', {
								kind: 'reference',
								uuid: property.uuid,
								cached_id: property.node_id,
								cached_name: property.meta.label,
								relative_path_from_root: []
							})
						]
					}
				);
				if (!result.success) {
					throw new Error(`failed to create getter for ${property.meta.label}`);
				}
				if (owner.current() && result.createdNodeId !== null) {
					session?.selectNode(result.createdNodeId, 'REPLACE');
				}
			})
			.catch(() => undefined);
	};

	const managerRole = (manager: UiNodeDto): string | null => {
		if (!graphState) return null;
		const role = parameterChild(manager, graphState.nodesById, 'role');
		if (role?.data.kind !== 'parameter') return null;
		const value = role.data.param.value;
		return value.kind === 'enum' ? value.value : null;
	};

	const createManagerGetter = (manager: UiNodeDto, position: GraphNodePosition): void => {
		if (
			!formula ||
			formulaReadOnly ||
			!graphState ||
			manager.node_type !== PROPERTY_MANAGER_NODE_TYPE
		) {
			return;
		}
		const role = managerRole(manager);
		const nodeType = role ? MANAGER_ANODE_BY_ROLE[role] : undefined;
		if (!nodeType) return;
		void mutations
			.run(async (owner) => {
				const session = owner.session;
				const formula = owner.formula;
				if (!formula) throw new Error('Formula owner is unavailable.');
				const result = await sendCreateUserItemByTypeIntent(
					formula.node_id,
					nodeType,
					manager.meta.label,
					{
						select_when_created: true,
						created_node_type: ANODE_NODE_TYPE,
						initial_params: [
							initialParam('position', {
								kind: 'vec2',
								value: [position.x, position.y]
							}),
							initialParam('config/manager_id', {
								kind: 'reference',
								uuid: manager.uuid,
								cached_id: manager.node_id,
								cached_name: manager.meta.label,
								relative_path_from_root: []
							})
						]
					}
				);
				if (!result.success) {
					throw new Error(`failed to create manager node for ${manager.meta.label}`);
				}
				if (owner.current() && result.createdNodeId !== null) {
					session?.selectNode(result.createdNodeId, 'REPLACE');
				}
			})
			.catch(() => undefined);
	};

	return {
		createNode,
		createPropertyItem,
		createEditableBuiltInFormulaCopy,
		createPropertyGetter,
		createManagerGetter
	};
}
