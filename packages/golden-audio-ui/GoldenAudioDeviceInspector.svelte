<script lang="ts">
	import type { NodeInspectorChildFilter, NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import AudioDeviceSelector from './AudioDeviceSelector.svelte';
	import { resolveGoldenAudioDeviceInspectorBinding } from './binding-registry';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let binding = $derived(resolveGoldenAudioDeviceInspectorBinding(node));

	const childMatchesKey = (child: UiNodeDto, key: string): boolean =>
		child.decl_id === key ||
		child.decl_id.split('/').at(-1) === key ||
		child.meta.short_name === key;

	const renderUnmanagedChild: NodeInspectorChildFilter = (child) =>
		!binding?.managedChildKeys?.some((key) => childMatchesKey(child, key));
</script>

{@render defaultHeader?.()}

{#snippet audioDeviceContent()}
	{#if binding}
		<AudioDeviceSelector {binding} />
	{:else}
		<p class="binding-error" role="alert">
			This audio node has no registered device-inspector adapter.
		</p>
	{/if}
	{@render defaultChildren?.('', renderUnmanagedChild)}
{/snippet}

{@render defaultContent?.(audioDeviceContent, 'golden-audio-device-inspector')}

<style>
	.binding-error {
		margin: 0.75rem;
		padding: 0.65rem;
		border: 0.0625rem solid #8e4444;
		border-radius: 0.35rem;
		background: #3c2328;
		color: #ffd8d8;
	}

	:global(.golden-audio-device-inspector) {
		min-inline-size: 0;
	}
</style>
