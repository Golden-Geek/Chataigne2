<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { sendSetParamIntent } from '../../../../store/ui-intents';
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
	let enabled = $derived(liveNode.meta.enabled);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let value = $derived(param?.value.kind === 'bool' ? param.value.value : false);
	let inlineLabel = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsInlineLabel = $derived(layoutMode === 'widget' && inlineLabel.length > 0);

	let draftValue = $state(false);

	$effect(() => {
		draftValue = value;
	});

	const updateValue = (nextValue: boolean): void => {
		if (!param || param.value.kind !== 'bool' || readOnly) {
			return;
		}
		draftValue = nextValue;
		void sendSetParamIntent(
			liveNode.node_id,
			{ kind: 'bool', value: nextValue },
			param.event_behaviour
		);
	};
</script>

{#if layoutMode === 'widget'}
	<button
		type="button"
		class="editor-checkbox-shell widget-layout widget-checkbox-button"
		class:with-inline-label={showsInlineLabel}
		class:checked={draftValue}
		disabled={!enabled}
		class:readonly={readOnly}
		aria-pressed={draftValue}
		onclick={() => {
			if (readOnly || !enabled) {
				return;
			}
			updateValue(!draftValue);
		}}>
		<span class="widget-checkbox-mark" aria-hidden="true">
			{#if showsInlineLabel}
				{inlineLabel}
			{:else if draftValue}
				&#10003;
			{/if}
		</span>
	</button>
{:else}
	<div class="editor-checkbox-shell">
		<input
			type="checkbox"
			class="editor-checkbox"
			disabled={!enabled}
			class:readonly={readOnly}
			checked={draftValue}
			onchange={(event) => {
				updateValue((event.target as HTMLInputElement).checked);
			}} />
	</div>
{/if}

<style>
	.editor-checkbox-shell {
		display: flex;
		align-items: center;
		justify-content: right;
		inline-size: 100%;
	}

	.editor-checkbox-shell.widget-layout {
		block-size: 100%;
		padding: 0;
	}

	.widget-checkbox-button {
		inline-size: 100%;
		block-size: 100%;
		border-radius: 0.75rem;
		border: solid 0.08rem rgb(from var(--gc-color-checkbox-fg) r g b / 0.45) !important;
		background: rgb(from var(--gc-color-checkbox-bg) r g b / 0.95);
		padding: 0.6rem;
		box-sizing: border-box;
	}

	.widget-checkbox-button.checked {
		background: rgb(from var(--gc-color-checkbox-fg) r g b / 0.18);
		border-color: var(--gc-color-checkbox-fg) !important;
	}

	.widget-checkbox-button.readonly {
		background: rgb(from var(--gc-color-readonly) r g b / 0.18);
		border-color: rgb(from var(--gc-color-readonly) r g b / 0.4) !important;
	}

	.widget-checkbox-mark {
		display: flex;
		align-items: center;
		justify-content: center;
		inline-size: 100%;
		block-size: 100%;
		font-size: clamp(1.6rem, 4vw, 3.6rem);
		font-weight: 700;
		line-height: 1;
		color: var(--gc-color-checkbox-fg);
	}

	.widget-checkbox-button.with-inline-label .widget-checkbox-mark {
		padding-inline: 0.6rem;
		font-size: 0.82rem;
		font-weight: 600;
		text-align: center;
	}
</style>
