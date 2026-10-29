<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { measurePreview } from './geometry';
	import { observePreviewLayout } from './observation';
	import type { PreviewSession, PreviewSessionState, PreviewRegistration } from './session';

	type Props = Omit<HTMLAttributes<HTMLDivElement>, 'children'> & {
		source: string;
		session: PreviewSession;
		visible?: boolean;
		label?: string;
		children?: Snippet;
	};
	let {
		source,
		session,
		visible = true,
		label = 'Preview',
		children,
		...attributes
	}: Props = $props();
	let element: HTMLDivElement;
	let availability = $state<PreviewSessionState['availability']>('idle');
	let installed = $state(false);
	let registrationError = $state(false);
	let registration = $state<PreviewRegistration | null>(null);

	$effect(() => {
		if (!element) return;
		const owner = session;
		const unsubscribe = owner.subscribe((state) => {
			availability = state.availability;
			installed = state.installed;
		});
		let view: PreviewRegistration;
		let observation: ReturnType<typeof observePreviewLayout> | undefined;
		try {
			view = owner.register(
				untrack(() => source),
				() => {
					const geometry = measurePreview(element, visible);
					observation?.measured(geometry);
					return geometry;
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
	aria-label={label}
>
	{#if visible && (availability !== 'available' || !installed || registrationError)}
		<div class="unavailable">Preview unavailable</div>
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
		background: var(--preview-unavailable-background, #202329);
		color: var(--preview-unavailable-color, #b7bcc5);
		font-size: 0.85em;
	}
	.overlay {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
</style>
