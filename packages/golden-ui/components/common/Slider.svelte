<script lang="ts">
	let {
		value = $bindable(),
		min = undefined,
		max = undefined,
		step = 0,
		stepBase = 0,
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
		showValue = false,
		forceInfinite = false
	} = $props();

	let infiniteMode = $derived(min === undefined || max === undefined || forceInfinite);
	let sliderDiv = $state(null as HTMLDivElement | null);

	let valueAtDown = $state(0);
	let mouseAtDown = $state(0);

	let isHorizontal = $derived(orientation === 'horizontal');

	let isDragging = $derived(false);

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

	let pointerLockId = $state(null as number | null);

	async function startDrag(e: MouseEvent) {
		if (disabled || readOnly) {
			return;
		}
		isDragging = true;
		onStartEdit && onStartEdit(value);
		mouseAtDown = isHorizontal ? e.clientX : e.clientY;
		valueAtDown = value;

		document.addEventListener('mousemove', dragUpdate);
		document.addEventListener('mouseup', stopDrag);

		// Hide mouse pointer globally by adding a CSS class
		document.body.classList.add('slider-hide-cursor');

		// await document.body.requestPointerLock();
	}

	function dragUpdate(e: MouseEvent) {
		if (!isDragging) return;
		const delta = isHorizontal ? e.movementX : e.movementY;
		const range = max - min;
		const previousValue = value;
		let nextValue = value;

		let alteredSensitivity = e.altKey
			? sensitivity / 10
			: e.shiftKey
				? sensitivity * 10
				: sensitivity;

		let sliderWidth = infiniteMode ? 100 : sliderDiv!.getBoundingClientRect().width;
		if (step > 0) {
			const stepCount = range / step;
			const pixelsPerStep = sliderWidth / stepCount / alteredSensitivity;
			const stepsMoved = Math.round(delta / pixelsPerStep);
			nextValue = Math.min(max, Math.max(min, valueAtDown + stepsMoved * step));
		} else if (!infiniteMode) {
			const percentDelta = (delta / sliderWidth) * alteredSensitivity;
			nextValue = Math.min(max, Math.max(min, valueAtDown + percentDelta * range));
		} else //infinite mode, no range
		{
			const valueDelta = (delta * alteredSensitivity) / 10;
			nextValue = valueAtDown + valueDelta;
		}

		value = nextValue;
		valueAtDown = value;

		if (previousValue === nextValue) {
			return;
		}

		onValueChange && onValueChange(nextValue);
	}

	function stopDrag(e: MouseEvent) {
		if (!isDragging) return;
		isDragging = false;
		// Remove the global cursor-hiding class
		document.body.classList.remove('slider-hide-cursor');

		isDragging = false;
		onEndEdit && onEndEdit(value);

		// document.exitPointerLock();
	}
</script>

{#if infiniteMode}
	<button class="button infinite-slider {isDragging ? 'dragging' : ''}" onmousedown={startDrag}>
		{'⟷'}
	</button>
{:else}
	<div
		class="slider"
		class:disabled
		class:readonly={readOnly}
		bind:this={sliderDiv}
		onmousedown={startDrag}
		role="slider"
		aria-valuenow={value}
		tabindex="-1"
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

	/* Hide cursor globally when dragging */
	:global(.slider-hide-cursor) {
		cursor: none !important;
	}

	.infinite-slider {
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
