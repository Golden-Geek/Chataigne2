<script lang="ts">
	import { untrack } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type WorkbenchSession } from 'golden_ui';
	import CodeEditor from 'golden_ui/components/common/CodeEditor.svelte';
	import { sendSetParamIntent } from 'golden_ui/store/ui-intents';
	import applyIcon from 'golden_ui/style/icons/play.svg';

	let { node, session }: { node: UiNodeDto; session: WorkbenchSession } = $props();
	const target = untrack(() => ({
		id: node.node_id,
		uuid: node.uuid,
		generation: session.snapshotGeneration
	}));
	const parameter = $derived(node.data.kind === 'parameter' ? node.data.param : null);
	const text = $derived(parameter?.value.kind === 'str' ? parameter.value.value : '');
	const readonly = $derived(
		!parameter || parameter.read_only || !node.meta.enabled || session.loading.visible
	);
	let draft = $state(untrack(() => text));
	let applying = $state(false);
	let error = $state('');
	$effect(() => {
		draft = text;
		error = '';
	});

	async function commit(value: string) {
		if (
			!parameter ||
			readonly ||
			applying ||
			value === text ||
			appState.session !== session ||
			session.status !== 'connected' ||
			session.snapshotGeneration !== target.generation ||
			session.getNodeData(target.id)?.uuid !== target.uuid
		)
			return;
		applying = true;
		error = '';
		try {
			if (
				!(await sendSetParamIntent(target.id, { kind: 'str', value }, parameter.event_behaviour))
			) {
				error = 'The shader edit could not be applied.';
			}
		} finally {
			applying = false;
		}
	}
</script>

<div class="embedded-shader-editor" aria-label="Embedded GLSL code">
	<div class="embedded-shader-toolbar">
		<span>GLSL</span>
		<IconButton
			icon={applyIcon}
			label="Apply shader"
			disabled={readonly || applying || draft === text}
			onclick={() => void commit(draft)} />
	</div>
	<CodeEditor
		value={text}
		{readonly}
		fill
		commitOnBlur
		onchange={(value) => (draft = value)}
		oncommit={commit} />
	{#if error}<span class="error" role="status">{error}</span>{/if}
</div>

<style>
	.embedded-shader-editor {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		height: 100%;
	}
	.embedded-shader-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-inline: 0.4rem;
		font-size: 0.75rem;
	}
	.error {
		padding: 0.25rem 0.4rem;
	}
</style>
