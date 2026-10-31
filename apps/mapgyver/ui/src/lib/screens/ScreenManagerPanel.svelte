<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState, type WorkbenchSession } from 'golden_ui';
	import { invokeScreenEdit } from './operations';
	import { invokeMediaEdit } from '../media/operations';
	import { SCREEN_NAMESPACE } from './generated/constants';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { createScreenAuthoringState, ScreenAuthoringController } from './authoring';
	import ScreenNavigation from './ScreenNavigation.svelte';
	import SurfaceNavigation from './SurfaceNavigation.svelte';
	import ScreenTargetForm from './ScreenTargetForm.svelte';
	import SurfaceSourceForm from './SurfaceSourceForm.svelte';
	import ScreenPreview from './ScreenPreview.svelte';
	import { createUiEditSession } from 'golden_ui/store/ui-intents';
	import { CornerGesture, createCornerGestureState } from './corner-gesture';

	let { panelId, panelType, title }: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
	const session = $derived(appState.session);
	const view = $state(createScreenAuthoringState());
	const controller = new ScreenAuthoringController<WorkbenchSession>(() => session, view, {
		screens: invokeScreenEdit,
		media: async (current) => {
			const result = await invokeMediaEdit(current, { kind: 'InspectManager' });
			if (result.kind !== 'Manager')
				throw new Error('Media inspection returned an unexpected result');
			return result.snapshot;
		}
	});
	const screen = $derived(controller.screen);
	const surface = $derived(controller.surface);
	const gestureState = $state(createCornerGestureState());
	const gesture = new CornerGesture<WorkbenchSession>(() => session, gestureState, {
		group: (current) => createUiEditSession('Warp Surface', 'surface-corner', current),
		move: async (current, reference, corner, point) => {
			const result = await invokeScreenEdit(current, {
				kind: 'MoveSurfaceCorner',
				reference,
				corner,
				point
			});
			if (result.kind !== 'SurfaceChanged')
				throw new Error('Corner edit returned an unexpected result');
			return result.snapshot;
		},
		accepted: (snapshot) => controller.acceptSurface(snapshot),
		active: (active) => controller.setInteracting(active),
		finished: () => {
			void controller.refresh();
		}
	});
	const busy = $derived(controller.busy || gestureState.active);
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(SCREEN_NAMESPACE);
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			gesture.observe();
			void controller.refresh();
		});
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
</script>

