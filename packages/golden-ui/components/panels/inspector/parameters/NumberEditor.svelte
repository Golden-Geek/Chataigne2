<script lang="ts">
	import { onDestroy } from 'svelte';
	import Slider from '../../../common/Slider.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto, UiRangeConstraint } from '../../../../types';

	interface NumberEditorPresentation {
		show_value_field?: boolean;
		show_slider?: boolean;
		compact?: boolean;
		max_decimals?: number;
		inside_label?: string;
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
		presentation?: NumberEditorPresentation;
		rangeOverride?: UiRangeConstraint | null;
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let constraints = $derived(param?.constraints);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let widgetHint = $derived((param?.ui_hints.widget ?? '').trim().toLowerCase());
	let timeSliderWidget = $derived(widgetHint === 'time_slider');
	let timeWidget = $derived(widgetHint === 'time' || timeSliderWidget);

	let kind = $derived(param?.value.kind ?? 'float');
	let isInteger = $derived(kind === 'int');
	let value = $derived(
		param && (param.value.kind === 'int' || param.value.kind === 'float') ? param.value.value : 0
	);

	let viewRange = $derived(rangeOverride ?? constraints?.range ?? null);
	let min = $derived(viewRange?.kind === 'uniform' ? viewRange.min : undefined);
	let max = $derived(viewRange?.kind === 'uniform' ? viewRange.max : undefined);
	let step = $derived(constraints?.step);
	let stepBase = $derived(constraints?.step_base);
	let hasRange = $derived(min !== undefined && max !== undefined);

	let numberInput = $state(null as HTMLInputElement | null);
	let draftValue = $state(0);
	let isEditing = $state(false);
	let editSession = createUiEditSession('Edit number', 'param-number');
	const NUMERIC_EPSILON = 1e-9;
	let forceSlider = $derived(presentation.show_slider === true || timeSliderWidget);
	let fieldOnlyWidget = $derived(
		!forceSlider &&
			(timeWidget ||
				widgetHint === 'text' ||
				widgetHint === 'input' ||
				widgetHint === 'field' ||
				widgetHint === 'number_field')
	);
	let showSlider = $derived(!fieldOnlyWidget);
	let showValueField = $derived(fieldOnlyWidget || presentation.show_value_field !== false);
	let fractionDigits = $derived(
		isInteger ? 0 : Math.max(0, Math.min(8, Math.round(presentation.max_decimals ?? 3)))
	);
	let fieldStep = $derived(
		isInteger ? 1 : fractionDigits <= 0 ? 1 : Number(`1e-${fractionDigits}`)
	);
	let sliderLabel = $derived(
		layoutMode === 'widget' && typeof presentation.inside_label === 'string'
			? presentation.inside_label.trim()
			: ''
	);
	let showsInlineFieldLabel = $derived(!hasRange && showValueField && sliderLabel.length > 0);
	let formattedDraftValue = $derived(
		timeWidget ? formatTimeValue(draftValue) : draftValue.toFixed(fractionDigits)
	);
	let formattedValue = $derived(
		timeWidget ? formatTimeValue(value) : value.toFixed(fractionDigits)
	);
	let displayedSliderValue = $derived.by(() => {
		if (min !== undefined && draftValue < min) {
			return min;
		}
		if (max !== undefined && draftValue > max) {
			return max;
		}
		return draftValue;
	});

	$effect(() => {
		if (isEditing) {
			return;
		}
		draftValue = value;
	});

	onDestroy(() => {
		void editSession.end();
	});

	function formatTimeValue(seconds: number): string {
		if (!Number.isFinite(seconds)) {
			return '00:00:00.000';
		}

		const sign = seconds < 0 ? '-' : '';
		let remainingMs = Math.round(Math.abs(seconds) * 1000);
		const hours = Math.floor(remainingMs / 3_600_000);
		remainingMs -= hours * 3_600_000;
		const minutes = Math.floor(remainingMs / 60_000);
		remainingMs -= minutes * 60_000;
		const wholeSeconds = Math.floor(remainingMs / 1000);
		const milliseconds = remainingMs - wholeSeconds * 1000;

		return `${sign}${hours.toString().padStart(2, '0')}:${minutes
			.toString()
			.padStart(2, '0')}:${wholeSeconds.toString().padStart(2, '0')}.${milliseconds
			.toString()
			.padStart(3, '0')}`;
	}

	function parseTimeValue(text: string): number | null {
		const raw = text.trim();
		if (raw.length === 0) {
			return null;
		}

		const sign = raw.startsWith('-') ? -1 : 1;
		const unsigned = sign < 0 ? raw.slice(1) : raw;
		const [clockPart, fractionalPart = ''] = unsigned.split('.');
		if (unsigned.split('.').length > 2 || !/^\d*$/.test(fractionalPart)) {
			return null;
		}

		const segments = clockPart.split(':');
		if (
			segments.length === 0 ||
			segments.length > 3 ||
			segments.some((segment) => !/^\d+$/.test(segment))
		) {
			return null;
		}

		let hours = 0;
		let minutes = 0;
		let wholeSeconds = 0;
		if (segments.length === 3) {
			hours = Number(segments[0]);
			minutes = Number(segments[1]);
			wholeSeconds = Number(segments[2]);
		} else if (segments.length === 2) {
			minutes = Number(segments[0]);
			wholeSeconds = Number(segments[1]);
		} else {
			wholeSeconds = Number(segments[0]);
		}

		if ((segments.length === 3 && minutes >= 60) || (segments.length >= 2 && wholeSeconds >= 60)) {
			return null;
		}

		const milliseconds = Number((fractionalPart + '000').slice(0, 3));
		return sign * (hours * 3600 + minutes * 60 + wholeSeconds + milliseconds / 1000);
	}

	const normalizeValue = (candidate: number): number | null => {
		let nextValue = candidate;
		if (!Number.isFinite(nextValue)) {
			return null;
		}
		const policy = constraints?.policy ?? 'ClampAdapt';
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

		if (step !== undefined) {
			if (step <= 0) {
				return null;
			}
			const base = stepBase ?? min ?? 0;
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

		if (isInteger) {
			const rounded = Math.round(nextValue);
			if (policy === 'Reject' && Math.abs(nextValue - rounded) > NUMERIC_EPSILON) {
				return null;
			}
			nextValue = Math.round(nextValue);
		}
		return nextValue;
	};

	const commitValue = (candidate: number): void => {
		if (!param || readOnly || !enabled) {
			return;
		}
		if (param.value.kind !== 'int' && param.value.kind !== 'float') {
			return;
		}
		const nextValue = normalizeValue(candidate);
		if (nextValue === null) {
			return;
		}
		if (Math.abs(nextValue - value) <= NUMERIC_EPSILON) {
			draftValue = nextValue;
			return;
		}
		draftValue = nextValue;
		void sendSetParamIntent(
			liveNode.node_id,
			{ kind: param.value.kind, value: nextValue },
			param.event_behaviour
		);
	};

	function setValueFromField(): void {
		if (!numberInput) {
			return;
		}
		const parsedValue = timeWidget ? parseTimeValue(numberInput.value) : Number(numberInput.value);
		if (parsedValue !== null && Number.isFinite(parsedValue)) {
			commitValue(parsedValue);
		} else {
			numberInput.value = formattedDraftValue;
		}
	}

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
</script>

<div
	class="number-property-container"
	class:infinite={!hasRange}
	class:field-only={!showSlider}
	class:compact={presentation.compact === true}
	class:widget-layout={layoutMode === 'widget'}>
	{#if showSlider}
		<div class="slider-wrapper">
			<Slider
				value={displayedSliderValue}
				{min}
				{max}
				{step}
				{stepBase}
				{readOnly}
				disabled={!enabled}
				label={hasRange ? sliderLabel : ''}
				forceInfinite={isInteger}
				onValueChange={(nextValue: number) => {
					commitValue(nextValue);
				}}
				onStartEdit={startEdit}
				onEndEdit={endEdit} />
		</div>
	{/if}

	{#if showValueField}
		{#if showsInlineFieldLabel}
			<div class="number-field-shell">
				<span class="number-field-prefix">{sliderLabel} :</span>
				<input
					bind:this={numberInput}
					type={timeWidget ? 'text' : 'number'}
					step={timeWidget ? undefined : fieldStep}
					class="number-field inline-labeled"
					class:time-field={timeWidget}
					disabled={!enabled}
					class:readonly={readOnly}
					value={formattedDraftValue}
					onblur={setValueFromField}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							numberInput?.blur();
						} else if (event.key === 'Escape') {
							if (numberInput) {
								numberInput.value = formattedValue;
								numberInput.blur();
							}
						}
					}} />
			</div>
		{:else}
			<input
				bind:this={numberInput}
				type={timeWidget ? 'text' : 'number'}
				step={timeWidget ? undefined : fieldStep}
				class="number-field"
				class:time-field={timeWidget}
				disabled={!enabled}
				class:readonly={readOnly}
				value={formattedDraftValue}
				onblur={setValueFromField}
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						numberInput?.blur();
					} else if (event.key === 'Escape') {
						if (numberInput) {
							numberInput.value = formattedValue;
							numberInput.blur();
						}
					}
				}} />
		{/if}
	{/if}
