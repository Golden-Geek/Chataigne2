<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto } from '../../../../types';

	let {
		node,
		layoutMode = 'default',
		insideLabel = null,
		readOnly: externalReadOnly = false
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		insideLabel?: string | null;
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let value = $derived(param?.value.kind === 'enum' ? param.value.value : '');
	let options = $derived(param?.constraints.enum_options ?? []);
	let inlineLabel = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsInlineLabel = $derived(layoutMode === 'widget' && inlineLabel.length > 0);

	let draftValue = $state('');
	let editSession = createUiEditSession('Select option', 'param-enum');

	$effect(() => {
		draftValue = value;
	});

	const updateValue = async (nextValue: string): Promise<void> => {
		if (!param || param.value.kind !== 'enum' || readOnly || !enabled) {
			return;
		}
		draftValue = nextValue;
		await editSession.begin();
		try {
			await sendSetParamIntent(
				liveNode.node_id,
				{ kind: 'enum', value: nextValue },
				param.event_behaviour
			);
		} finally {
			await editSession.end();
		}
	};
</script>

{#if showsInlineLabel}
	<div class="dropdown-editor-shell widget-layout">
		<span class="dropdown-editor-prefix">{inlineLabel} :</span>
		<select
			class="dropdown-editor inline-labeled"
			value={draftValue}
			disabled={!enabled}
			class:readonly={readOnly}
			onchange={(event) => {
				void updateValue((event.target as HTMLSelectElement).value);
			}}>
			{#each options as { variant_id, label }}
				<option value={variant_id}>{label}</option>
			{/each}
		</select>
	</div>
{:else}
	<select
		class="dropdown-editor"
		class:widget-layout={layoutMode === 'widget'}
		value={draftValue}
		disabled={!enabled}
		class:readonly={readOnly}
		onchange={(event) => {
			void updateValue((event.target as HTMLSelectElement).value);
		}}>
		{#each options as { variant_id, label }}
			<option value={variant_id}>{label}</option>
		{/each}
	</select>
{/if}

<style>
	.dropdown-editor-shell {
		display: flex;
		align-items: center;
		gap: 0.55rem;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		padding: 0.6rem 0.8rem;
		border-radius: 0.75rem;
		background: rgb(from var(--gc-color-background) r g b / 0.48);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.48);
		box-sizing: border-box;
	}

	.dropdown-editor-prefix {
		flex: 0 0 auto;
		font-size: 0.72rem;
		font-weight: 600;
		white-space: nowrap;
	}

	.dropdown-editor {
		height: 100%;
		width: 100%;
		box-sizing: border-box;
		font-size: 0.75rem;
	}

	.dropdown-editor.widget-layout {
		inline-size: 100%;
		block-size: 100%;
	}

	.dropdown-editor.inline-labeled {
		flex: 1 1 auto;
		min-inline-size: 0;
		border: none;
		background: transparent;
		padding: 0;
		outline: none;
	}
</style>
