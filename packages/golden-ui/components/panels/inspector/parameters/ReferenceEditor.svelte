<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import type { UiNodeDto } from '../../../../types';
	import type { NodePickerModalView } from '../../../../store/node-picker-modal.svelte';
	import { resolveReferenceChoiceProvider } from '../reference-choice-provider';
	import ReferenceChoiceEditor from './ReferenceChoiceEditor.svelte';
	import ReferenceTreeEditor from './ReferenceTreeEditor.svelte';
	let {
		node,
		layoutMode = 'default',
		insideLabel = null,
		referencePickerViews = [],
		readOnly = false
	}: {
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		insideLabel?: string | null;
		referencePickerViews?: NodePickerModalView[];
		readOnly?: boolean;
	} = $props();
	const session = $derived(appState.session);
	const live = $derived(session?.getNodeData(node.node_id) ?? node);
	const provider = $derived(resolveReferenceChoiceProvider(live, session));
</script>

{#if provider}
	<ReferenceChoiceEditor node={live} {provider} {layoutMode} {insideLabel} {readOnly} />
{:else}
	<ReferenceTreeEditor node={live} {layoutMode} {insideLabel} {referencePickerViews} {readOnly} />
{/if}
