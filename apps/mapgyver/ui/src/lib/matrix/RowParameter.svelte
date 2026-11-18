<script lang="ts">
	import { appState, resolveParameterEditor, type UiNodeDto } from 'golden_ui';
	let { uuid, label, field }: { uuid: string; label: string; field: 'opacity' | 'blend' } =
		$props();
	const session = $derived(appState.session);
	const graph = $derived(session?.graph.state);
	const element = $derived(graph?.nodesById.getByUuid(uuid));
	// Declaration-only indexes exclude the Layer's potentially large Cell collection.
	function child(parent: UiNodeDto | undefined, declaration: string) {
		if (!parent || !graph) return undefined;
		for (const id of graph.structuralChildrenById.get(parent.node_id) ?? []) {
			const node = graph.nodesById.get(id);
			if (node?.decl_id === declaration) return node;
		}
		return undefined;
	}
	const parameters = $derived(child(element, 'parameters'));
	const parameter = $derived(child(parameters, `parameters/${field}`));
	const Editor = $derived(resolveParameterEditor(parameter, graph?.nodesById)?.component);
	function readOnly(node: UiNodeDto) {
		const param = node.data.kind === 'parameter' ? node.data.param : null;
		return (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.historyBusy ||
			!param ||
			param.read_only ||
			!['manual', 'templateText'].includes(param.control.mode)
		);
	}
</script>

{#if parameter && Editor}
	<div
		class="row-parameter"
		class:blend={field === 'blend'}
		data-matrix-row-controls
		data-matrix-parameter={field}
		data-parameter-uuid={parameter.uuid}
		data-parameter-id={parameter.node_id}
		draggable="false"
		title={`${parameter.meta.label} for ${label}`}>
		<div class="editor">
			<Editor
				node={parameter}
				layoutMode="widget"
				readOnly={readOnly(parameter)}
				accessibleLabel={`${parameter.meta.label} for ${label}`}
				presentation={field === 'opacity'
					? { slider_orientation: 'vertical', show_value_field: false }
					: undefined} />
		</div>
	</div>
{/if}

<style>
	.row-parameter,
	.editor {
		min-inline-size: 0;
		inline-size: 100%;
		block-size: 100%;
	}
	.row-parameter.blend {
		inline-size: min(7.25rem, 100%);
		block-size: 1.35rem;
		flex: none;
	}
</style>
