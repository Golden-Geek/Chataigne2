<script lang="ts">
	import { type Color, ColorUtil } from './Color.svelte';
	import { slide } from 'svelte/transition';

	let {
		color = $bindable(),
		onchange = undefined,
		onStartEdit = undefined,
		onEndEdit = undefined,
		previewIsSwitch = false,
		forceExpanded = false,
		showHex = true,
		showRgbaFields = true,
		layoutMode = 'default'
	} = $props();

	let isSwitch = $derived(previewIsSwitch);
	let userPreviewOnly = $state(true);
	let previewOnly = $derived(forceExpanded ? false : isSwitch ? userPreviewOnly : true);

	// --- State ---
	// Interaction Mode: 0 = Sat/Val (Standard), 1 = Hue/Val, 2 = Hue/Sat
	let mode = $state(0);
	let showInputs = $state(true);
	let dragging = $state(false);
	let dragTarget = $state(null as 'area' | 'slider' | 'alpha' | null);
	let dragAreaX = $state(0);
	let dragAreaY = $state(0);
	let dragSliderVal = $state(0);
	let animTime = 200;

	// --- Derived State for UI (single source of truth: `color`) ---
	let current = $derived(ColorUtil.fromAny(color));
	let hsv = $derived(ColorUtil.toHSV(current));

	// In HSV, hue is undefined when the color is achromatic (black/grey/white).
	// `ColorUtil.toHSV()` returns h=0 in those cases, which makes the cursor/slider jump.
	// Preserve the last meaningful hue (and preserve saturation while value is ~0) so the UI stays stable.
	const HSV_EPS = 0.0001;
	let lastHue = $state(0);
	let lastSat = $state(0);

	let rawHue = $derived(hue01(hsv.h));
	let rawS = $derived(clamp01(hsv.s));
	let rawV = $derived(clamp01(hsv.v));

	let h = $derived(rawS > HSV_EPS && rawV > HSV_EPS ? rawHue : lastHue);
	let s = $derived(rawS);
	let v = $derived(rawV);
	let a = $derived(clamp01(current.a));
	let solid = $derived({ ...current, a: 1 } as Color);
	let hex = $derived(ColorUtil.toHex(solid, false));
	let hexInput = $derived(hex.startsWith('#') ? hex.slice(1) : hex);

	// DOM Elements for bounding rect calculations
	let areaRef = $state(null as HTMLDivElement | null);
	let sliderRef = $state(null as HTMLDivElement | null);
	let alphaRef = $state(null as HTMLDivElement | null);

	function clamp01(n: number): number {
		return Math.max(0, Math.min(1, n));
	}

	function hue01(n: number): number {
		const x = n % 1;
		return x < 0 ? x + 1 : x;
	}

	function colorsCloseEnough(left: Color, right: Color, eps = 0.001): boolean {
		return (
			Math.abs(left.r - right.r) <= eps &&
			Math.abs(left.g - right.g) <= eps &&
			Math.abs(left.b - right.b) <= eps &&
			Math.abs(left.a - right.a) <= eps
		);
	}

	function normalizeColor(c: Color): Color {
		return {
			r: Number(clamp01(c.r).toFixed(4)),
			g: Number(clamp01(c.g).toFixed(4)),
			b: Number(clamp01(c.b).toFixed(4)),
			a: Number(clamp01(c.a).toFixed(4))
		};
	}

	function setColor(next: Color): void {
		const prev = ColorUtil.fromAny(color);
		const normalized = normalizeColor(next);
		if (colorsCloseEnough(prev, normalized)) {
			return;
		}

		const nextHSV = ColorUtil.toHSV(normalized);
		const nextHue = hue01(nextHSV.h);
		const nextS = clamp01(nextHSV.s);
		const nextV = clamp01(nextHSV.v);
		if (nextS > HSV_EPS && nextV > HSV_EPS) lastHue = nextHue;
		if (nextV > HSV_EPS) lastSat = nextS;

		if (onchange === undefined) {
			color = normalized;
			return;
		}

		onchange && onchange(normalized);
	}

	function notifyStartEdit(): void {
		onStartEdit && onStartEdit();
	}

	function notifyEndEdit(): void {
		onEndEdit && onEndEdit();
	}

	// Dynamic Styles for the 2D Area
	let areaBackground = $derived.by(() => {
		if (mode === 0) return `hsl(${h * 360}, 100%, 50%)`; // Background is Hue
		if (mode === 1) return `hsl(0, ${s * 100}%, 50%)`; // Background is Saturation (Red base)
		if (mode === 2)
			return `rgb(${Math.round(v * 255)},${Math.round(v * 255)},${Math.round(v * 255)})`; // Background is Value (Grey)
	});

	let areaOverlay1 = $derived.by(() => {
		if (mode === 0) return `linear-gradient(to right, #fff, transparent)`; // Saturation (Left->Right)
		if (mode === 1)
			return `linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)`; // Hue
		if (mode === 2)
			return `linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)`; // Hue
	});

	let areaOverlay2 = $derived.by(() => {
		if (mode === 0) return `linear-gradient(to top, #000, transparent)`; // Value (Bottom->Top)
		if (mode === 1) return `linear-gradient(to top, #000, transparent)`; // Value
		if (mode === 2) return `linear-gradient(to top, transparent, #808080)`; // Saturation (Top=Grey/LowSat, Bottom=Color)?
		// Actually for Mode 2 (Hue/Sat), Y usually maps Saturation.
		// Let's do: Top (Sat 1), Bottom (Sat 0) for intuitive "down is washed out" or vice versa.
		// Standard is usually Bottom=Black? No, in HS mode, Value is the slider.
		return `linear-gradient(to top, rgba(128,128,128,1), transparent)`;
	});

	// Slider Backgrounds
	let sliderBackground = $derived.by(() => {
		if (mode === 0)
			return `linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)`; // Hue
		if (mode === 1) return `linear-gradient(to right, #fff, #f00)`; // Saturation (approx visual)
		if (mode === 2) return `linear-gradient(to right, #000, #fff)`; // Value
	});

	// --- Logic ---

	// Position calculations based on Mode
	let cursorX = $derived.by(() => {
		if (dragTarget === 'area') return dragAreaX * 100;
		if (mode === 0) return (rawV <= HSV_EPS ? lastSat : s) * 100;
		if (mode === 1) return h * 100;
		if (mode === 2) return h * 100;
		return 0;
	});

	let cursorY = $derived.by(() => {
		if (dragTarget === 'area') return dragAreaY * 100;
		if (mode === 0) return (1 - v) * 100;
		if (mode === 1) return (1 - v) * 100;
		if (mode === 2) return (rawV <= HSV_EPS ? lastSat : s) * 100;
		return 0;
	});

	let sliderPos = $derived.by(() => {
		if (dragTarget === 'slider') return dragSliderVal * 100;
		if (mode === 0) return h * 100;
		if (mode === 1) return s * 100;
		if (mode === 2) return v * 100;
		return 0;
	});

	// --- Input Handlers ---

	function handleArea(e: PointerEvent, isDown = false) {
		if (!isDown && e.buttons !== 1) return;
		const rect = areaRef!.getBoundingClientRect();
		const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));

		dragAreaX = x;
		dragAreaY = y;

		if (mode === 0) {
			setColor(ColorUtil.fromHSV(h, x, 1 - y, a));
		} else if (mode === 1) {
			lastHue = x;
			setColor(ColorUtil.fromHSV(x, s, 1 - y, a));
		} else if (mode === 2) {
			lastHue = x;
			lastSat = y;
			setColor(ColorUtil.fromHSV(x, y, v, a));
		}
	}

	function handleSlider(e: PointerEvent, isDown = false) {
		if (!isDown && e.buttons !== 1) return;
		const rect = sliderRef!.getBoundingClientRect();
		const val = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));

		dragSliderVal = val;

		if (mode === 0) {
			lastHue = val;
			setColor(ColorUtil.fromHSV(val, s, v, a));
		} else if (mode === 1) setColor(ColorUtil.fromHSV(h, val, v, a));
		else if (mode === 2) setColor(ColorUtil.fromHSV(h, s, val, a));
	}

	function handleAlpha(e: PointerEvent, isDown = false) {
		if (!isDown && e.buttons !== 1) return;
		const rect = alphaRef!.getBoundingClientRect();
		const nextA = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		setColor(ColorUtil.fromHSV(h, s, v, nextA));
	}

	function handleHexInput(e: Event) {
		const val = (e.target as HTMLInputElement).value.replace('#', '').trim();
		if (!(val.length === 6 || val.length === 8)) return;

		const next = ColorUtil.fromHex(val);
		if (val.length === 6) next.a = a;
		setColor(next);
	}

	function updateSingleRGB(channel: 'r' | 'g' | 'b', val: number): void {
		const base = current;
		setColor({
			r: channel === 'r' ? clamp01(val) : base.r,
			g: channel === 'g' ? clamp01(val) : base.g,
			b: channel === 'b' ? clamp01(val) : base.b,
			a: base.a
		});
	}
