<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { createFixtureSession } from 'golden_ui/testing/session';
	import { measureCollectionFlow } from 'golden_ui/testing/collections';
	import { sendPatchMetaIntent } from 'golden_ui/store/ui-intents';
	import type { PanelApi, PanelParams, PanelProps } from 'golden_ui';
	import ModulePanel from '../../../src/lib/panels/modules/ModulePanel.svelte';
	import FormulaLibraryPanel from '../../../src/lib/systems/alchemist/components/FormulaLibraryPanel.svelte';
	import StateProcessorManager from '../../../src/lib/systems/alchemist/components/StateProcessorManager.svelte';
	import AlchemistEditorPanel from '../../../src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte';
	import { appTreeNodes, type AppTreeKind } from './tree-nodes';

	const fixture = createFixtureSession();
	appState.session = fixture.session;
	let kind = $state<AppTreeKind>('modules');
	let size = 0;
	let generation = $state(0);
	let modulePanel = $state<ReturnType<typeof ModulePanel>>();
	let formulaPanel = $state<ReturnType<typeof FormulaLibraryPanel>>();
	let processorPanel = $state<ReturnType<typeof StateProcessorManager>>();
	let parameters = $state.raw<PanelParams>({});
	const panelApi: PanelApi = {
		setTitle: () => {},
		close: () => {},
		getParams: <T extends PanelParams = PanelParams>() => parameters as T,
		updateParams: (next) => {
			parameters = { ...parameters, ...next };
		}
	};
	let props = $derived({
		panelApi,
		panelId: 'app-tree',
		panelType: 'fixture',
		title: 'App trees',
		params: parameters
	} satisfies PanelProps);
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setup(nextKind: AppTreeKind, count: number) {
		kind = nextKind;
		size = count;
		generation++;
		fixture.loadNodes(appTreeNodes(kind, size));
		parameters = kind === 'properties' ? { formulaNodeId: size + 5 } : {};
		await frames();
		return { kind, size, nodes: fixture.session.graph.state.nodesById.size };
	}
	const panel = () =>
		kind === 'modules' ? modulePanel! : kind === 'formulas' ? formulaPanel! : processorPanel!;
	export async function focusNode(id: number) {
		if (kind !== 'properties') await panel().focusTreeNode(id);
		else {
			fixture.session.selectNode(id, 'REPLACE');
			await frames();
			document
				.querySelector<HTMLElement>(`.manager-list-content [data-tree-node-id="${id}"]`)
				?.focus();
		}
		await frames();
	}
	export async function navigate(key: string) {
		if (kind === 'properties') {
			if (key === 'End' || key === 'Home') await focusNode(key === 'End' ? size + 1 : 2);
			else if (key === 'ArrowRight' || key === 'ArrowLeft') await setExpanded(key === 'ArrowRight');
			else if (key === 'Enter')
				fixture.session.selectNode(
					Number(document.activeElement?.getAttribute('data-tree-node-id')),
					'REPLACE'
				);
		} else await panel().navigateTree(key);
		await frames();
	}
	export async function setExpanded(expanded: boolean) {
		if (kind === 'properties')
			await sendPatchMetaIntent(2, { presentation: { collapsed: !expanded } });
		else panel().setNodeExpanded(2, expanded);
		await frames();
	}
	export const selection = () => [...fixture.session.selectedNodesIds];
	export async function filterBuiltin() {
		formulaPanel!.toggleSource('builtin');
		await frames();
	}
	export async function measure(action: 'focus' | 'expand') {
		await focusNode(2);
		if (action === 'expand') await setExpanded(true);
		return measureCollectionFlow(
			`app-tree-${kind}-${action}`,
			size,
			(index) => {
				if (action === 'focus') void navigate(index % 2 ? 'Home' : 'End');
				else void setExpanded(Boolean(index % 2));
			},
			(index) => {
				const id = action === 'focus' ? (index % 2 ? 2 : size + 1) : 2;
				const row = document.querySelector<HTMLElement>(
					`.manager-list-content [data-tree-node-id="${id}"]`
				);
				const viewport = document.querySelector<HTMLElement>('.manager-list-content');
				if (
					!row ||
					!viewport ||
					row.getBoundingClientRect().bottom <= viewport.getBoundingClientRect().top ||
					row.getBoundingClientRect().top >= viewport.getBoundingClientRect().bottom
				)
					return false;
				return action === 'focus'
					? row === document.activeElement
					: row.getAttribute('aria-expanded') === String(Boolean(index % 2));
			}
		);
	}
	onDestroy(() => {
		appState.session = null;
	});
</script>

<main data-app-tree={kind}>
	{#key generation}
		{#if kind === 'modules'}<ModulePanel bind:this={modulePanel} {...props} />
		{:else if kind === 'formulas'}<FormulaLibraryPanel bind:this={formulaPanel} {...props} />
		{:else if kind === 'processors'}<StateProcessorManager
				bind:this={processorPanel}
				manager={fixture.session.graph.state.nodesById.get(1)!} />
		{:else}<AlchemistEditorPanel {...props} />{/if}
	{/key}
</main>

<style>
	main {
		inline-size: 70rem;
		block-size: 36rem;
	}
</style>
