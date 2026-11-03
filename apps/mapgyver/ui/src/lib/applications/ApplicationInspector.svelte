<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import {
		appState,
		type WorkbenchSession,
		type UiNodeDto,
		type NodeInspectorChildFilter
	} from 'golden_ui';
	import { ApplicationController, applicationState } from './authoring';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { ApplicationRuntimeReader, runtimeState } from './runtime';
	import { createRuntimeTransport } from './runtime-transport';
	import RuntimeStatus from './RuntimeStatus.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const session = $derived(appState.session);
	const application = $derived.by(() => {
		const parentId = session?.graph.state.parentById.get(node.node_id);
		const parent = parentId == null ? null : session?.getNodeData(parentId);
		return parent?.node_type === 'mapgyver.matrix.cell.v2' ||
			parent?.node_type === 'mapgyver.matrix.column.v2'
			? parent.uuid
			: node.uuid;
	});
	const configured = $derived.by(() => {
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		const file = folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters/app_file');
		return (
			file?.data.kind === 'parameter' &&
			file.data.param.value.kind === 'file' &&
			!!file.data.param.value.value
		);
	});
	const view = $state(applicationState());
	const controller = new ApplicationController<WorkbenchSession>(
		() => session,
		view,
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
		const uuid = node.uuid;
		const target = application;
		const currentNode = current?.getNodeData(node.node_id);
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			if (currentNode?.uuid !== uuid) {
				controller.select(null);
				return;
			}
			if (view.selected !== target) controller.select(null);
			controller.observe();
			if (view.selected !== target) controller.select(target);
		});
	});
	onDestroy(() => controller.dispose());
</script>

{@render defaultHeader?.()}
{#snippet content()}
	{@render defaultChildren?.('', (child) => child.decl_id === 'parameters' || configured)}
	<div class="properties">
		{#if view.error || view.readError}<p role="alert">{view.error || view.readError}</p>{/if}
		{#if view.missing}<p role="status">Application no longer exists.</p>{/if}
		<details>
			<summary>Runtime</summary><RuntimeStatus
				snapshot={runtime.snapshot}
				selected={view.snapshot}
				live={runtime.live}
				loading={runtime.loading}
				error={runtime.error || runtime.transportError}
				disabled={busy}
				refresh={() => runtimeReader.refresh()} />
		</details>
	</div>
{/snippet}
{@render defaultContent?.(content)}

<style>
	.properties {
		display: grid;
		gap: 0.5rem;
		padding: 0.5rem;
	}
</style>