</script>

<div
	class="cp-container"
	class:expanded={!previewOnly}
	class:widget-layout={layoutMode === 'widget'}>
	<div class="cp-header">
		{#if !previewOnly}
			<div class="cp-modes" transition:slide={{ duration: animTime }}>
				<button class:active={mode === 0} onclick={() => (mode = 0)} title="Saturation-Brightness"
					>SB</button>
				<button class:active={mode === 1} onclick={() => (mode = 1)} title="Hue-Brightness"
					>HB</button>
				<button class:active={mode === 2} onclick={() => (mode = 2)} title="Hue-Saturation"
					>HS</button>
			</div>
		{/if}
		<button
			class="cp-preview"
			style="--color: {ColorUtil.toCSSRGBA(current)}; 
			--alpha: {a}; 
			--checker-color: rgba(220,220,220, {1 - a});"
			onclick={() => {
				if (previewIsSwitch) userPreviewOnly = !userPreviewOnly;
			}}
			title={previewOnly ? 'Switch to Full Picker' : 'Switch to Preview Only'}></button>
	</div>

	{#if !previewOnly}
		<div class="color-picker-content" transition:slide={{ duration: animTime }}>
			<div
				bind:this={areaRef}
				class="cp-area {dragging ? 'dragging' : ''}"
				style="background-color: {areaBackground};"
				onpointerdown={(e: PointerEvent) => {
					(e.target as HTMLElement).setPointerCapture(e.pointerId);
					dragTarget = 'area';
					handleArea(e, true);
					notifyStartEdit();
					dragging = true;
				}}
				onpointermove={(e: PointerEvent) => handleArea(e)}
				onpointerup={() => {
					dragging = false;
					dragTarget = null;
					notifyEndEdit();
				}}
				role="application">
				<div class="cp-overlay" style="background: {areaOverlay1}"></div>
				<div class="cp-overlay" style="background: {areaOverlay2}"></div>
				<div
					class="cp-cursor"
					style="left: {cursorX}%; top: {cursorY}%; background-color: {cursorY > 50 && mode === 0
						? 'white'
						: 'transparent'}; border-color: {cursorY > 50 && mode === 0 ? 'black' : 'white'};">
				</div>
			</div>

			<div class="cp-controls">
				<div
					bind:this={sliderRef}
					class="cp-slider-track"
					style="background: {sliderBackground}"
					onpointerdown={(e: PointerEvent) => {
						(e.target as HTMLElement).setPointerCapture(e.pointerId);
						dragTarget = 'slider';
						handleSlider(e, true);
						notifyStartEdit();
					}}
					onpointermove={(e) => handleSlider(e)}
					onpointerup={() => {
						dragTarget = null;
						notifyEndEdit();
					}}
					role="slider"
					aria-valuenow={mode === 0 ? h : mode === 1 ? s : v}
					tabindex="0">
					<div class="cp-thumb" style="left: {sliderPos}%"></div>
				</div>

				<div
					bind:this={alphaRef}
					class="cp-slider-track alpha-track"
					onpointerdown={(e: PointerEvent) => {
						(e.target as HTMLElement).setPointerCapture(e.pointerId);
						dragTarget = 'alpha';
						handleAlpha(e, true);
						notifyStartEdit();
					}}
					onpointermove={(e: PointerEvent) => handleAlpha(e)}
					onpointerup={() => {
						dragTarget = null;
						notifyEndEdit();
					}}
					role="slider"
					aria-valuenow={a}
					tabindex="0">
					<div
						class="cp-track-bg"
						style="background: linear-gradient(to right, transparent, {hex})">
					</div>
					<div class="cp-thumb" style="left: {a * 100}%"></div>
				</div>
			</div>

			<div class="cp-inputs">
				{#if showHex || showRgbaFields}
					<div class="cp-hex-row">
						{#if showHex}
							<span class="cp-label">#</span>
							<input
								type="text"
								value={hexInput}
								oninput={handleHexInput}
								onfocus={notifyStartEdit}
								onblur={notifyEndEdit}
								class="cp-input-hex" />
						{/if}
						{#if showRgbaFields}
							<button class="cp-toggle-btn" onclick={() => (showInputs = !showInputs)}>
								{showInputs ? 'Hide' : 'Edit'}
							</button>
						{/if}
					</div>
				{/if}

				{#if showInputs && showRgbaFields}
					<div class="cp-rgba-grid">
						<div class="cp-field">
							<label for="cp-r">R</label>
							<input
								id="cp-r"
								type="number"
								step="0.01"
								min="0"
								max="1"
								value={Number(solid.r.toFixed(2))}
								oninput={(e) => updateSingleRGB('r', +(e.target as HTMLInputElement).value)} />
						</div>
						<div class="cp-field">
							<label for="cp-g">G</label>
							<input
								id="cp-g"
								type="number"
								step="0.01"
								min="0"
								max="1"
								value={Number(solid.g.toFixed(2))}
								oninput={(e) => updateSingleRGB('g', +(e.target as HTMLInputElement).value)} />
						</div>
						<div class="cp-field">
							<label for="cp-b">B</label>
							<input
								id="cp-b"
								type="number"
								step="0.01"
								min="0"
								max="1"
								value={Number(solid.b.toFixed(2))}
								oninput={(e) => updateSingleRGB('b', +(e.target as HTMLInputElement).value)} />
						</div>
						<div class="cp-field">
							<label for="cp-a">A</label>
							<input
								id="cp-a"
								type="number"
								step="0.01"
								min="0"
								max="1"
								value={Number(a.toFixed(2))}
								oninput={(e) => {
									const nextA = +(e.target as HTMLInputElement).value;
									setColor({ ...current, a: clamp01(nextA) });
								}} />
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.cp-container {
		border-radius: 0.75rem;
		color: #e4e4e7; /* Zinc 200 */
		user-select: none;
		box-sizing: border-box;
		padding: 0;
		transition:
			padding 0.2s,
			background-color 0.2s,
			width 0.2s;
	}

	.cp-container.expanded {
		border: 0.0625rem solid #27272a;
		background-color: #18181b;
		padding: 0.3rem 0.7rem;
	}

	.cp-container.widget-layout {
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.color-picker-content {
		display: flex;
		flex-direction: column;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.cp-container.widget-layout.expanded {
		overflow: hidden;
	}

	/* Header */
	.cp-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		transition: margin-bottom 0.2s;
	}

	.cp-container.expanded .cp-header {
		margin-bottom: 0.5rem;
	}

	.cp-modes {
		display: flex;
		gap: 0.125rem;
		background: #27272a;
		padding: 0.125rem;
		border-radius: 0.375rem;
	}

	.cp-modes button {
		background: transparent;
		border: none;
		color: #a1a1aa;
		font-size: 0.625rem;
		font-weight: 700;
		padding: 0.25rem 0.5rem;
		cursor: pointer;
		border-radius: 0.25rem;
		transition: all 0.2s;
	}

	.cp-modes button.active {
		background: #3f3f46;
		color: white;
	}

	.cp-preview {
		width: 1.5rem;
		height: 1.5rem;
		border-radius: 50%;
		border: 0.125rem solid #3f3f46;
		box-shadow: inset 0 0 0 0.0625rem rgba(0, 0, 0, 0.2);
		background-color: var(--color);
		background-image:
			linear-gradient(45deg, var(--checker-color) 25%, transparent 25%),
			linear-gradient(-45deg, var(--checker-color) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--checker-color) 75%),
			linear-gradient(-45deg, transparent 75%, var(--checker-color) 75%);
		background-size: 0.5rem 0.5rem;
		background-position:
			0 0,
			0 0.25rem,
			0.25rem -0.25rem,
			-0.25rem 0;
	}

	/* 2D Area */
	.cp-area {
		width: 100%;
		height: 10rem;
		border-radius: 0.5rem;
		position: relative;
		overflow: hidden;
		cursor: crosshair;
		margin-bottom: 0.75rem;
		touch-action: none;
	}

	.cp-container.widget-layout .cp-area {
		flex: 1 1 auto;
		min-block-size: 8rem;
		margin-bottom: 0.5rem;
	}

	.cp-area.dragging {
		cursor: none;
	}

	.cp-overlay {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		pointer-events: none;
	}

	.cp-cursor {
		width: 0.75rem;
		height: 0.75rem;
		border: 0.125rem solid white;
		border-radius: 50%;
		position: absolute;
		transform: translate(-50%, -50%);
		box-shadow: 0 0.0625rem 0.1875rem rgba(0, 0, 0, 0.3);
		pointer-events: none;
	}

	/* Sliders */
	.cp-controls {
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
		margin-bottom: 0.75rem;
	}

	.cp-container.widget-layout .cp-controls {
		flex: 0 0 auto;
		margin-bottom: 0.5rem;
	}

	.cp-slider-track {
		height: 0.75rem;
		border-radius: 0.375rem;
		position: relative;
		cursor: pointer;
		touch-action: none;
	}

	.alpha-track {
		background-image:
			linear-gradient(45deg, #27272a 25%, transparent 25%),
			linear-gradient(-45deg, #27272a 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, #27272a 75%),
			linear-gradient(-45deg, transparent 75%, #27272a 75%);
		background-size: 0.5rem 0.5rem;
	}

	.cp-track-bg {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		border-radius: 0.375rem;
	}

	.cp-thumb {
		width: 0.875rem;
		height: 0.875rem;
		background: white;
		border-radius: 50%;
		position: absolute;
		top: 50%;
		transform: translate(-50%, -50%);
		box-shadow: 0 0.0625rem 0.125rem rgba(0, 0, 0, 0.4);
		pointer-events: none;
	}

	/* Inputs */
	.cp-inputs {
		background: #27272a;
		border-radius: 0.5rem;
		box-sizing: border-box;
		padding: 0.5rem;
	}

	.cp-hex-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.cp-label {
		color: #71717a;
		font-size: 0.75rem;
		font-weight: bold;
	}

	.cp-input-hex {
		background: transparent;
		border: none;
		color: #fff;
		font-family: monospace;
		font-size: 0.8125rem;
		width: 100%;
		outline: none;
		text-transform: uppercase;
	}

	.cp-toggle-btn {
		background: #3f3f46;
		border: none;
		color: #d4d4d8;
		font-size: 0.625rem;
		padding: 0.125rem 0.375rem;
		border-radius: 0.25rem;
		cursor: pointer;
	}

	.cp-rgba-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr 1fr;
		gap: 0.375rem;
		margin-top: 0.5rem;
		border-top: 0.0625rem solid #3f3f46;
		padding-top: 0.5rem;
	}

	.cp-field {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.125rem;
	}

	.cp-field label {
		font-size: 0.5625rem;
		color: #71717a;
		font-weight: 700;
	}

	.cp-field input {
		width: 100%;
		background: #18181b;
		border: 0.0625rem solid #3f3f46;
		border-radius: 0.25rem;
		color: #e4e4e7;
		font-size: 0.6875rem;
		text-align: center;
		padding: 2px 0;
		outline: none;
	}

	.cp-field input:focus {
		border-color: #52525b;
	}

	/* Remove arrows from number input */
	input::-webkit-outer-spin-button,
	input::-webkit-inner-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}
</style>
