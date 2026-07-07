<script lang="ts">
	import {
		MainWindow,
		registerNodeInspector,
		registerOutlinerRowSupplement,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { appIcons } from '$lib/assets/icons/node-icons.svelte';
	import ModuleCommandInspector from '$lib/inspectors/modules/ModuleCommandInspector.svelte';
	import ModuleInspectorPanelHeader from '$lib/inspectors/modules/ModuleInspectorPanelHeader.svelte';
	import ModuleNodeInspector from '$lib/inspectors/modules/ModuleNodeInspector.svelte';
	import ModulePanel from '$lib/panels/modules/ModulePanel.svelte';
	import AlchemistEditorPanel from '$lib/state_machine/components/AlchemistEditorPanel.svelte';
	import ConditionManagerInspector from '$lib/state_machine/components/ConditionManagerInspector.svelte';
	import FormulaLibraryPanel from '$lib/state_machine/components/FormulaLibraryPanel.svelte';
	import InputSourceInspector from '$lib/state_machine/components/InputSourceInspector.svelte';
	import InputValueConditionInspector from '$lib/state_machine/components/InputValueConditionInspector.svelte';
	import ProcessorFormulaInspector from '$lib/state_machine/components/ProcessorFormulaInspector.svelte';
	import ProcessorFormulaInspectorPanelHeader from '$lib/state_machine/components/ProcessorFormulaInspectorPanelHeader.svelte';
	import StateMachinePanel from '$lib/state_machine/components/StateMachinePanel.svelte';

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
</script>

<MainWindow {userPanels} {initialPanels} nodeIcons={appIcons} />
