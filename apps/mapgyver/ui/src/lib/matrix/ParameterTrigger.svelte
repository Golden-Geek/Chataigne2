<script lang="ts">
	import { appState, IconButton, type UiNodeDto } from 'golden_ui';
	import stopIcon from '../icons/stop.svg';
	let {
		owner,
		field,
		label,
		icon = stopIcon,
		disabled = false
	}: {
		owner: string;
		field: 'play' | 'cut' | 'stop';
		label: string;
		icon?: string;
		disabled?: boolean;
	} = $props();
	const session = $derived(appState.session);
	const graph = $derived(session?.graph.state);
	const node = $derived(graph?.nodesById.getByUuid(owner));
	function child(parent: UiNodeDto | undefined, declaration: string) {
		if (!parent || !graph) return undefined;
		for (const id of graph.structuralChildrenById.get(parent.node_id) ?? []) {
			const child = graph.nodesById.get(id);
			if (child?.decl_id === declaration) return child;
		}
		return undefined;
	}
	const parameter = $derived(child(child(node, 'parameters'), `parameters/${field}`));
	const value = $derived(parameter?.data.kind === 'parameter' ? parameter.data.param : null);
	const readOnly = $derived(
		disabled ||
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.historyBusy ||
			!node?.meta.enabled ||
			!parameter?.meta.enabled ||
			!value ||
			value.value.kind !== 'trigger' ||
			value.read_only ||
			!['manual', 'templateText'].includes(value.control.mode)
	);
	async function invoke(event: MouseEvent) {
		event.stopPropagation();
		if (readOnly || !session || !parameter || !value) return;
		try {
			await session.sendIntent({
				kind: 'setParam',
				node: parameter.node_id,
				value: { kind: 'trigger' },
				behaviour: value.event_behaviour
			});
		} catch (error) {
			console.error('Matrix Parameter trigger failed', error);
		}
	}
</script>

<IconButton
	{icon}
	{label}
	disabled={readOnly}
	onclick={invoke}
	data-matrix-control={field}
	data-owner-uuid={owner}
	data-parameter-uuid={parameter?.uuid}
	data-parameter-id={parameter?.node_id} />
