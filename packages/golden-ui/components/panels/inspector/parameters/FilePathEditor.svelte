<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto } from '../../../../types';
	import FileInput from '../../../common/FileInput.svelte';

	let {
		node,
		readOnly: externalReadOnly = false
	}: {
		node: UiNodeDto;
		readOnly?: boolean;
	} = $props();
	const session = $derived(appState.session);
	const liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	const param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	const value = $derived(param?.value.kind === 'file' ? param.value.value : '');
	const readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
</script>

<FileInput
	{value}
	{readOnly}
	enabled={liveNode.meta.enabled}
	fileConstraints={param?.constraints.file}
	label={liveNode.meta.label}
	oncommit={(value) => {
		if (param?.value.kind === 'file' && !readOnly && liveNode.meta.enabled)
			return sendSetParamIntent(liveNode.node_id, { kind: 'file', value }, param.event_behaviour);
	}} />
