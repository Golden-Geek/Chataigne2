<script lang="ts">
	import { onDestroy } from 'svelte';
	import { appState } from '../../../store/workbench.svelte';
	import EnableButton from '../../common/EnableButton.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../store/ui-intents';
	import type { UiNodeDto } from '../../../types';
	import {
		clampWidgetMaxDecimals,
		getEffectiveWidgetScalarRange,
		getWidgetBoolOption,
		getWidgetIntOption
	} from './dashboard-node-widget-options';

	const START_ANGLE = -135;
	const END_ANGLE = 135;
	const FULL_SWEEP = END_ANGLE - START_ANGLE;
	const EPSILON = 1e-9;

	let {
		targetNode,
		widgetNode = null,
		insideLabel = null,
		showEnableButton = true
	} = $props<{
		targetNode: UiNodeDto;
		widgetNode?: UiNodeDto | null;
		insideLabel?: string | null;
		showEnableButton?: boolean;
		includeChildren?: boolean;
		editMode?: boolean;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveWidgetNode = $derived(
		widgetNode ? (graph?.nodesById.get(widgetNode.node_id) ?? widgetNode) : null
	);
	let liveTargetNode = $derived(graph?.nodesById.get(targetNode.node_id) ?? targetNode);
	let param = $derived(liveTargetNode.data.kind === 'parameter' ? liveTargetNode.data.param : null);
	let enabled = $derived(liveTargetNode.meta.enabled);
	let readOnly = $derived(Boolean(param?.read_only));
	let kind = $derived(param?.value.kind ?? null);
	let isNumeric = $derived(kind === 'int' || kind === 'float');
	let isInteger = $derived(kind === 'int');
	let value = $derived(
		param && (param.value.kind === 'int' || param.value.kind === 'float') ? param.value.value : 0
	);
	let range = $derived(getEffectiveWidgetScalarRange(graph, liveWidgetNode, liveTargetNode));
	let min = $derived(range?.kind === 'uniform' ? range.min : undefined);
	let max = $derived(range?.kind === 'uniform' ? range.max : undefined);
	let hasUsableRange = $derived(
		typeof min === 'number' &&
			typeof max === 'number' &&
			Number.isFinite(min) &&
			Number.isFinite(max) &&
			max > min
	);
	let step = $derived(param?.constraints.step);
	let stepBase = $derived(param?.constraints.step_base);

	let draftValue = $state(0);
	let isEditing = $state(false);
	let knobElement = $state(null as HTMLDivElement | null);
	let valueInput = $state(null as HTMLInputElement | null);
	let pointerId = $state<number | null>(null);
	let editSession = createUiEditSession('Edit rotary number', 'param-rotary-number');

	const polarPoint = (angleDeg: number, radius: number): [number, number] => {
		const radians = ((angleDeg - 90) * Math.PI) / 180;
		return [50 + Math.cos(radians) * radius, 50 + Math.sin(radians) * radius];
	};

	const describeArc = (startAngleDeg: number, endAngleDeg: number, radius: number): string => {
		const [startX, startY] = polarPoint(startAngleDeg, radius);
		const [endX, endY] = polarPoint(endAngleDeg, radius);
		const largeArc = endAngleDeg - startAngleDeg > 180 ? 1 : 0;
		return `M ${startX} ${startY} A ${radius} ${radius} 0 ${largeArc} 1 ${endX} ${endY}`;
	};

	$effect(() => {
		if (isEditing) {
			return;
		}
		draftValue = value;
	});

	onDestroy(() => {
		void editSession.end();
	});

	let showValueField = $derived(
		getWidgetBoolOption(graph, liveWidgetNode, 'rotary_show_value_field', true)
	);
	let fractionDigits = $derived(
		isInteger
			? 0
			: clampWidgetMaxDecimals(
					getWidgetIntOption(graph, liveWidgetNode, 'rotary_max_decimals', 3),
					3
				)
	);
	let centeredFill = $derived(
		hasUsableRange &&
			getWidgetBoolOption(graph, liveWidgetNode, 'rotary_centered_fill', true) &&
			typeof min === 'number' &&
			typeof max === 'number' &&
			min < 0 &&
			max > 0
	);
	let fieldStep = $derived(
		isInteger ? 1 : fractionDigits <= 0 ? 1 : Number(`1e-${fractionDigits}`)
	);
	let insideLabelText = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsEnableButton = $derived(showEnableButton && liveTargetNode.meta.can_be_disabled);
	let displayedDraftValue = $derived.by(() => {
		if (typeof min === 'number' && draftValue < min) {
			return min;
		}
		if (typeof max === 'number' && draftValue > max) {
			return max;
		}
		return draftValue;
	});
	let normalizedValue = $derived.by(() => {
		if (!hasUsableRange || typeof min !== 'number' || typeof max !== 'number') {
			return 0;
		}
		return Math.min(1, Math.max(0, (displayedDraftValue - min) / (max - min)));
	});
	let currentAngle = $derived(START_ANGLE + normalizedValue * FULL_SWEEP);
	let centerAngle = $derived(START_ANGLE + FULL_SWEEP / 2);
	let trackPath = $derived(describeArc(START_ANGLE, END_ANGLE, 40));
	let valuePath = $derived.by(() => {
		if (!hasUsableRange) {
			return '';
		}
		if (centeredFill) {
			if (Math.abs(currentAngle - centerAngle) <= EPSILON) {
				return '';
			}
			const startAngle = Math.min(centerAngle, currentAngle);
			const endAngle = Math.max(centerAngle, currentAngle);
			return describeArc(startAngle, endAngle, 40);
		}
		if (normalizedValue <= 0) {
			return '';
		}
		return describeArc(START_ANGLE, currentAngle, 40);
	});
	let indicatorTransform = $derived(`rotate(${currentAngle} 50 50)`);
	let formattedDraftValue = $derived(draftValue.toFixed(fractionDigits));
	let formattedValue = $derived(value.toFixed(fractionDigits));

	const normalizeValue = (candidate: number): number | null => {
		if (!Number.isFinite(candidate)) {
			return null;
		}
		if (!hasUsableRange || typeof min !== 'number' || typeof max !== 'number') {
			return null;
		}

		let nextValue = candidate;
		const policy = param?.constraints.policy ?? 'ClampAdapt';
		if (nextValue < min) {
			if (policy === 'ClampAdapt') {
				nextValue = min;
			} else {
				return null;
			}
		}
		if (nextValue > max) {
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
			const base = stepBase ?? min;
			const scaled = (nextValue - base) / step;
			const nearest = Math.round(scaled);
			if (policy === 'ClampAdapt') {
				nextValue = base + nearest * step;
			} else if (Math.abs(scaled - nearest) > EPSILON) {
				return null;
			}
		}
		if (policy === 'ClampAdapt') {
			nextValue = Math.min(max, Math.max(min, nextValue));
		}
		if (isInteger) {
			const rounded = Math.round(nextValue);
			if (policy === 'Reject' && Math.abs(nextValue - rounded) > EPSILON) {
				return null;
			}
			nextValue = rounded;
		}
		return nextValue;
	};

	const startEdit = (): void => {
		if (!isNumeric || readOnly || !enabled || !hasUsableRange) {
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
		pointerId = null;
		void editSession.end();
	};

	const commitValue = (candidate: number): void => {
		if (!isNumeric || !param || readOnly || !enabled || !hasUsableRange) {
			return;
		}
		const nextValue = normalizeValue(candidate);
		if (nextValue === null) {
			return;
		}
		draftValue = nextValue;
		if (Math.abs(nextValue - value) <= EPSILON) {
			return;
		}
		void sendSetParamIntent(
			liveTargetNode.node_id,
			{ kind: param.value.kind, value: nextValue },
			param.event_behaviour
		);
	};

	const updateFromPointer = (event: PointerEvent): void => {
		if (!knobElement || !hasUsableRange || typeof min !== 'number' || typeof max !== 'number') {
			return;
		}
		const rect = knobElement.getBoundingClientRect();
		const centerX = rect.left + rect.width / 2;
		const centerY = rect.top + rect.height / 2;
		const dx = event.clientX - centerX;
		const dy = event.clientY - centerY;
		let angle = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
		if (angle < -180) {
			angle += 360;
		}
		if (angle > 180) {
			angle -= 360;
		}
		const clampedAngle = Math.min(END_ANGLE, Math.max(START_ANGLE, angle));
		const nextValue = min + ((clampedAngle - START_ANGLE) / FULL_SWEEP) * (max - min);
		commitValue(nextValue);
	};

	const commitFieldValue = (): void => {
		if (!valueInput) {
			return;
		}
		const parsed = Number(valueInput.value);
		if (Number.isFinite(parsed)) {
			commitValue(parsed);
			return;
		}
		valueInput.value = formattedValue;
	};

	const indicatorAccentClass = $derived(centeredFill ? 'centered' : 'edge');
</script>

<div class="dashboard-node-widget-number-rotary">
	{#if showsEnableButton}
		<div class="dashboard-node-widget-enable">
			<EnableButton node={liveTargetNode} />
		</div>
	{/if}
	{#if !isNumeric}
		<div class="dashboard-node-widget-mode-empty">
			Rotary mode only applies to int and float parameters.
		</div>
	{:else if !hasUsableRange}
		<div class="dashboard-node-widget-mode-empty">
			Rotary mode requires a bounded numeric range.
		</div>
	{:else}
		<div class="dashboard-node-widget-number-rotary-shell">
			{#if insideLabelText.length > 0}
				<div class="dashboard-node-widget-number-rotary-label">{insideLabelText}</div>
			{/if}
			<div class="dashboard-node-widget-number-rotary-body">
				<div
					class="dashboard-node-widget-number-rotary-knob"
					class:disabled={!enabled || readOnly}
					bind:this={knobElement}
					role="slider"
					tabindex={enabled && !readOnly ? 0 : -1}
					aria-valuemin={typeof min === 'number' ? min : undefined}
					aria-valuemax={typeof max === 'number' ? max : undefined}
					aria-valuenow={displayedDraftValue}
					onpointerdown={(event) => {
						if (!knobElement || readOnly || !enabled) {
							return;
						}
						pointerId = event.pointerId;
						knobElement.setPointerCapture(event.pointerId);
						startEdit();
						updateFromPointer(event);
					}}
					onpointermove={(event) => {
						if (pointerId !== event.pointerId) {
							return;
						}
						updateFromPointer(event);
					}}
					onpointerup={(event) => {
						if (pointerId !== event.pointerId) {
							return;
						}
						knobElement?.releasePointerCapture(event.pointerId);
						endEdit();
					}}
					onpointercancel={(event) => {
						if (pointerId !== event.pointerId) {
							return;
						}
						knobElement?.releasePointerCapture(event.pointerId);
						endEdit();
					}}
					onkeydown={(event) => {
						if (readOnly || !enabled || !hasUsableRange || step === undefined) {
							return;
						}
						if (
							event.key !== 'ArrowLeft' &&
							event.key !== 'ArrowDown' &&
							event.key !== 'ArrowRight' &&
							event.key !== 'ArrowUp'
						) {
							return;
						}
						event.preventDefault();
						const direction = event.key === 'ArrowLeft' || event.key === 'ArrowDown' ? -1 : 1;
						const delta =
							step > 0
								? step
								: typeof max === 'number' && typeof min === 'number'
									? (max - min) / 100
									: 0;
						commitValue(draftValue + direction * delta);
					}}>
					<svg viewBox="0 0 100 100" aria-hidden="true">
						<path class="track" d={trackPath}></path>
						{#if valuePath.length > 0}
							<path class={`value ${indicatorAccentClass}`} d={valuePath}></path>
						{/if}
						<circle class="body" cx="50" cy="50" r="28"></circle>
						<line class="indicator" x1="50" y1="50" x2="50" y2="18" transform={indicatorTransform}
						></line>
					</svg>
				</div>
				{#if showValueField}
					<input
						bind:this={valueInput}
						class="dashboard-node-widget-number-rotary-field"
						type="number"
						step={fieldStep}
						disabled={!enabled}
						class:readonly={readOnly}
						value={formattedDraftValue}
						onfocus={startEdit}
						onblur={() => {
							commitFieldValue();
							endEdit();
						}}
						onkeydown={(event) => {
							if (event.key === 'Enter') {
								valueInput?.blur();
							} else if (event.key === 'Escape') {
								if (valueInput) {
									valueInput.value = formattedValue;
									valueInput.blur();
								}
							}
						}} />
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.dashboard-node-widget-number-rotary,
	.dashboard-node-widget-number-rotary-shell,
	.dashboard-node-widget-number-rotary-body {
		display: flex;
		flex: 1 1 auto;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-number-rotary {
		position: relative;
	}

	.dashboard-node-widget-number-rotary-shell,
	.dashboard-node-widget-number-rotary-body {
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.75rem;
		position: relative;
	}

	.dashboard-node-widget-enable {
		position: absolute;
		inset-block-start: 0.75rem;
		inset-inline-end: 0.75rem;
		z-index: 1;
		display: flex;
		align-items: center;
	}

	.dashboard-node-widget-number-rotary-label {
		max-inline-size: 100%;
		padding-inline: 0.75rem;
		text-align: center;
		font-size: 0.78rem;
		line-height: 1.2;
	}

	.dashboard-node-widget-number-rotary-knob {
		inline-size: min(100%, 12rem);
		aspect-ratio: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		touch-action: none;
	}

	.dashboard-node-widget-number-rotary-knob.disabled {
		cursor: default;
		opacity: 0.6;
	}

	.dashboard-node-widget-number-rotary-knob svg {
		inline-size: 100%;
		block-size: 100%;
		overflow: visible;
	}

	.dashboard-node-widget-number-rotary-knob .track,
	.dashboard-node-widget-number-rotary-knob .value {
		fill: none;
		stroke-linecap: round;
		stroke-width: 0.5rem;
	}

	.dashboard-node-widget-number-rotary-knob .track {
		stroke: rgb(from var(--gc-color-slider-bg) r g b / 42%);
	}

	.dashboard-node-widget-number-rotary-knob .value {
		stroke: var(--gc-color-slider-fg);
	}

	.dashboard-node-widget-number-rotary-knob .value.centered {
		stroke: var(--gc-color-accent, var(--gc-color-slider-fg));
	}

	.dashboard-node-widget-number-rotary-knob .body {
		fill: rgb(from var(--gc-color-slider-bg) r g b / 14%);
		stroke: rgb(from var(--gc-color-slider-bg) r g b / 58%);
		stroke-width: 0.15rem;
	}

	.dashboard-node-widget-number-rotary-knob .indicator {
		stroke: var(--gc-color-text, currentColor);
		stroke-linecap: round;
		stroke-width: 0.3rem;
	}

	.dashboard-node-widget-number-rotary-field {
		inline-size: min(100%, 8em);
		padding: 0.45em 0.6em;
		border: 0.08rem solid rgb(from var(--gc-color-slider-bg) r g b / 72%);
		border-radius: 0.6rem;
		background: rgb(from var(--gc-color-slider-bg) r g b / 12%);
		text-align: center;
	}

	.dashboard-node-widget-number-rotary-field.readonly {
		opacity: 0.75;
	}

	.dashboard-node-widget-mode-empty {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: center;
		padding: 0.75rem;
		text-align: center;
		font-size: 0.72rem;
		opacity: 0.72;
	}
</style>
