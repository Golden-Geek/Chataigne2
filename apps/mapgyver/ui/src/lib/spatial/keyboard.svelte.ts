import { onDestroy, untrack } from 'svelte';
import { appState } from 'golden_ui';
import { KeyboardGesture } from 'golden_ui/store/keyboard-gesture';
import type { Point } from '../media/generated/Point';
import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
import type { SpatialResizeHandle } from '../media/generated/SpatialResizeHandle';
import { SPATIAL_RESIZE_HANDLES } from './handles';

export type SpatialKeyboardMode = RectGeometryMode | { Rotate: { handle: SpatialResizeHandle } };

export interface SpatialKeyboardClient {
	id: string | null;
	active: boolean;
	disabled: boolean;
	start(mode: SpatialKeyboardMode): boolean;
	move(point: Point): void;
	finish(): void;
	stop(): void;
}

/** Focused spatial input; each adapter captures its own immutable backend geometry basis. */
export function createSpatialKeyboard(client: () => SpatialKeyboardClient | undefined) {
	let owned = $state(false);
	let editing: SpatialKeyboardClient | null = null;
	let previousSession: unknown;
	let mode = $state<SpatialKeyboardMode>('Translate');
	const keyboard = new KeyboardGesture<{
		identity: string;
		client: SpatialKeyboardClient;
		mode: SpatialKeyboardMode;
	}>({
		identity: (target) => target.identity,
		start: (target) => {
			if (target.client.disabled || !target.client.start(target.mode)) return false;
			editing = target.client;
			owned = true;
			return true;
		},
		move: (point) => editing?.move(point),
		finish: () => {
			if (owned) editing?.finish();
		},
		stop: () => {
			if (owned) editing?.stop();
		}
	});
	const target = $derived.by(() => {
		const session = appState.session,
			current = client();
		return session?.status === 'connected' && !session.loading.visible && current?.id
			? {
					identity: JSON.stringify([
						session.projectGeneration,
						session.snapshotGeneration,
						current.id,
						mode
					]),
					client: current,
					mode
				}
			: null;
	});
	$effect(() => {
		const current = client(),
			next = target,
			session = appState.session;
		if (!current?.active) {
			owned = false;
			editing = null;
		}
		const active = owned && !!current?.active,
			ready = !!current && !current.disabled && !current.active;
		untrack(() => {
			if (previousSession !== session) {
				keyboard.cancel();
				previousSession = session;
			}
			keyboard.observe(next, active, ready);
		});
	});
	onDestroy(() => keyboard.cancel());
	return {
		keydown(event: KeyboardEvent) {
			if (event.key === 'Escape') {
				if (owned) event.preventDefault();
				keyboard.cancel();
				return;
			}
			if (client()?.active && !owned) return;
			const control =
				event.target instanceof Element
					? event.target.closest(
							'[data-instance-resize], [data-matrix-layer-resize], [data-matrix-cell-resize], [data-instance-rotation]'
						)
					: null;
			const name =
				control?.getAttribute('data-instance-resize') ??
				control?.getAttribute('data-matrix-layer-resize') ??
				control?.getAttribute('data-matrix-cell-resize') ??
				control?.getAttribute('data-instance-rotation');
			const handle = SPATIAL_RESIZE_HANDLES.find((handle) => handle.name === name)?.name;
			mode = handle
				? control?.hasAttribute('data-instance-rotation')
					? { Rotate: { handle } }
					: { Resize: { handle } }
				: 'Translate';
			if (keyboard.keydown(event, target)) {
				event.preventDefault();
				event.stopPropagation();
			}
		},
		keyup: (event: KeyboardEvent) => keyboard.keyup(event.key),
		finish: () => keyboard.finish()
	};
}