</div>

<style>
	.slider-wrapper {
		display: flex;
		justify-content: right;
		flex-grow: 1;
		align-items: stretch;
		height: 100%;
		min-width: 0;
	}

	.number-property-container {
		--gc-number-field-inline-size: 5rem;
		--gc-number-time-field-inline-size: 7.2rem;
		display: flex;
		align-items: center;
		justify-content: right;
		gap: 0.25rem;
		width: 100%;
		height: 1.2rem;
	}

	.number-property-container.widget-layout {
		inline-size: 100%;
		block-size: 100%;
		height: 100%;
		align-items: stretch;
		gap: 0.2rem;
	}

	.number-property-container.widget-layout.compact {
		gap: 0.18rem;
	}

	.number-property-container.widget-layout .slider-wrapper {
		flex: 1 1 auto;
		height: 100%;
	}

	/* Infinite (rangeless) numbers: keep the drag handle right next to the field
	   instead of letting the slider wrapper grow and push them apart. Applies in
	   the inspector (default layout) too. */
	.number-property-container.infinite {
		gap: 0.2rem;
		flex: 0 0 auto;
		inline-size: auto;
		width: auto;
		justify-content: flex-start;
	}

	.number-property-container.infinite .slider-wrapper {
		flex: 0 0 auto;
	}

	.number-property-container.infinite .number-field {
		flex: 0 0 var(--gc-number-field-inline-size);
		inline-size: var(--gc-number-field-inline-size);
		margin-left: 0;
	}

	:global(.parameter-inspector.density-compact) .number-property-container {
		justify-content: flex-end;
		min-inline-size: 0;
	}

	:global(.parameter-inspector.density-compact)
		.number-property-container.infinite
		.slider-wrapper {
		flex: 0 0 auto;
		justify-content: center;
	}

	:global(.parameter-inspector.density-compact) .number-property-container.infinite {
		flex: 0 0 auto;
		inline-size: auto;
		justify-content: flex-start;
		width: auto;
	}

	:global(.parameter-inspector.density-compact) .number-property-container .number-field {
		flex: 0 0 auto;
		inline-size: var(--gc-number-field-inline-size);
		min-inline-size: var(--gc-number-field-inline-size);
		max-width: none;
		margin-left: 0;
	}

	:global(.parameter-inspector.density-compact)
		.number-property-container
		.number-field.time-field {
		inline-size: var(--gc-number-time-field-inline-size);
		min-inline-size: var(--gc-number-time-field-inline-size);
	}

	:global(.parameter-inspector.density-compact) .number-property-container.infinite .number-field {
		flex: 0 0 var(--gc-number-field-inline-size);
		inline-size: var(--gc-number-field-inline-size);
		min-inline-size: var(--gc-number-field-inline-size);
	}

	.number-property-container.field-only .number-field {
		width: 100%;
		margin-left: 0;
	}

	.number-property-container.field-only .number-field-shell {
		flex: 1 1 auto;
	}

	.number-field {
		flex: 0 0 var(--gc-number-field-inline-size);
		inline-size: var(--gc-number-field-inline-size);
		height: 100%;
		box-sizing: border-box;
		max-width: none;
		margin-left: 0.25rem;
	}

	.infinite .number-field {
		max-width: none;
	}

	.number-field.time-field {
		font-variant-numeric: tabular-nums;
		flex: 0 0 var(--gc-number-time-field-inline-size);
		inline-size: var(--gc-number-time-field-inline-size);
		min-inline-size: var(--gc-number-time-field-inline-size);
		max-width: none;
	}

	.number-field-shell {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		gap: 0.25rem;
		min-inline-size: 0;
		padding: 0 0.45rem;
		border-radius: 0.7rem;
		background: rgb(from var(--gc-color-background) r g b / 0.48);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.48);
		box-sizing: border-box;
	}

	.number-field-prefix {
		flex: 0 0 auto;
		font-size: 0.72rem;
		font-weight: 600;
		white-space: nowrap;
	}

	.number-property-container.widget-layout .number-field {
		flex: 0 0 auto;
		inline-size: clamp(4rem, 18%, 6rem);
		min-inline-size: 4rem;
		width: auto;
		max-width: none;
		margin-left: 0;
	}

	.number-property-container.widget-layout.compact .number-field {
		inline-size: clamp(3.2rem, 38%, 4.1rem);
		min-inline-size: 3.2rem;
	}

	.number-property-container.widget-layout .number-field.time-field {
		inline-size: clamp(7rem, 26%, 10rem);
		min-inline-size: 7rem;
	}

	.number-property-container.widget-layout.infinite .slider-wrapper {
		flex: 0 0 2rem;
		justify-content: center;
	}

	.number-property-container.widget-layout.compact.infinite .slider-wrapper {
		flex: 1 1 auto;
		justify-content: stretch;
	}

	.number-property-container.widget-layout.infinite .number-field {
		flex: 1 1 auto;
		inline-size: auto;
		min-inline-size: 4rem;
	}

	.number-property-container.widget-layout.compact.infinite .number-field {
		flex: 0 0 auto;
		inline-size: clamp(3.2rem, 38%, 4.1rem);
		min-inline-size: 3.2rem;
	}

	.number-property-container.widget-layout .number-field-shell {
		flex: 1 1 auto;
		min-inline-size: 0;
	}

	.number-property-container.widget-layout .number-field.inline-labeled {
		flex: 1 1 auto;
		inline-size: auto;
		min-inline-size: 0;
		max-width: none;
		margin-left: 0;
		border: none;
		background: transparent;
		padding: 0;
		outline: none;
	}

	input::-webkit-outer-spin-button,
	input::-webkit-inner-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}
</style>
