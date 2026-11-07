<script lang="ts">
	import { onDestroy } from 'svelte';
	import { constrainSliderValue, sliderDragDelta } from './slider-value';
	let {
		value = $bindable(),
		min = undefined,
		max = undefined,
		step = 0,
		stepBase = undefined,
		dragStep = undefined,
		sensitivity = 1,
		orientation = 'horizontal',
		disabled = false,
		readOnly = false,
		onStartEdit = null,
		onEndEdit = null,
		onValueChange = null,
		bgColor = 'var(--gc-color-slider-bg)',
		fgColor = 'var(--gc-color-slider-fg)',
		label = '',
		accessibleLabel = 'Adjust value',
		showValue = false,
		forceInfinite = false
	} = $props();

	let infiniteMode = $derived(min === undefined || max === undefined || forceInfinite);
	let sliderDiv = $state(null as HTMLDivElement | null);

	let isHorizontal = $derived(orientation === 'horizontal');
	let isDragging = $state(false);
	let initialValue = 0;
	let rawValue = 0;
	let lastPosition = 0;
	let pointer: number | null = null;
	let owner: HTMLElement | null = null;

	let midZero = $derived(min < 0 && max > 0);
	let relativeValue = $derived(
		midZero ? (value >= 0 ? value / max : value / min) : (value - min) / (max - min)
	);
	let relativeZero = $derived(midZero ? Math.abs(min) / (max - min) : 0);

	let targetLeft = $derived(
		!midZero ? 0 : value >= 0 ? relativeZero : relativeZero * (1 - relativeValue)
	);

	let targetRight = $derived(
		1 -
			(!midZero
				? relativeValue
				: value >= 0
					? relativeZero + relativeValue * (1 - relativeZero)
					: relativeZero)
	);

	function publish(candidate: number) {
		const next = constrainSliderValue(candidate, { min, max, step, stepBase });
		if (!Number.isFinite(next) || next === value) return;
		value = next;
		onValueChange?.(next);
	}
	function begin() {
		isDragging = true;
		initialValue = rawValue = value;
		onStartEdit?.();
	}
	function finish(cancel = false) {
		if (!isDragging) return;
		isDragging = false;
		const element = owner,
			id = pointer;
		owner = null;
		pointer = null;
		if (id !== null && element?.hasPointerCapture(id)) element.releasePointerCapture(id);
		if (cancel) publish(initialValue);
		onEndEdit?.();
	}
	function startDrag(event: PointerEvent) {
		if (disabled || readOnly || event.button !== 0 || isDragging) return;
		event.preventDefault();
		event.stopPropagation();
		owner = event.currentTarget as HTMLElement;
		owner.focus();
		pointer = event.pointerId;
		lastPosition = isHorizontal ? event.clientX : event.clientY;
		begin();
		owner.setPointerCapture(pointer);
	}
	function dragUpdate(event: PointerEvent) {
		if (!isDragging || event.pointerId !== pointer) return;
		const position = isHorizontal ? event.clientX : event.clientY;
		rawValue += sliderDragDelta(position - lastPosition, {
			min,
			max,
			step,
			dragStep,
			width: sliderDiv?.getBoundingClientRect().width ?? 1,
			relative: infiniteMode,
			sensitivity: sensitivity * (event.altKey ? 0.1 : event.shiftKey ? 10 : 1)
		});
		lastPosition = position;
		// Clamp the accumulator at bounds while retaining fractional steps between moves.
		rawValue = Math.max(min ?? -Infinity, Math.min(max ?? Infinity, rawValue));
		publish(rawValue);
	}
	function keydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && isDragging) {
			event.preventDefault();
			event.stopPropagation();
			finish(true);
			return;
		}
		if (disabled || readOnly || pointer !== null) return;
		const direction = ['ArrowRight', 'ArrowUp', 'PageUp'].includes(event.key)
			? 1
			: ['ArrowLeft', 'ArrowDown', 'PageDown'].includes(event.key)
				? -1
				: 0;
		if (!direction && !['Home', 'End'].includes(event.key)) return;
		event.preventDefault();
		event.stopPropagation();
		if (!isDragging) begin();
		const unit = step || dragStep || (infiniteMode ? 0.1 : (max - min) / 100);
		const amount =
			unit * (event.key.startsWith('Page') || event.shiftKey ? 10 : event.altKey ? 0.1 : 1);
		publish(
			event.key === 'Home'
				? (min ?? value)
				: event.key === 'End'
					? (max ?? value)
					: value + direction * amount
		);
	}
	function pointerFinish(event: PointerEvent, cancel = false) {
		if (event.pointerId === pointer) finish(cancel);
	}
	$effect(() => {
		if (disabled || readOnly) finish();
	});
	onDestroy(() => finish());
