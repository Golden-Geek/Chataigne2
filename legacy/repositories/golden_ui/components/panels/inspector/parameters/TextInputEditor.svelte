<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { sendSetTextParamSmartIntent } from '../../../../store/ui-intents';
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
	let value = $derived.by((): string => {
		if (!param || param.value.kind !== 'str') {
			return '';
		}
		if (param.control.mode === 'templateText' && param.control.spec.mode === 'templateText') {
			return param.control.spec.template;
		}
		return param.value.value;
	});

	let draftValue = $state('');
	let inlineLabel = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let widgetHint = $derived((param?.ui_hints?.widget ?? '').trim().toLowerCase());
	let multiline = $derived(widgetHint === 'textarea' || widgetHint === 'multiline');
	let showsInlineLabel = $derived(layoutMode === 'widget' && inlineLabel.length > 0);

	$effect(() => {
		draftValue = value;
	});

	const commitValue = async (nextValue: string): Promise<void> => {
		if (!param || param.value.kind !== 'str' || readOnly || !enabled) {
			return;
		}
		if (nextValue === value) {
			return;
		}

		draftValue = nextValue;
		await sendSetTextParamSmartIntent(liveNode.node_id, nextValue, param.event_behaviour);
	};

	const updateDraftValue = (event: Event): void => {
		draftValue = (event.target as HTMLInputElement | HTMLTextAreaElement).value;
	};

	const commitTargetValue = (target: HTMLInputElement | HTMLTextAreaElement): void => {
		void commitValue(target.value);
	};

	const resetTargetValue = (target: HTMLInputElement | HTMLTextAreaElement): void => {
		draftValue = value;
		target.blur();
	};
</script>

{#if showsInlineLabel}
	<div class="string-editor-shell widget-layout" class:multiline-layout={multiline}>
		<span class="string-editor-prefix">{inlineLabel} :</span>
		{#if multiline}
			<textarea
				class="string-editor inline-labeled multiline"
				value={draftValue}
				disabled={!enabled}
				rows="5"
				class:readonly={readOnly}
				oninput={updateDraftValue}
				onblur={(event) => {
					commitTargetValue(event.target as HTMLTextAreaElement);
				}}
				onkeydown={(event) => {
					const target = event.target as HTMLTextAreaElement;
					if ((event.key === 'Enter' && (event.ctrlKey || event.metaKey)) || event.key === 'Tab') {
						commitTargetValue(target);
					}
					if (event.key === 'Escape') {
						resetTargetValue(target);
					}
				}}></textarea>
		{:else}
			<input
				type="text"
				class="string-editor inline-labeled"
				value={draftValue}
				disabled={!enabled}
				class:readonly={readOnly}
				onchange={(event) => {
					commitTargetValue(event.target as HTMLInputElement);
				}}
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						const target = event.target as HTMLInputElement;
						commitTargetValue(target);
						target.blur();
					}
					if (event.key === 'Escape') {
						resetTargetValue(event.target as HTMLInputElement);
					}
				}} />
		{/if}
	</div>
{:else if multiline}
	<textarea
		class="string-editor multiline"
		class:widget-layout={layoutMode === 'widget'}
		value={draftValue}
		disabled={!enabled}
		rows="5"
		class:readonly={readOnly}
		oninput={updateDraftValue}
		onblur={(event) => {
			commitTargetValue(event.target as HTMLTextAreaElement);
		}}
		onkeydown={(event) => {
			const target = event.target as HTMLTextAreaElement;
			if ((event.key === 'Enter' && (event.ctrlKey || event.metaKey)) || event.key === 'Tab') {
				commitTargetValue(target);
			}
			if (event.key === 'Escape') {
				resetTargetValue(target);
			}
		}}></textarea>
{:else}
	<input
		type="text"
		class="string-editor"
		class:widget-layout={layoutMode === 'widget'}
		value={draftValue}
		disabled={!enabled}
		class:readonly={readOnly}
		onchange={(event) => {
			commitTargetValue(event.target as HTMLInputElement);
		}}
		onkeydown={(event) => {
			if (event.key === 'Enter') {
				const target = event.target as HTMLInputElement;
				commitTargetValue(target);
				target.blur();
			}
			if (event.key === 'Escape') {
				resetTargetValue(event.target as HTMLInputElement);
			}
		}} />
{/if}

<style>
	.string-editor-shell {
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

	.string-editor-shell.multiline-layout {
		align-items: flex-start;
	}

	.string-editor-prefix {
		flex: 0 0 auto;
		font-size: 0.72rem;
		font-weight: 600;
		white-space: nowrap;
	}

	.string-editor {
		flex: 1;
		height: 100%;
		box-sizing: border-box;
		font-size: 0.75rem;
	}

	.string-editor.widget-layout {
		inline-size: 100%;
		block-size: 100%;
	}

	.string-editor.inline-labeled {
		border: none;
		background: transparent;
		padding: 0;
		min-inline-size: 0;
		outline: none;
	}

	.string-editor.multiline {
		min-block-size: 6rem;
		resize: vertical;
		padding: 0.55rem 0.7rem;
		line-height: 1.45;
		font: inherit;
	}
</style>
