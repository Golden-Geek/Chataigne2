<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type WorkbenchSession, type UiNodeDto } from 'golden_ui';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import { ApplicationController, applicationState } from './authoring';
	import { catalogState } from './catalog';
	import ApplicationDefinitionForm from './ApplicationDefinitionForm.svelte';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { ApplicationRuntimeReader, runtimeState } from './runtime';
	import { createRuntimeTransport } from './runtime-transport';
	import RuntimeStatus from './RuntimeStatus.svelte';
	let { node, defaultHeader }: { node: UiNodeDto; defaultHeader?: Snippet } = $props();
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
	const runtime = $state(runtimeState());
	const runtimeReader = new ApplicationRuntimeReader(
		() => session,
		runtime,
		invokeMediaEdit,
		createRuntimeTransport()
	);
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
		}
		untrack(() => runtimeReader.observe());
	});
	onDestroy(() => runtimeReader.dispose());
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

	$effect(() => {
		const uuid = node.uuid;
		void session?.snapshotGeneration;
		untrack(() => {
			controller.observe();
			if (view.selected !== uuid) controller.select(uuid);
		});
	});
</script>

{@render defaultHeader?.()}
<div class="properties">
	{#if view.error || view.readError}<p role="alert">{view.error || view.readError}</p>{/if}
	{#if view.snapshot}
		{@const application = view.snapshot}
		<div class="actions">
			<label
				>Allow new launches<input
					type="checkbox"
					checked={application.enablement !== 'Disabled'}
					disabled={busy}
					onchange={(event) =>
						controller.edit({
							kind: 'SetEnabled',
							application: application.id,
							enabled: event.currentTarget.checked
						})} /></label>
			<IconButton
				icon={copyIcon}
				label="Duplicate application"
				disabled={busy}
				onclick={() => controller.edit({ kind: 'Duplicate', application: application.id })} />
			<IconButton
				icon={deleteIcon}
				label="Remove definition"
				disabled={busy}
				onclick={() => controller.edit({ kind: 'Remove', application: application.id })} />
		</div>
		{#key JSON.stringify([application.id, application.definition_instance, application.definition])}
			<ApplicationDefinitionForm
				definition={application.definition}
				disabled={busy}
				save={(definition) =>
					controller.edit({ kind: 'Configure', application: application.id, definition })} />
		{/key}
	{/if}
	<details>
		<summary>Runtime</summary><RuntimeStatus
			snapshot={runtime.snapshot}
			selected={view.snapshot}
			catalog={catalog.page}
			live={runtime.live}
			loading={runtime.loading}
			error={runtime.error || runtime.transportError}
			disabled={busy}
			refresh={() => runtimeReader.refresh()} />
	</details>
</div>

<style>
	.properties {
		display: grid;
		gap: 0.5rem;
		padding: 0.5rem;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}
	.actions label {
		flex: 1;
	}
</style>