</script>

{#if infiniteMode}
	<button
		class="button infinite-slider {isDragging ? 'dragging' : ''}"
		role="slider"
		aria-label={accessibleLabel}
		aria-valuenow={value}
		aria-valuemin={min}
		aria-valuemax={max}
		aria-orientation={isHorizontal ? 'horizontal' : 'vertical'}
		disabled={disabled || readOnly}
		onpointerdown={startDrag}
		onpointermove={dragUpdate}
		onpointerup={(event) => pointerFinish(event)}
		onpointercancel={(event) => pointerFinish(event, true)}
		onlostpointercapture={(event) => pointerFinish(event, true)}
		onkeydown={keydown}
		onkeyup={() => {
			if (pointer === null) finish();
		}}
		onblur={() => finish()}
		onclick={(event) => event.stopPropagation()}>
		↔
	</button>
{:else}
	<div
		class="slider"
		class:disabled
		class:readonly={readOnly}
		bind:this={sliderDiv}
		onpointerdown={startDrag}
		onpointermove={dragUpdate}
		onpointerup={(event) => pointerFinish(event)}
		onpointercancel={(event) => pointerFinish(event, true)}
		onlostpointercapture={(event) => pointerFinish(event, true)}
		onkeydown={keydown}
		onkeyup={() => {
			if (pointer === null) finish();
		}}
		onblur={() => finish()}
		onclick={(event) => event.stopPropagation()}
		role="slider"
		aria-label={accessibleLabel}
		aria-valuenow={value}
		aria-valuemin={min}
		aria-valuemax={max}
		aria-disabled={disabled || readOnly}
		aria-orientation={isHorizontal ? 'horizontal' : 'vertical'}
		tabindex={disabled || readOnly ? -1 : 0}
		style="--bg-color: {bgColor}; --fg-color: {fgColor};">
		<div
			class="slider-foreground {midZero ? 'mid-zero' : ''}"
			style="--value: {(value - min) / (max - min)};  --left:{targetLeft *
				100}%; --right:{targetRight * 100}%">
		</div>

		{#if label || showValue}
			<div class="slider-label">
				{label}
				{label && showValue ? ' : ' : ''}
				{showValue ? value.toFixed(3) : ''}
			</div>
		{/if}
	</div>
{/if}

<style>
	.slider {
		touch-action: none;
		min-width: 0;
		min-height: 0;
		display: block;
		flex: 1 1 auto;
		align-self: stretch;
		position: relative;
		height: 100%;
		width: 100%;
		user-select: none;
		border-radius: 0.5rem;
		overflow: hidden;
		background: var(--bg-color);
		border: none;
		transition: filter 0.2s;
	}

	.slider:hover {
		filter: brightness(120%);
	}

	.slider.disabled,
	.slider.readonly {
		pointer-events: none;
	}

	.slider.disabled {
		filter: grayscale(60%);
	}

	.slider-label {
		position: absolute;
		width: 100%;
		text-align: center;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		font-size: 0.8rem;
		font-weight: bold;
		color: rgba(from var(--text-color) r g b / 70%);
		pointer-events: none;
	}

	.slider-foreground {
		position: absolute;
		padding: 0;
		margin: 0;
		left: var(--left);
		right: var(--right);
		top: 0;
		height: 100%;
		/* border-radius: 12px 0 0 12px; */
		background: var(--fg-color);
		/* width: calc(var(--value) * 100%); */
		pointer-events: none;
	}

	.readonly .slider-foreground {
		background: var(--gc-color-readonly);
	}

	.slider-foreground.mid-zero {
		min-width: 1px;
	}

	.infinite-slider {
		touch-action: none;
		background: none;
		border: none;
		font-size: 0.6rem;
		cursor: ew-resize;
		color: rgb(from var(--gc-color-text) r g b / 50%);
		transition: color 0.2s;
	}

	.infinite-slider:hover {
		color: var(--gc-color-text);
	}

	.infinite-slider:hover,
	.infinite-slider.dragging {
		background-color: rgba(from var(--gc-color-slider-bg) r g b / 10%);
	}
</style>
