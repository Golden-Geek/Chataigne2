<script lang="ts">
	import { browser, dev } from '$app/environment';
	import {
		MainWindow,
		registerNodeInspector,
		registerNodeInspectorMatcher,
		registerOutlinerRowSupplement,
		type PanelSpawnRequest,
		type UiNodeDto,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { appIcons } from '$lib/assets/icons/node-icons.svelte';
	import { resolveRuntimeEndpoints } from '$lib/runtimeEndpoints';
	import ModuleCommandInspector from '$lib/inspectors/modules/ModuleCommandInspector.svelte';
	import ModuleInspectorPanelHeader from '$lib/inspectors/modules/ModuleInspectorPanelHeader.svelte';
	import ModuleNodeInspector from '$lib/inspectors/modules/ModuleNodeInspector.svelte';
	import ModulePanel from '$lib/panels/modules/ModulePanel.svelte';
	import {
		moduleEditorPanelDefinitions,
		registerDefaultModuleEditors
	} from '$lib/panels/modules/module-editor-setup';
	import AlchemistEditorPanel from '$lib/systems/alchemist/components/AlchemistEditorPanel.svelte';
	import ConditionManagerInspector from '$lib/systems/alchemist/components/ConditionManagerInspector.svelte';
	import FormulaLibraryPanel from '$lib/systems/alchemist/components/FormulaLibraryPanel.svelte';
	import FormulaNodeInspector from '$lib/systems/alchemist/components/FormulaNodeInspector.svelte';
	import InputSourceInspector from '$lib/systems/alchemist/components/InputSourceInspector.svelte';
	import InputValueConditionInspector from '$lib/systems/alchemist/components/InputValueConditionInspector.svelte';
	import ProcessorFormulaInspector from '$lib/systems/alchemist/components/ProcessorFormulaInspector.svelte';
	import ProcessorFormulaInspectorPanelHeader from '$lib/systems/alchemist/components/ProcessorFormulaInspectorPanelHeader.svelte';
	import StateMachinePanel from '$lib/systems/state_machine/components/StateMachinePanel.svelte';
	import { registerSharedFormulaRemovalGuard } from '$lib/systems/alchemist/sharedFormulaRemoval';
	import { registerProcessorLaneParameterPreviews } from '$lib/systems/alchemist/preview/processorLaneInspection.svelte';
	import SoundCardDirectionParametersInspector from '$lib/modules/audio/sound-card/SoundCardDirectionParametersInspector.svelte';
	import SoundCardConnectionInspector from '$lib/modules/audio/sound-card/SoundCardConnectionInspector.svelte';
	import SoundCardRoutingInspector from '$lib/modules/audio/sound-card/SoundCardRoutingInspector.svelte';

	registerSharedFormulaRemovalGuard();
	registerProcessorLaneParameterPreviews();
	registerDefaultModuleEditors();

	registerNodeInspector('module_command', {
		component: ModuleCommandInspector
	});

	// Generic (module-independent) output commands share the command inspector so
	// their Trigger button renders in the header, exactly like module commands.
	registerNodeInspector('generic_command', {
		component: ModuleCommandInspector
	});

	// The Outputs manager and Output groups expose a Trigger button (fire all
	// contained outputs) in their header via the same inspector.
	registerNodeInspector('sm_outputs_manager', {
		component: ModuleCommandInspector
	});
	registerNodeInspector('sm_output_group', {
		component: ModuleCommandInspector
	});

	registerNodeInspector('module', {
		component: ModuleNodeInspector,
		panelHeaderComponent: ModuleInspectorPanelHeader
	});
	registerNodeInspector('sound_card_input_routing', {
		component: SoundCardRoutingInspector
	});
	registerNodeInspector('sound_card_output_routing', {
		component: SoundCardRoutingInspector
	});
	registerNodeInspector('sound_card_input_parameters', {
		component: SoundCardDirectionParametersInspector
	});
	registerNodeInspector('sound_card_output_parameters', {
		component: SoundCardDirectionParametersInspector
	});
	registerNodeInspectorMatcher(
		'sound-card-connection',
		(node: UiNodeDto): boolean => {
			const declaredKey = node.decl_id.split('/').at(-1) ?? node.decl_id;
			if (declaredKey !== 'connection') return false;

			const session = appState.session;
			let current: UiNodeDto | undefined = node;
			while (current && session) {
				if (current.node_type === 'sound_card_module') return true;
				const parentId = session.graph.state.parentById.get(current.node_id);
				current = parentId === undefined ? undefined : session.graph.state.nodesById.get(parentId);
			}
			return false;
		},
		{ component: SoundCardConnectionInspector }
	);

	registerNodeInspector('state_processor', {
		component: ProcessorFormulaInspector,
		panelHeaderComponent: ProcessorFormulaInspectorPanelHeader
	});

	registerNodeInspector('sm_input_value_condition', {
		component: InputValueConditionInspector
	});

	registerNodeInspector('sm_condition_manager', {
		component: ConditionManagerInspector
	});

	registerNodeInspector('sm_input_source', {
		component: InputSourceInspector
	});

	registerNodeInspector('alchemist_formula', {
		component: FormulaNodeInspector
	});

	// registerOutlinerRowSupplement('module', {
	// 	component: ModuleItem
	// });

	const userPanels: UserPanelDefinitionMap = {
		modules: {
			title: 'Modules',
			component: ModulePanel,
			description: 'Filtered outliner view for modules'
		},
		stateMachine: {
			title: 'State Machine',
			component: StateMachinePanel,
			description: 'Statechart and processor node graphs'
		},
		alchemistEditor: {
			title: 'Alchemist Editor',
			component: AlchemistEditorPanel,
			description: 'Visual editor for custom Alchemist Formulas'
		},
		...moduleEditorPanelDefinitions(),
		formulaLibrary: {
			title: 'Formula Library',
			component: FormulaLibraryPanel,
			description: 'View and manage Alchemist formulas'
		}
	};

	const initialPanels: PanelSpawnRequest[] = [
		{
			panelId: 'outliner',
			panelType: 'outliner'
		},
		{
			panelId: 'state-machine',
			panelType: 'stateMachine',
			required: true,
			position: {
				referencePanelId: 'outliner',
				direction: 'right'
			}
		},
		{
			panelId: 'dashboard',
			panelType: 'dashboard',
			inactive: true,
			position: {
				referencePanelId: 'state-machine',
				direction: 'within'
			}
		},
		{
			panelId: 'formula-library',
			panelType: 'formulaLibrary',
			position: {
				referencePanelId: 'state-machine',
				direction: 'within'
			}
		},
		{
			panelId: 'logger',
			panelType: 'logger',
			position: {
				referencePanelId: 'state-machine',
				direction: 'below'
			}
		},
		{
			panelId: 'inspector',
			panelType: 'inspector',
			position: {
				referencePanelId: 'state-machine',
				direction: 'right'
			}
		}
	];

	const runtimeEndpoints = browser
		? resolveRuntimeEndpoints(window.location, { development: dev })
		: undefined;
</script>

<MainWindow
	{userPanels}
	{initialPanels}
	nodeIcons={appIcons}
	httpBaseUrl={runtimeEndpoints?.httpBaseUrl}
	wsUrl={runtimeEndpoints?.wsUrl} />
