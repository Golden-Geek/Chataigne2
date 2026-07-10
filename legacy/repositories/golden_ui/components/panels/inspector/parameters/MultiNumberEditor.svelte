<script lang="ts">
	import { onDestroy } from 'svelte';
	import Slider from '../../../common/Slider.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto, UiRangeConstraint } from '../../../../types';

	interface VectorEditorPresentation {
		layout?: 'inline' | 'column';
		show_value_fields?: boolean;
		max_decimals?: number;
	}

	let {
		node,
		layoutMode = 'default',
		presentation = {},
		rangeOverride = null,
		readOnly: externalReadOnly = false
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		presentation?: VectorEditorPresentation;
		rangeOverride?: UiRangeConstraint | null;
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let value = $derived(
		param && (param.value.kind === 'vec2' || param.value.kind === 'vec3') ? param.value.value : []
	);
	let viewRange = $derived(rangeOverride ?? param?.constraints.range ?? null);

	let draftValue = $state<number[]>([]);
	let isEditing = $state(false);
	let editSession = createUiEditSession('Edit vector', 'param-vector');
	const NUMERIC_EPSILON = 1e-9;
	let showValueFields = $derived(presentation.show_value_fields !== false);
	const inferFractionDigitsFromStep = (step: number | undefined): number | null => {
		if (!Number.isFinite(step) || step === undefined || step <= 0) {
			return null;
		}
		for (let digits = 0; digits <= 8; digits += 1) {
			const scaled = step * 10 ** digits;
			if (Math.abs(scaled - Math.round(scaled)) <= NUMERIC_EPSILON) {
				return digits;
			}
		}
		return 8;
	};
	let fractionDigits = $derived.by(() => {
		if (presentation.max_decimals !== undefined) {
			return Math.max(0, Math.min(8, Math.round(presentation.max_decimals)));
		}
		const inferred = inferFractionDigitsFromStep(param?.constraints.step);
		return inferred ?? 2;
	});
	let widgetLayoutKind = $derived(presentation.layout === 'column' ? 'column' : 'inline');

	$effect(() => {
		if (isEditing) {
			return;
		}
		draftValue = [...value];
	});

	onDestroy(() => {
		void editSession.end();
	});

	const startEdit = (): void => {
		if (readOnly || !enabled) {
			return;
		}
		isEditing = true;
		void editSession.begin();
	};

	const endEdit = (): void => {
		if (!isEditing) {
			return;
		}
		isEditing = false;
		void editSession.end();
	};

	const commitValue = (nextValue: number, index: number): void => {
		if (!param || readOnly || !enabled) {
			return;
		}
		if (param.value.kind !== 'vec2' && param.value.kind !== 'vec3') {
			return;
		}
		const normalized = normalizeComponentValue(nextValue, index);
		if (normalized === null) {
			return;
		}
		if (Math.abs((value[index] ?? 0) - normalized) <= NUMERIC_EPSILON) {
			const next = [...draftValue];
			next[index] = normalized;
			draftValue = next;
			return;
		}

		const next = [...draftValue];
		next[index] = normalized;
		draftValue = next;

		void sendSetParamIntent(
			liveNode.node_id,
			{ kind: param.value.kind, value: next as [number, number] | [number, number, number] },
			param.event_behaviour
		);
	};

	const componentMin = (index: number): number | undefined => {
		if (!viewRange) {
			return undefined;
		}
		const range = viewRange;
		if (range.kind === 'uniform') {
			return range.min;
		}
		if (range.kind === 'components') {
			return range.min?.[index];
		}
		return undefined;
	};

	const componentMax = (index: number): number | undefined => {
		if (!viewRange) {
			return undefined;
		}
		const range = viewRange;
		if (range.kind === 'uniform') {
			return range.max;
		}
		if (range.kind === 'components') {
			return range.max?.[index];
		}
		return undefined;
	};

	const normalizeComponentValue = (candidate: number, index: number): number | null => {
		if (!Number.isFinite(candidate)) {
			return null;
		}
		if (!param) {
			return null;
		}

		const min = componentMin(index);
		const max = componentMax(index);
		if (min !== undefined && max !== undefined && min > max) {
			return null;
		}

		const policy = param.constraints.policy ?? 'ClampAdapt';
		let nextValue = candidate;

		if (min !== undefined && nextValue < min) {
			if (policy === 'ClampAdapt') {
				nextValue = min;
			} else {
				return null;
			}
		}

		if (max !== undefined && nextValue > max) {
			if (policy === 'ClampAdapt') {
				nextValue = max;
			} else {
				return null;
			}
		}

		const step = param.constraints.step;
		if (step !== undefined) {
			if (step <= 0) {
				return null;
			}
			const base = param.constraints.step_base ?? min ?? 0;
			const scaled = (nextValue - base) / step;
			const nearest = Math.round(scaled);
			if (policy === 'ClampAdapt') {
				nextValue = base + nearest * step;
			} else if (Math.abs(scaled - nearest) > NUMERIC_EPSILON) {
				return null;
			}
		}

		if (policy === 'ClampAdapt') {
			if (min !== undefined) {
				nextValue = Math.max(min, nextValue);
			}
			if (max !== undefined) {
				nextValue = Math.min(max, nextValue);
			}
		}

		return nextValue;
	};

	const displayedSliderValue = (index: number): number => {
		const min = componentMin(index);
		const max = componentMax(index);
		const currentValue = draftValue[index] ?? 0;
		if (min !== undefined && currentValue < min) {
			return min;
		}
		if (max !== undefined && currentValue > max) {
			return max;
		}
		return currentValue;
	};
</script>

<div
	class="multi-number-editor"
	class:widget-layout={layoutMode === 'widget'}
	class:column-layout={layoutMode === 'widget' && widgetLayoutKind === 'column'}>
	{#if draftValue.length > 0}
		{#each draftValue as item, index}
			<div class="single-number-editor">
				<div class="slider-wrapper">
					<Slider
						value={displayedSliderValue(index)}
						min={componentMin(index)}
						max={componentMax(index)}
						step={param?.constraints.step}
						stepBase={param?.constraints.step_base}
						{readOnly}
						disabled={!enabled}
						onValueChange={(nextValue: number) => commitValue(nextValue, index)}
						onStartEdit={startEdit}
						onEndEdit={endEdit} />
				</div>
				{#if showValueFields}
					<input
						type="text"
						class="number-field"
						disabled={!enabled}
						class:readonly={readOnly}
						value={item.toFixed(fractionDigits)}
						onchange={(event) => {
							const nextValue = Number((event.target as HTMLInputElement).value);
							if (Number.isFinite(nextValue)) {
								commitValue(nextValue, index);
							}
						}}
						onkeydown={(event) => {
							if (event.key === 'Enter') {
								const nextValue = Number((event.target as HTMLInputElement).value);
								if (Number.isFinite(nextValue)) {
									commitValue(nextValue, index);
								}
								(event.target as HTMLInputElement).blur();
							} else if (event.key === 'Escape') {
								const reverted = [...draftValue];
								reverted[index] = value[index] ?? 0;
								draftValue = reverted;
								(event.target as HTMLInputElement).blur();
							}
						}} />
				{/if}
			</div>
		{/each}
	{:else}
		<span class="empty">No values</span>
	{/if}
</div>

<style>
	.multi-number-editor {
		--gc-number-field-inline-size: 5rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		width: 100%;
	}

	.multi-number-editor.widget-layout {
		flex-direction: row;
		height: 100%;
		justify-content: stretch;
		align-items: stretch;
		gap: 0.45rem;
	}

	.multi-number-editor.widget-layout.column-layout {
		flex-direction: column;
		gap: 0.45rem;
	}

	.single-number-editor {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		width: 100%;
		height: 1.2rem;
	}

	.multi-number-editor.widget-layout .single-number-editor {
		flex: 1 1 0;
		align-items: stretch;
		height: auto;
		min-inline-size: 0;
	}

	.slider-wrapper {
		display: flex;
		flex: 1 1 auto;
		align-items: stretch;
		justify-content: right;
		min-width: 0;
		height: 100%;
	}

	.multi-number-editor.widget-layout .slider-wrapper {
		height: 100%;
		inline-size: 100%;
	}

	.number-field {
		flex: 0 0 var(--gc-number-field-inline-size);
		inline-size: var(--gc-number-field-inline-size);
		height: 100%;
		box-sizing: border-box;
	}

	.multi-number-editor.widget-layout .number-field {
		inline-size: 100%;
		width: 100%;
	}

	.multi-number-editor.widget-layout.column-layout .number-field {
		inline-size: clamp(4rem, 26%, 7rem);
	}
</style>
