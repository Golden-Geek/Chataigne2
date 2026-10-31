<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState, type WorkbenchSession } from 'golden_ui';
	import { ApplicationController, applicationState } from './authoring';
	import { catalogState } from './catalog';
	import CatalogControls from './CatalogControls.svelte';
	import ApplicationDefinitionForm from './ApplicationDefinitionForm.svelte';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	let { panelId, panelType, title }: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
	const session = $derived(appState.session);
	const view = $state(applicationState()),
		catalog = $state(catalogState());
	const controller = new ApplicationController<WorkbenchSession>(
		() => session,
		view,
		catalog,
		invokeMediaEdit
	);
	const busy = $derived(controller.busy);
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => controller.observe());
	});
	onDestroy(() => controller.dispose());
</script>

<section
	aria-label={panel?.title ?? title ?? 'Applications'}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<header>
		<h2>Shared applications</h2>
		<button disabled={busy} onclick={() => controller.edit({ kind: 'Create', label: null })}
			>Add application</button>
		<button disabled={busy} onclick={() => controller.observe()}>Refresh applications</button>
	</header>
	<p>
		Define an app once, then reference it from multiple Matrix columns. Column distances control
		preloading and retention.
	</p>
	<CatalogControls
		state={catalog}
		disabled={busy}
		update={(search, start, refresh) => {
			catalog.search = search;
			catalog.start = start;
			if (refresh) controller.catalog.refresh();
		}} />
	{#if view.error || view.readError}<p role="alert">{view.error || view.readError}</p>{/if}
	{#if session?.status !== 'connected'}<p>Connect to MapGyver to edit applications.</p>
	{:else}
		<div class="workspace">
			<nav aria-label="Shared applications">
				<ul>
					{#each catalog.page?.applications ?? [] as application (application.id)}
						<li>
							<button
								disabled={busy}
								aria-pressed={view.selected === application.id}
								onclick={() => controller.select(application.id)}>
								{application.label}<small
									>{application.enabled ? '' : 'Disabled · '}{application.configured
										? 'Configured'
										: 'Unconfigured'}</small>
							</button>
						</li>
					{/each}
				</ul>
				{#if catalog.page?.total === 0}<p>No matching applications.</p>{/if}
			</nav>
			<div class="editor">
				{#if view.snapshot}
					{@const application = view.snapshot}
					<h3>{application.label}</h3>
					<small
						>Shared ID: {application.id} · {application.enabled ? 'Enabled' : 'Disabled'}</small>
					<form
						onsubmit={(event) => {
							event.preventDefault();
							const label = String(new FormData(event.currentTarget).get('label'));
							void controller.edit({ kind: 'Rename', application: application.id, label });
						}}>
						<label
							>Application name<input
								name="label"
								value={application.label}
								required
								disabled={busy} /></label>
						<button disabled={busy}>Rename application</button>
					</form>
					<div class="actions">
						<button
							disabled={busy}
							onclick={() => controller.edit({ kind: 'Duplicate', application: application.id })}
							>Duplicate application</button>
						<button
							disabled={busy}
							onclick={() => controller.edit({ kind: 'Remove', application: application.id })}
							>Remove definition</button>
					</div>
					<p>
						Removing a definition preserves saved column references; retained outgoing/render use
						releases its process when finished.
					</p>
					{#key JSON.stringify([application.id, application.definition])}
						<ApplicationDefinitionForm
							definition={application.definition}
							disabled={busy}
							save={(definition) =>
								controller.edit({ kind: 'Configure', application: application.id, definition })} />
					{/key}
				{:else if view.loading}<p>Loading application…</p>
				{:else}<p>Select a shared application, or add one with backend defaults.</p>{/if}
			</div>
		</div>
	{/if}
</section>

<style>
	section {
		padding: 0.75rem;
		overflow: auto;
		height: 100%;
		box-sizing: border-box;
	}
	header,
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}
	.workspace {
		display: grid;
		grid-template-columns: minmax(10rem, 1fr) minmax(0, 3fr);
		gap: 1rem;
	}
	ul {
		list-style: none;
		padding: 0;
	}
	nav button {
		display: grid;
		width: 100%;
		text-align: left;
		margin-bottom: 0.4rem;
	}
	[aria-pressed='true'] {
		outline: 0.1rem solid currentColor;
	}
	form,
	label {
		display: grid;
		gap: 0.4rem;
		margin: 0.5rem 0;
		min-width: 0;
	}
	input {
		min-width: 0;
		max-width: 100%;
	}
	small {
		overflow-wrap: anywhere;
		opacity: 0.8;
	}
	@media (max-width: 40rem) {
		.workspace {
			grid-template-columns: 1fr;
		}
	}
</style>
