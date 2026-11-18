<script lang="ts">
	import { onDestroy } from 'svelte';
	import ColorPicker from './ColorPicker.svelte';
	import { appState } from '../../store/workbench.svelte';
	import { createUiEditSession } from '../../store/ui-intents';
	import { LatestEditGesture, createLatestEditGestureState } from '../../store/latest-edit-gesture';
	import type { UiColorDto } from '../../types';
	import type { NodeColorTarget } from './node-color-menu';
	import { computePointAnchoredMenuPlacement, getMainViewportBounds } from './floating-panel';

	let target = $state.raw<NodeColorTarget | null>(null);
	let point = $state({ x: 0, y: 0 });
	let panel = $state<HTMLDivElement>();
	let draft = $state<UiColorDto>({ r: 1, g: 1, b: 1, a: 1 });
	const gestureState = $state(createLatestEditGestureState());
	const gesture = new LatestEditGesture(() => appState.session, gestureState, {
		group: (host) => createUiEditSession('Set color', 'node-color', host),
		move: async (
			_host,
			reference: { target: NodeColorTarget },
			_handle: object,
			color: UiColorDto
		) => {
			if (!reference.target.valid()) throw new Error('Color target is no longer available');
			await reference.target.write(color);
		},
		accepted: () => {},
		active: () => {},
		finished: () => {}
	});
	export function open(next: NodeColorTarget, anchor: { x: number; y: number }) {
		if (gestureState.active || appState.session?.hasActiveEditSession || !next.valid()) return;
		target = next;
		point = anchor;
		draft = next.read();
	}
	export function close() {
		target = null;
		gesture.finish();
	}
	function change(color: unknown) {
		if (!target?.valid() || !color || typeof color !== 'object') return;
		const value = color as UiColorDto;
		const clamp = (v: number) => (Number.isFinite(v) ? Math.max(0, Math.min(1, v)) : 0);
		draft = { r: clamp(value.r), g: clamp(value.g), b: clamp(value.b), a: clamp(value.a) };
		if (!gestureState.active && !gesture.start({ target }, {})) return;
		gesture.move(draft);
	}
	function position() {
		if (!panel) return;
		const rect = panel.getBoundingClientRect();
		const placement = computePointAnchoredMenuPlacement({
			bounds: getMainViewportBounds(),
			anchorX: point.x,
			anchorY: point.y,
			menuWidth: rect.width,
			menuHeight: rect.height
		});
		panel.style.left = `${placement.left}px`;
		panel.style.top = `${placement.top}px`;
	}
	$effect(() => {
		if (panel) position();
	});
	$effect(() => {
		gesture.observe();
		if (target && !target.valid()) {
			target = null;
			gesture.stop();
		}
	});
	$effect(() => {
		if (target && !gestureState.active) draft = target.read();
	});
	onDestroy(() => gesture.dispose());
</script>

<svelte:window
	onresize={position}
	onkeydown={(event) => {
		if (target && event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			close();
		}
	}} />
{#if target}
	<button
		class="color-backdrop"
		data-local-context-menu
		aria-label="Close color picker"
		onpointerdown={close}></button>
	<div class="gc-node-context-color-submenu" data-local-context-menu bind:this={panel}>
		<ColorPicker forceExpanded={true} color={draft} onchange={change} />
		{#if gestureState.error}<p role="alert">{gestureState.error}</p>{/if}
	</div>
{/if}

<style>
	.color-backdrop {
		position: fixed;
		inset: 0;
		z-index: 1301;
		border: 0;
		background: transparent;
	}
	.gc-node-context-color-submenu {
		position: fixed;
		z-index: 1302;
		inline-size: min(24rem, calc(100vw - 1rem));
		max-block-size: calc(100vh - 1rem);
		overflow: auto;
		overscroll-behavior: contain;
	}
</style>
