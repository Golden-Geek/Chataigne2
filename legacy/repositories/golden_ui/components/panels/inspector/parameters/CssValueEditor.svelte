<script lang="ts">
	import { onDestroy } from 'svelte';
	import Slider from '../../../common/Slider.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import { CSS_UNIT_OPTIONS } from '../../../../css-value';
	import type { CssUnit, UiNodeDto } from '../../../../types';

	let {
		node,
		layoutMode = 'default',
		readOnly: externalReadOnly = false
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let constraints = $derived(param?.constraints);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let value = $derived(param?.value.kind === 'css_value' ? param.value.value : 0);
	let unit = $derived(param?.value.kind === 'css_value' ? param.value.unit : 'rem');

	let draftValue = $state(0);
	let draftUnit = $state<CssUnit>('rem');
	let numberInput = $state(null as HTMLInputElement | null);
	let isEditing = $state(false);
	let editSession = createUiEditSession('Edit css value', 'param-css-value');
	const NUMERIC_EPSILON = 1e-9;

	let min = $derived(constraints?.range?.kind === 'uniform' ? constraints.range.min : undefined);
	let max = $derived(constraints?.range?.kind === 'uniform' ? constraints.range.max : undefined);
	let step = $derived(constraints?.step);
	let stepBase = $derived(constraints?.step_base);

	$effect(() => {
		if (isEditing) {
			return;
		}
		draftValue = value;
		draftUnit = unit;
	});

	onDestroy(() => {
		void editSession.end();
	});

	const normalizeValue = (candidate: number): number | null => {
		if (!Number.isFinite(candidate)) {
			return null;
		}
		const policy = constraints?.policy ?? 'ClampAdapt';
		const range = constraints?.range;
		let nextValue = candidate;
		if (range?.kind === 'uniform') {
			const min = range.min;
			const max = range.max;
			if (min !== undefined && max !== undefined && min > max) {
				return null;
			}
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
		}
		const step = constraints?.step;
		if (step !== undefined) {
			if (step <= 0) {
				return null;
			}
			const base =
				constraints?.step_base ??
				(constraints?.range?.kind === 'uniform' ? constraints.range.min : undefined) ??
				0;
			const scaled = (nextValue - base) / step;
			const nearest = Math.round(scaled);
			if (policy === 'ClampAdapt') {
				nextValue = base + nearest * step;
			} else if (Math.abs(scaled - nearest) > 1e-9) {
				return null;
			}
		}
		return nextValue;
	};

	const commitValue = async (nextValue: number, nextUnit: CssUnit): Promise<void> => {
		if (!param || param.value.kind !== 'css_value' || readOnly || !enabled) {
			return;
		}
		const normalized = normalizeValue(nextValue);
		if (normalized === null) {
			return;
		}
		draftValue = normalized;
		draftUnit = nextUnit;
		if (Math.abs(normalized - value) <= NUMERIC_EPSILON && nextUnit === unit) {
			return;
		}
		await sendSetParamIntent(
			liveNode.node_id,
			{ kind: 'css_value', value: normalized, unit: nextUnit },
			param.event_behaviour
		);
	};

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

	const setValueFromField = (): void => {
		if (!numberInput) {
			return;
		}
		const parsedValue = Number(numberInput.value);
		if (Number.isFinite(parsedValue)) {
			void commitValue(parsedValue, draftUnit);
			return;
		}
		numberInput.value = draftValue.toFixed(3);
	};
</script>

<div class="css-value-editor" class:widget-layout={layoutMode === 'widget'}>
	<div class="slider-wrapper">
		<Slider
			bind:value={draftValue}
			{min}
			{max}
			{step}
			{stepBase}
			{readOnly}
			disabled={!enabled}
			onValueChange={(nextValue: number) => {
				void commitValue(nextValue, draftUnit);
			}}
			onStartEdit={startEdit}
			onEndEdit={endEdit} />
	</div>
	<input
		bind:this={numberInput}
		type="number"
		class="css-value-number"
		step={constraints?.step ?? 0.01}
		value={draftValue.toFixed(3)}
		disabled={!enabled}
		class:readonly={readOnly}
		onblur={setValueFromField}
		onkeydown={(event) => {
			if (event.key === 'Enter') {
				numberInput?.blur();
			}
			if (event.key === 'Escape') {
				draftValue = value;
				if (numberInput) {
					numberInput.value = value.toFixed(3);
					numberInput.blur();
				}
			}
		}} />
	<select
		class="css-value-unit"
		value={draftUnit}
		disabled={!enabled || readOnly}
		onchange={(event) => {
			const nextUnit = (event.target as HTMLSelectElement).value as CssUnit;
			void commitValue(draftValue, nextUnit);
		}}>
		{#each CSS_UNIT_OPTIONS as option}
			<option value={option.unit}>{option.label}</option>
		{/each}
	</select>
</div>

<style>
	.css-value-editor {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		width: 100%;
		height: 1.2rem;
	}

	.css-value-editor.widget-layout {
		height: 100%;
		align-items: stretch;
	}

	.slider-wrapper {
		display: flex;
		flex: 1 1 auto;
		justify-content: right;
		align-items: center;
		min-inline-size: 0;
		height: 70%;
	}

	.css-value-editor.widget-layout .slider-wrapper {
		height: 100%;
	}

	.css-value-number {
		height: 100%;
		box-sizing: border-box;
		font-size: 0.75rem;
		width: 30%;
	}

	.css-value-editor.widget-layout .css-value-number {
		flex-basis: clamp(4rem, 18%, 6rem);
	}

	.css-value-unit {
		height: 100%;
		min-inline-size: 3.25rem;
		max-inline-size: 4rem;
		padding-inline: 0.25rem;
		font-size: 0.68rem;
		line-height: 1;
		text-overflow: ellipsis;
	}

	input::-webkit-outer-spin-button,
	input::-webkit-inner-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}
</style>
