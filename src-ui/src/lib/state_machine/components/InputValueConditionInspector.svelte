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
	import { selectedLaneConditionValid } from '../preview/processorLaneInspection.svelte';

	let {
		node,
		level,
		layoutMode = 'default',
		defaultHeader,
		defaultContent
	}: NodeInspectorComponentProps = $props();

	type SourceGroup =
		| 'number'
		| 'string'
		| 'bool'
		| 'trigger'
		| 'vec2'
		| 'vec3'
		| 'color'
		| 'unknown';
	type ReferenceMode = 'none' | 'number' | 'range' | 'bool' | 'string' | 'vec2' | 'vec3' | 'color';
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
			sources: ['number', 'string', 'bool', 'vec2', 'vec3', 'color', 'unknown'],
			referenceMode: 'number'
		},
		{
			id: 'not_equal',
			label: '!=',
			title: 'Does not equal',
			sources: ['number', 'string', 'bool', 'vec2', 'vec3', 'color', 'unknown'],
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
			sources: ['number', 'string', 'bool', 'vec2', 'vec3', 'color', 'unknown'],
			referenceMode: 'none'
		},
		{
			id: 'magnitude_greater_than',
			label: 'Magnitude >',
			title: 'Magnitude greater than',
			sources: ['vec2', 'vec3', 'color'],
			referenceMode: 'number'
		},
		{
			id: 'magnitude_less_than',
			label: 'Magnitude <',
			title: 'Magnitude less than',
			sources: ['vec2', 'vec3', 'color'],
			referenceMode: 'number'
		},
		{
			id: 'speed_greater_than',
			label: 'Speed >',
			title: 'Speed greater than',
			sources: ['number', 'vec2', 'vec3'],
			referenceMode: 'number'
		},
		{
			id: 'speed_less_than',
			label: 'Speed <',
			title: 'Speed less than',
			sources: ['number', 'vec2', 'vec3'],
			referenceMode: 'number'
		},
		{
			id: 'abs_speed_greater_than',
			label: 'Abs Speed >',
			title: 'Absolute speed greater than',
			sources: ['number', 'vec2', 'vec3'],
			referenceMode: 'number'
		},
		{
			id: 'abs_speed_less_than',
			label: 'Abs Speed <',
			title: 'Absolute speed less than',
			sources: ['number', 'vec2', 'vec3'],
			referenceMode: 'number'
		},
		{
			id: 'luminance_greater_than',
			label: 'Luma >',
			title: 'Luminance greater than',
			sources: ['color'],
			referenceMode: 'number'
		},
		{
			id: 'luminance_less_than',
			label: 'Luma <',
			title: 'Luminance less than',
			sources: ['color'],
			referenceMode: 'number'
		},
		{
			id: 'alpha_greater_than',
			label: 'Alpha >',
			title: 'Alpha greater than',
			sources: ['color'],
			referenceMode: 'number'
		},
		{
			id: 'alpha_less_than',
			label: 'Alpha <',
			title: 'Alpha less than',
			sources: ['color'],
			referenceMode: 'number'
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

	const projectionComponent = (projection: string | null): string | null => {
		const normalized = projection?.trim().toLowerCase();
		if (!normalized || normalized === 'none') return null;
		const component = normalized
			.split(/[.:/\\[\]]/)
			.filter((part) => part.length > 0)
			.at(-1);
		switch (component) {
			case '0':
			case 'x':
				return 'x';
			case '1':
			case 'y':
				return 'y';
			case '2':
			case 'z':
				return 'z';
			case 'r':
			case 'red':
				return 'r';
			case 'g':
			case 'green':
				return 'g';
			case 'b':
			case 'blue':
				return 'b';
			case '3':
			case 'a':
			case 'alpha':
				return 'a';
			default:
				return null;
		}
	};

	const projectedSourceGroupFor = (
		value: ParamValue | null,
		projection: string | null
	): SourceGroup | null => {
		const component = projectionComponent(projection);
		if (!component) return null;
		switch (value?.kind) {
			case 'vec2':
				return component === 'x' || component === 'y' ? 'number' : null;
			case 'vec3':
				return component === 'x' || component === 'y' || component === 'z' ? 'number' : null;
			case 'color':
				return ['r', 'g', 'b', 'a'].includes(component) ? 'number' : null;
			default:
				return null;
		}
	};

	const sourceGroupFor = (source: UiNodeDto | null, projection: string | null): SourceGroup => {
		const value = parameterValue(source);
		const projectedGroup = projectedSourceGroupFor(value, projection);
		if (projectedGroup) return projectedGroup;
		switch (value?.kind) {
			case 'bool':
				return 'bool';
			case 'trigger':
				return 'trigger';
			case 'int':
			case 'float':
			case 'css_value':
				return 'number';
			case 'vec2':
				return 'vec2';
			case 'vec3':
				return 'vec3';
			case 'color':
				return 'color';
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
		if (group === 'trigger') return 'none';
		if (group === 'unknown') return 'none';
		if (!option || option.referenceMode === 'none') return 'none';
		if (option.id === 'equal' || option.id === 'not_equal') {
			if (group === 'bool') return 'bool';
			if (group === 'vec2' || group === 'vec3' || group === 'color') return group;
		}
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
	let laneValid = $derived(selectedLaneConditionValid(liveNode));
	let conditionValid = $derived(
		laneValid ??
			(validNode?.data.kind === 'parameter' && validNode.data.param.value.kind === 'bool'
			? validNode.data.param.value.value
			: false)
	);
	let sourceReferenceNode = $derived(childByDeclId(liveNode, 'source'));
	let sourceParameter = $derived.by((): UiNodeDto | null => {
		const value = parameterValue(sourceReferenceNode);
		const sourceId = referencedNodeId(value);
		const candidate = sourceId === null ? null : (graphNodesById?.get(sourceId) ?? null);
		return candidate?.data.kind === 'parameter' ? candidate : null;
	});
	let sourceProjectionNode = $derived(childByDeclId(liveNode, 'source_projection'));
	let sourceProjection = $derived(stringParameterValue(sourceProjectionNode) ?? 'none');
	let sourceGroup = $derived(sourceGroupFor(sourceParameter, sourceProjection));
	let comparatorNode = $derived(childByDeclId(liveNode, 'comparator'));
	let comparatorValue = $derived(stringParameterValue(comparatorNode) ?? 'equal');
	let availableComparatorOptions = $derived(
		COMPARATOR_OPTIONS.filter((option) => option.sources.includes(sourceGroup))
	);
	let showComparator = $derived(sourceGroup !== 'trigger' && availableComparatorOptions.length > 0);
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
	let referenceBoolNode = $derived(childByDeclId(liveNode, 'reference_bool'));
	let referenceStringNode = $derived(childByDeclId(liveNode, 'reference_string'));
	let referenceVec2Node = $derived(childByDeclId(liveNode, 'reference_vec2'));
	let referenceVec3Node = $derived(childByDeclId(liveNode, 'reference_vec3'));
	let referenceColorNode = $derived(childByDeclId(liveNode, 'reference_color'));
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
		if (!sourceParameter || !comparatorNode || !showComparator) return;
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
		<ReferencedParameterHeaderControl owner={liveNode} />
		<ValidationChip
			valid={conditionValid}
			title={conditionValid ? 'Condition valid' : 'Condition invalid'} />
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
			class:has-reference-stack={referenceMode === 'range'}
			class:without-comparator={!showComparator}>
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

			{#if showComparator}
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
			{/if}

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
				{:else if referenceMode === 'bool' && referenceBoolNode}
					<NodeInspector
						nodes={[referenceBoolNode]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{:else if referenceMode === 'string' && referenceStringNode}
					<NodeInspector
						nodes={[referenceStringNode]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{:else if referenceMode === 'vec2' && referenceVec2Node}
					<NodeInspector
						nodes={[referenceVec2Node]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{:else if referenceMode === 'vec3' && referenceVec3Node}
					<NodeInspector
						nodes={[referenceVec3Node]}
						level={level + 1}
						order="solo"
						{layoutMode}
						density="compact"
						labelOverride="" />
				{:else if referenceMode === 'color' && referenceColorNode}
					<NodeInspector
						nodes={[referenceColorNode]}
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
	:global(.node-title:has(.condition-header-extra)) {
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		overflow: hidden;
	}

	:global(.node-title:has(.condition-header-extra) > .title-text) {
		flex: 0 1 auto;
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.condition-header-extra {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		box-sizing: border-box;
		flex: 1 1 auto;
		inline-size: auto;
		min-inline-size: 3.4rem;
		max-inline-size: none;
		margin-inline-start: 0.2rem;
		overflow: hidden;
		container-type: inline-size;
	}

	.condition-header-extra :global(.validation-chip) {
		box-sizing: border-box;
		flex: 0 0 3.4rem;
		inline-size: 3.4rem;
		min-inline-size: 3.4rem;
		margin-inline-start: auto;
		padding-inline: 0.35rem;
	}

	.condition-header-extra :global(.referenced-parameter-header-control) {
		flex: 1 1 2.5rem;
		inline-size: auto;
		min-inline-size: 0;
		max-inline-size: none;
	}

	@container (max-width: 6.15rem) {
		.condition-header-extra :global(.referenced-parameter-header-control) {
			display: none;
		}
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

	.condition-main-row.without-comparator {
		grid-template-columns: max-content minmax(8rem, 1fr);
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