<section
	class="manager"
	aria-label={panel?.title ?? title ?? 'Screen Manager'}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<header>
		<div>
			<h2>Screen Manager</h2>
			<p>Arrange screens and map reusable source channels.</p>
		</div>
		<button disabled={busy} onclick={() => controller.edit({ kind: 'CreateScreen', label: null })}
			>Add screen</button>
		<button
			disabled={view.pending ||
				view.refreshing ||
				session?.status !== 'connected' ||
				session.loading.visible}
			onclick={() => controller.refresh()}>Refresh</button>
	</header>
	<p class="notice">
		Enabled fullscreen, rectangle and virtual Spout outputs keep running when this editor closes.
		Unavailable fullscreen displays keep their saved selection and resume when available.
	</p>
	{#if view.error}<p class="error" role="alert">{view.error}</p>{/if}
	{#if gestureState.error}<p class="error" role="status">{gestureState.error}</p>{/if}
	{#if session?.status !== 'connected'}
		<p>Connect to MapGyver to edit screens.</p>
	{:else if view.catalog === null}
		<p>Loading screens…</p>
	{:else if view.catalog.screens.length === 0}
		<div class="empty">
			<h3>No screens yet</h3>
			<p>Add a screen, choose its target and assign channels to its surfaces.</p>
		</div>
	{:else}
		<div class="workspace">
			<ScreenNavigation
				catalog={view.catalog}
				selected={view.selectedScreen}
				disabled={busy}
				select={(id) => controller.selectScreen(id)}
				edit={(intent) => controller.edit(intent)} />
			<div class="editor">
				{#if screen}
					{#key screen.id}
						<ScreenPreview
							{screen}
							selected={view.selectedSurface}
							disabled={controller.busy}
							select={(id) => {
								if (!gestureState.active) view.selectedSurface = id;
							}}
							start={(reference, corner) => gesture.start(reference, corner)}
							move={(point) => gesture.move(point)}
							finish={() => gesture.finish()}
							stop={() => gesture.stop()} />
						<ScreenTargetForm
							{screen}
							disabled={busy}
							configure={(definition) =>
								controller.edit({ kind: 'ConfigureScreen', screen: screen.id, definition })} />
						<SurfaceNavigation
							{screen}
							selected={view.selectedSurface}
							disabled={busy}
							select={(id) => (view.selectedSurface = id)}
							edit={(intent) => controller.edit(intent)} />
					{/key}
					{#if surface}
						{#key surface.reference.surface}
							<SurfaceSourceForm
								{surface}
								media={view.media}
								disabled={busy}
								configure={(definition) =>
									controller.edit({
										kind: 'ConfigureSurface',
										reference: surface.reference,
										definition
									})} />
						{/key}
					{/if}
				{/if}
			</div>
		</div>
	{/if}
</section>

<style>
	.manager {
		container-type: inline-size;
		height: 100%;
		overflow: auto;
		padding: 1.2rem;
		box-sizing: border-box;
		color: #dedfe5;
		background: var(--mapgyver-panel-background, #181b22);
		font-size: 0.9rem;
	}
	header {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.7rem;
		margin-bottom: 1rem;
	}
	header > div {
		flex: 1 1 100%;
	}
	h2 {
		font-size: 1.2rem;
		margin: 0 0 0.35rem;
	}
	p {
		margin: 0;
	}
	header p,
	.manager :global(.hint) {
		opacity: 0.65;
	}
	.notice {
		padding: 0.8rem;
		border: 0.0625rem solid #8fa8cd50;
		border-radius: 0.4rem;
		margin-bottom: 1rem;
	}
	.workspace {
		display: grid;
		grid-template-columns: minmax(12rem, 0.7fr) minmax(16rem, 1.3fr);
		gap: 1.2rem;
	}
	.editor {
		display: grid;
		align-content: start;
		gap: 1.2rem;
		min-width: 0;
	}
	.empty {
		display: grid;
		gap: 0.6rem;
		padding: 1rem 0;
	}
	.error {
		color: #ffb0b0;
		padding: 0.7rem 0;
		overflow-wrap: anywhere;
	}
	.manager :global(button),
	.manager :global(input),
	.manager :global(select) {
		font: inherit;
		color: inherit;
		background: #ffffff0a;
		border: 0.0625rem solid #ffffff30;
		border-radius: 0.3rem;
		padding: 0.5em 0.65em;
		min-width: 0;
	}
	.manager :global(button) {
		cursor: pointer;
	}
	.manager :global(button:disabled) {
		opacity: 0.4;
		cursor: default;
	}
	.manager :global(fieldset) {
		display: grid;
		gap: 0.9rem;
		min-width: 0;
		margin: 0;
		padding: 1rem;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.5rem;
	}
	.manager :global(legend) {
		font-weight: 600;
		padding: 0 0.4em;
		overflow-wrap: anywhere;
	}
	.manager :global(label) {
		display: grid;
		gap: 0.35em;
		min-width: 0;
	}
	.manager :global(label.toggle) {
		display: flex;
		align-items: center;
		gap: 0.5em;
	}
	.manager :global(.fields) {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
		gap: 0.7rem;
	}
	.manager :global(h4) {
		margin: 0;
	}
	.manager :global(button[type='submit']) {
		justify-self: start;
		background: #416caa50;
	}
	.manager :global(.danger) {
		color: #f5a3a3;
	}
	@container (max-width: 40rem) {
		.workspace {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
