<script lang="ts">
	import {
		NodeInspector,
		type NodeId,
		type NodeInspectorComponentProps,
		type NodePickerModalView,
		type ParamValue,
		type UiNodeDto
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import ReferencedParameterHeaderControl from './ReferencedParameterHeaderControl.svelte';
	import ValidationChip from './ValidationChip.svelte';

	let {
		node,
		level,
		layoutMode = 'default',
		defaultHeader,
		defaultContent
	}: NodeInspectorComponentProps = $props();

	type SourceGroup = 'number' | 'string' | 'bool' | 'unknown';
	type ReferenceMode = 'none' | 'number' | 'range' | 'string';
	type ComparatorOption = {
		id: string;
		label: string;
		title: string;
		sources: readonly SourceGroup[];
		referenceMode: ReferenceMode;
	};

	const COMPARATOR_OPTIONS: readonly ComparatorOption[] = [
		{
			id: 'equal',
			label: '=',
			title: 'Equals',
			sources: ['number', 'string', 'unknown'],
			referenceMode: 'number'
		},
		{
			id: 'not_equal',
			label: '!=',
			title: 'Does not equal',
			sources: ['number', 'string', 'unknown'],
			referenceMode: 'number'
		},
		{
			id: 'greater_than',
			label: '>',
			title: 'Greater than',
			sources: ['number'],
			referenceMode: 'number'
		},
		{
			id: 'greater_than_or_equal',
			label: '>=',
			title: 'Greater than or equal',
			sources: ['number'],
			referenceMode: 'number'
		},
		{
			id: 'less_than',
			label: '<',
			title: 'Less than',
			sources: ['number'],
			referenceMode: 'number'
		},
		{
			id: 'less_than_or_equal',
			label: '<=',
			title: 'Less than or equal',
			sources: ['number'],
			referenceMode: 'number'
		},
		{
			id: 'between',
			label: 'Between',
			title: 'Between',
			sources: ['number'],
			referenceMode: 'range'
		},
		{
			id: 'outside',
			label: 'Outside',
			title: 'Outside',
			sources: ['number'],
			referenceMode: 'range'
		},
		{
			id: 'is_true',
			label: 'True',
			title: 'Is true',
			sources: ['bool'],
			referenceMode: 'none'
		},
		{
			id: 'is_false',
			label: 'False',
			title: 'Is false',
			sources: ['bool'],
			referenceMode: 'none'
		},
		{
			id: 'contains',
			label: 'Contains',
			title: 'Contains',
			sources: ['string'],
			referenceMode: 'string'
		},
		{
			id: 'does_not_contain',
			label: "Doesn't contain",
			title: "Doesn't contain",
			sources: ['string'],
			referenceMode: 'string'
		},
		{
			id: 'starts_with',
			label: 'Starts with',
			title: 'Starts with',
			sources: ['string'],
			referenceMode: 'string'
		},
		{
			id: 'ends_with',
			label: 'Ends with',
			title: 'Ends with',
			sources: ['string'],
			referenceMode: 'string'
		},
		{
			id: 'regex_match',
			label: 'Regex',
			title: 'Regex match',
			sources: ['string'],
			referenceMode: 'string'
		},
		{
			id: 'value_changed',
			label: 'Changed',
			title: 'Value changed',
			sources: ['number', 'string', 'bool', 'unknown'],
			referenceMode: 'none'
		}
	];

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let graphParentById = $derived(session?.graph.state.parentById ?? null);
	let liveNode: UiNodeDto = $derived(graphNodesById?.get(node.node_id) ?? node);
	let graphNodesByUuid = $derived.by((): ReadonlyMap<string, NodeId> => {
		if (!graphNodesById) return new Map();
		return new Map(
			Array.from(graphNodesById.values()).map((candidate) => [candidate.uuid, candidate.node_id])
		);
	});

	const childByDeclId = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphNodesById) return null;
		for (const childId of parent.children) {
			const child = graphNodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	const parameterValue = (candidate: UiNodeDto | null): ParamValue | null =>
		candidate?.data.kind === 'parameter' ? candidate.data.param.value : null;

	const stringParameterValue = (candidate: UiNodeDto | null): string | null => {
		const value = parameterValue(candidate);
		return value?.kind === 'enum' || value?.kind === 'str' || value?.kind === 'file'
			? value.value
			: null;
	};

	const boolParameterValue = (candidate: UiNodeDto | null): boolean | null => {
		const value = parameterValue(candidate);
		return value?.kind === 'bool' ? value.value : null;
	};

	const isModuleNode = (candidate: UiNodeDto | null): boolean =>
		candidate !== null &&
		(candidate.user_item_kind === 'module' || childByDeclId(candidate, 'values') !== null);

	const isModuleValuesBranch = (candidate: UiNodeDto): boolean => {
		if (!graphNodesById || !graphParentById) return false;
		let current: NodeId | undefined = candidate.node_id;
		while (current !== undefined) {
			const currentNode = graphNodesById.get(current);
			if (!currentNode) return false;
			const parentId = graphParentById.get(current);
			const parentNode = parentId === undefined ? null : (graphNodesById.get(parentId) ?? null);
			if (currentNode.decl_id === 'values' && isModuleNode(parentNode)) {
				return true;
			}
			current = parentId;
		}
		return false;
	};

	const isModuleValuesFolder = (candidate: UiNodeDto | null): boolean => {
		if (!candidate || !graphNodesById || !graphParentById || candidate.decl_id !== 'values') {
			return false;
		}
		const parentId = graphParentById.get(candidate.node_id);
		const parentNode = parentId === undefined ? null : (graphNodesById.get(parentId) ?? null);
		return isModuleNode(parentNode);
	};

	const moduleValueViewFilter = (candidate: UiNodeDto): boolean =>
		isModuleNode(candidate) || isModuleValuesBranch(candidate);

	const moduleValueViewRowFilter = (candidate: UiNodeDto): boolean =>
		isModuleNode(candidate) ||
		(isModuleValuesBranch(candidate) && !isModuleValuesFolder(candidate));

	const referencedNodeId = (value: ParamValue | null): NodeId | null => {
		if (!graphNodesById || value?.kind !== 'reference') return null;
		return value.cached_id ?? graphNodesByUuid.get(value.uuid) ?? null;
	};

	const sourceGroupFor = (source: UiNodeDto | null): SourceGroup => {
		const value = parameterValue(source);
		switch (value?.kind) {
			case 'bool':
				return 'bool';
			case 'int':
			case 'float':
			case 'css_value':
				return 'number';
			case 'str':
			case 'file':
			case 'enum':
				return 'string';
			default:
				return 'unknown';
		}
	};

	const referenceModeFor = (comparator: string, group: SourceGroup): ReferenceMode => {
		const option = COMPARATOR_OPTIONS.find((candidate) => candidate.id === comparator);
		if (group === 'unknown') return 'none';
		if (!option || option.referenceMode === 'none') return 'none';
		if (option.referenceMode === 'range') return 'range';
		if (group === 'string') return 'string';
		return option.referenceMode;
	};

	const setParameterValue = async (
		paramNode: UiNodeDto | null,
		value: ParamValue
	): Promise<void> => {
		if (paramNode?.data.kind !== 'parameter' || paramNode.data.param.read_only || !session) {
			return;
		}
		await session.sendIntent({
			kind: 'setParam',
			node: paramNode.node_id,
			value,
			behaviour: paramNode.data.param.event_behaviour
		});
	};

	let validNode = $derived(childByDeclId(liveNode, 'valid'));
	let conditionValid = $derived(
		validNode?.data.kind === 'parameter' && validNode.data.param.value.kind === 'bool'
			? validNode.data.param.value.value
			: false
	);
	let sourceReferenceNode = $derived(childByDeclId(liveNode, 'source'));
	let sourceParameter = $derived.by((): UiNodeDto | null => {
		const value = parameterValue(sourceReferenceNode);
		const sourceId = referencedNodeId(value);
		const candidate = sourceId === null ? null : (graphNodesById?.get(sourceId) ?? null);
		return candidate?.data.kind === 'parameter' ? candidate : null;
	});
	let sourceGroup = $derived(sourceGroupFor(sourceParameter));
	let comparatorNode = $derived(childByDeclId(liveNode, 'comparator'));
	let comparatorValue = $derived(stringParameterValue(comparatorNode) ?? 'equal');
	let availableComparatorOptions = $derived(
		COMPARATOR_OPTIONS.filter((option) => option.sources.includes(sourceGroup))
	);
	let selectedComparator = $derived(
		availableComparatorOptions.some((option) => option.id === comparatorValue)
			? comparatorValue
			: (availableComparatorOptions[0]?.id ?? 'value_changed')
	);
	let selectedComparatorLabel = $derived(
		availableComparatorOptions.find((option) => option.id === selectedComparator)?.label ??
			'Comparator'
	);
	let referenceMode = $derived(referenceModeFor(selectedComparator, sourceGroup));
	let toggleModeNode = $derived(childByDeclId(liveNode, 'toggle_mode'));
	let toggleMode = $derived(boolParameterValue(toggleModeNode) ?? false);
	let referenceNode = $derived(childByDeclId(liveNode, 'reference'));
	let referenceMaxNode = $derived(childByDeclId(liveNode, 'reference_max'));
	let referenceStringNode = $derived(childByDeclId(liveNode, 'reference_string'));
	let advancedNode = $derived(childByDeclId(liveNode, 'advanced'));
	let sourceReferencePickerViews = $derived<NodePickerModalView[]>([
		{
			id: 'module-value',
			label: 'Module Value',
			nodeVisibilityFilter: moduleValueViewFilter,
			nodeRowVisibilityFilter: moduleValueViewRowFilter
		},
		{
			id: 'generic',
			label: 'Generic'
		}
	]);

	$effect(() => {
		if (!sourceParameter || !comparatorNode) return;
		if (availableComparatorOptions.some((option) => option.id === comparatorValue)) return;
		const fallback = availableComparatorOptions[0]?.id;
		if (fallback) {
			void setParameterValue(comparatorNode, { kind: 'enum', value: fallback });
		}
	});
</script>

{#snippet conditionHeaderExtra()}
	<span
		class="condition-header-extra"
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<ValidationChip
			valid={conditionValid}
			title={conditionValid ? 'Condition valid' : 'Condition invalid'} />
		<ReferencedParameterHeaderControl owner={liveNode} />
	</span>
{/snippet}

{#snippet conditionContent()}
	{#if sourceReferenceNode}
		<div class="condition-source">
			<NodeInspector
				nodes={[sourceReferenceNode]}
				level={level + 1}
				order="solo"
				{layoutMode}
				referencePickerViews={sourceReferencePickerViews} />
		</div>
	{/if}

	<div class="condition-editor">
		<div
			class="condition-main-row"
			class:has-reference={referenceMode !== 'none'}
			class:has-reference-stack={referenceMode === 'range'}>
			<div class="toggle-field">
				<button
					type="button"
					title="Toggle Mode"
					aria-label="Toggle Mode"
					class:active={toggleMode}
					aria-pressed={toggleMode}
					onclick={() => {
						void setParameterValue(toggleModeNode, { kind: 'bool', value: !toggleMode });
					}}>
					Toggle
				</button>
			</div>

			<label class="comparator-field">
				<span class="visually-hidden">Comparator</span>
				<select
					value={selectedComparator}
					class:operator-symbol={selectedComparatorLabel.length <= 2}
					title={availableComparatorOptions.find((option) => option.id === selectedComparator)
						?.title ?? 'Comparator'}
					onchange={(event) => {
						const target = event.currentTarget as HTMLSelectElement;
						void setParameterValue(comparatorNode, { kind: 'enum', value: target.value });
					}}>
					{#each availableComparatorOptions as option}
						<option value={option.id} title={option.title}>{option.label}</option>
					{/each}
				</select>
			</label>

			<div class="condition-reference-area" class:empty={referenceMode === 'none'}>
				{#if referenceMode === 'number' && referenceNode}
					<NodeInspector
						nodes={[referenceNode]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{:else if referenceMode === 'range' && referenceNode && referenceMaxNode}
					<div class="condition-reference-stack">
						<NodeInspector
							nodes={[referenceNode]}
							level={level + 1}
							order="first"
							{layoutMode}
							density="compact"
							labelOverride="" />
						<NodeInspector
							nodes={[referenceMaxNode]}
							level={level + 1}
							order="last"
							{layoutMode}
							density="compact"
							labelOverride="" />
					</div>
				{:else if referenceMode === 'string' && referenceStringNode}
					<NodeInspector
						nodes={[referenceStringNode]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{/if}
			</div>
		</div>
	</div>

	{#if advancedNode}
		<div class="condition-advanced-folder">
			<NodeInspector nodes={[advancedNode]} level={level + 1} order="solo" {layoutMode} />
		</div>
	{/if}
{/snippet}

{@render defaultHeader?.(conditionHeaderExtra)}
{@render defaultContent?.(conditionContent, 'input-value-condition-inspector')}

<style>
	.condition-header-extra {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		min-inline-size: 0;
		margin-left: 0.2rem;
	}

	:global(.input-value-condition-inspector) {
		min-inline-size: 0;
	}

	.condition-editor {
		display: grid;
		gap: 0.55rem;
		min-inline-size: 0;
	}

	.condition-source,
	.condition-advanced-folder {
		min-inline-size: 0;
	}

	.condition-source {
		margin-block-end: 0.45rem;
	}

	.condition-advanced-folder {
		margin-block-start: 0.45rem;
	}

	.condition-main-row {
		display: grid;
		grid-template-columns: max-content max-content minmax(8rem, 1fr);
		gap: 0.35rem;
		align-items: center;
		min-inline-size: 0;
	}

	.toggle-field,
	.comparator-field,
	.condition-reference-area {
		display: flex;
		align-items: center;
		min-inline-size: 0;
	}

	.toggle-field button,
	.comparator-field select {
		border: 0.0625rem solid var(--gc-border, rgb(68 77 88));
		border-radius: 0.35rem;
		background: var(--gc-surface-raised, rgb(31 35 42));
		color: var(--gc-text, #e8edf2);
		font: inherit;
		line-height: 1;
	}

	.toggle-field button {
		inline-size: 3.2rem;
		block-size: 1.35rem;
		padding: 0;
		cursor: pointer;
		font-size: 0.6rem;
		font-weight: 700;
	}

	.comparator-field select {
		inline-size: 6.8rem;
		max-inline-size: 6.8rem;
		min-inline-size: 3.5rem;
		block-size: 1.45rem;
		padding-inline: 0.45rem;
		font-size: 0.72rem;
	}

	.comparator-field select.operator-symbol {
		inline-size: 3.6rem;
		max-inline-size: 3.6rem;
		min-inline-size: 3rem;
	}

	.toggle-field button.active {
		border-color: color-mix(in srgb, var(--gc-success, #47a66a) 70%, var(--gc-border, #444d58));
		background: color-mix(
			in srgb,
			var(--gc-success, #47a66a) 18%,
			var(--gc-surface-raised, #1f232a)
		);
		color: var(--gc-success, #47a66a);
	}

	.condition-reference-area {
		inline-size: 100%;
	}

	.condition-reference-area.empty {
		min-block-size: 1.45rem;
	}

	.condition-reference-stack {
		display: grid;
		gap: 0.15rem;
		inline-size: 100%;
		min-inline-size: 0;
	}

	.condition-reference-area :global(.node-inspector) {
		inline-size: 100%;
		min-inline-size: 0;
		padding-top: 0;
	}

	.condition-reference-area :global(.parameter-inspector) {
		padding-top: 0;
		padding-bottom: 0;
	}

	.condition-reference-area :global(.parameter-inspector .parameter-wrapper) {
		max-inline-size: none;
	}

	.condition-reference-area :global(.parameter-inspector.density-compact .parameter-info) {
		max-inline-size: 7rem;
	}

	.condition-reference-area :global(.parameter-inspector.density-compact .parameter-controls) {
		min-inline-size: 0;
	}

	.condition-reference-area :global(.number-property-container),
	.condition-reference-area :global(.string-editor) {
		inline-size: 100%;
		min-inline-size: 0;
	}

	.condition-reference-area
		:global(.parameter-inspector.density-compact .number-property-container.infinite) {
		inline-size: auto;
		width: auto;
	}

	.visually-hidden {
		position: absolute;
		inline-size: 0.0625rem;
		block-size: 0.0625rem;
		margin: -0.0625rem;
		overflow: hidden;
		clip: rect(0 0 0 0);
		white-space: nowrap;
	}

	@media (max-width: 42rem) {
		.condition-main-row {
			grid-template-columns: 1fr;
		}

		.toggle-field,
		.comparator-field {
			justify-self: start;
		}
	}
</style>
