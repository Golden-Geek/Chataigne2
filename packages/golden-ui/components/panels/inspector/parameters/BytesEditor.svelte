<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto } from '../../../../types';

	let {
		node,
		readOnly: externalReadOnly = false,
		insideLabel = null
	} = $props<{
		node: UiNodeDto;
		readOnly?: boolean;
		insideLabel?: string | null;
		layoutMode?: 'default' | 'widget';
	}>();
	let liveNode = $derived(appState.session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let value = $derived(param?.value.kind === 'bytes' ? param.value.value : []);
	let readOnly = $derived(externalReadOnly || Boolean(param?.read_only));
	let draft = $state('');
	let error = $state('');
	$effect(() => {
		draft = value.join(' ');
		error = '';
	});

	async function commit(): Promise<void> {
		if (!param || readOnly || !liveNode.meta.enabled) return;
		const tokens = draft.trim() ? draft.trim().split(/[\s,]+/) : [];
		if (tokens.some((token) => !/^\d+$/.test(token) || Number(token) > 255)) {
			error = 'Enter byte values from 0 to 255, separated by spaces or commas.';
			return;
		}
		error = '';
		const bytes = tokens.map(Number);
		if (bytes.length === value.length && bytes.every((byte, index) => byte === value[index]))
			return;
		await sendSetParamIntent(
			liveNode.node_id,
			{ kind: 'bytes', value: bytes },
			param.event_behaviour
		);
	}
</script>

<div class="bytes-editor">
	<textarea
		aria-label={insideLabel || 'Byte values'}
		aria-invalid={Boolean(error)}
		bind:value={draft}
		readonly={readOnly}
		disabled={!liveNode.meta.enabled}
		rows="3"
		onblur={() => {
			void commit();
		}}
		onkeydown={(event) => {
			if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
				void commit();
			}
			if (event.key === 'Escape') {
				draft = value.join(' ');
				error = '';
			}
		}}></textarea>
	{#if error}<span role="alert">{error}</span>{/if}
</div>

<style>
	.bytes-editor {
		display: flex;
		flex-direction: column;
		gap: 0.3em;
		width: 100%;
	}
	textarea {
		width: 100%;
		min-width: 0;
		padding: 0.4em;
		font: inherit;
		resize: vertical;
	}
	span {
		font-size: 0.85em;
	}
</style>
