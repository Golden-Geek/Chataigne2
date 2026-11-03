<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { measurePreview } from './geometry';
	import { observePreviewLayout } from './observation';
	import type { PreviewSession, PreviewSessionState, PreviewRegistration } from './session';
	import type { OverlayPrimitive } from './generated/OverlayPrimitive';

	type Props = Omit<HTMLAttributes<HTMLDivElement>, 'children'> & {
		source: string;
		session: PreviewSession;
		visible?: boolean;
		label?: string;
		children?: Snippet;
		overlays?: OverlayPrimitive[];
		overlayCapacity?: number;
		onpresentationchange?(nativeOverlays: boolean): void;
	};
	let {
		source,
		session,
		visible = true,
		label = 'Preview',
		children,
		overlays = [],
		overlayCapacity = 0,
		onpresentationchange,
		...attributes
	}: Props = $props();
	let element: HTMLDivElement;
	let availability = $state<PreviewSessionState['availability']>('idle');
	let installed = $state(false);
	let presentedViews = $state<readonly string[]>([]);
	let registrationError = $state(false);
	let registration = $state<PreviewRegistration | null>(null);
	// Register in component order; intersection delivery can otherwise reorder
	// overlapping native surfaces. Hidden/clipped surfaces retire on observation.
	let inViewport = $state(true);
	const nativeOverlays = $derived(
		overlays.length > 0 &&
			registration !== null &&
			availability === 'available' &&
			presentedViews.includes(registration.id)
	);
	$effect(() => {
		onpresentationchange?.(nativeOverlays);
	});

	$effect(() => {
		if (!element) return;
		const observer = new IntersectionObserver((entries) => {
			inViewport = entries.some((entry) => entry.isIntersecting);
		});
		observer.observe(element);
		return () => observer.disconnect();
	});

	$effect(() => {
		if (!element || !visible || !inViewport) return;
		const owner = session;
		const unsubscribe = owner.subscribe((state) => {
			availability = state.availability;
			installed = state.installed;
			presentedViews = state.presentedViews;
		});
		let view: PreviewRegistration;
		let observation: ReturnType<typeof observePreviewLayout> | undefined;
		try {
			view = owner.register(
				untrack(() => source),
				() => {
					const geometry = measurePreview(element, visible);
					observation?.measured(geometry);
					return {
						...geometry,
						overlays: untrack(() => overlays),
						overlay_capacity: untrack(() => overlayCapacity)
					};
				}
			);
			registrationError = false;
		} catch {
			registrationError = true;
			return unsubscribe;
		}
		registration = view;
		observation = observePreviewLayout(owner, element, view.id);
		return () => {
			unsubscribe();
			observation.dispose();
			view.unregister();
			registration = null;
		};
	});

	$effect(() => {
		visible;
		overlays;
		overlayCapacity;
		registration?.update(source);
		registration?.invalidate();
	});
</script>

<div
	{...attributes}
	bind:this={element}
	class="native-preview {attributes.class ?? ''}"
	data-preview-state={availability}
	data-preview-installed={installed}
	data-native-overlays={nativeOverlays}
	aria-label={label}
>
	{#if visible && (availability !== 'available' || !registration || !presentedViews.includes(registration.id) || registrationError)}
		<div class="unavailable">
			{#if availability === 'unavailable' || availability === 'disposed' || registrationError}Preview
				unavailable{/if}
		</div>
	{/if}
	{#if children}<div class="overlay">{@render children()}</div>{/if}
</div>

<style>
	.native-preview {
		position: relative;
		width: 100%;
		height: 100%;
		min-height: 1em;
		background: transparent;
	}
	.unavailable {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		background: var(--preview-unavailable-background, #000);
		color: var(--preview-unavailable-color, #b7bcc5);
		font-size: 0.85em;
	}
	.overlay {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.native-preview[data-native-overlays='true'] :global([data-native-overlay-control]) {
		opacity: 0;
	}
</style>
