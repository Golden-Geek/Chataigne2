<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, FileInput, NodeInspector, type UiNodeDto } from 'golden_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { PictureFileBinding } from './generated/PictureFileBinding';
	import type { MediaEdit } from './generated/MediaEdit';
	import type { MediaEditResult } from './generated/MediaEditResult';
	import { invokeMediaEdit } from './operations';
	let {
		node,
		channel,
		busy,
		chooseExisting = false,
		edit
	}: {
		node: UiNodeDto;
		channel: ChannelSnapshot;
		busy: boolean;
		chooseExisting?: boolean;
		edit: (intent: MediaEdit) => Promise<MediaEditResult | undefined>;
	} = $props();
	const session = $derived(appState.session);
	let result = $state.raw<{
		session: typeof session;
		generation: number;
		binding: PictureFileBinding;
	} | null>(null);
	let error = $state('');
	let request = 0;
	let disposed = false;
	const binding = $derived(
		result?.session === session &&
			result?.generation === session?.snapshotGeneration &&
			result?.binding.reference.channel === channel.reference.channel
			? result.binding
			: null
	);
	const file = $derived.by(() => {
		const address = binding?.parameter;
		const file = address && session?.getNodeData(address.node);
		return file?.uuid === address?.uuid &&
			file?.data.kind === 'parameter' &&
			file.data.param.value.kind === 'file'
			? file
			: null;
	});
	const reference = $derived.by(() => {
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		return (
			folder?.children
				.map((id) => session?.getNodeData(id))
				.find((child) => child?.decl_id === 'parameters/asset') ?? null
		);
	});
	async function refresh() {
		const current = session;
		const id = ++request;
		if (
			!current ||
			current.status !== 'connected' ||
			current.loading.visible ||
			current.hasActiveEditSession
		)
			return;
		const generation = current.snapshotGeneration;
		const selected = channel.reference;
		try {
			const value = await invokeMediaEdit(current, {
				kind: 'InspectPictureFile',
				reference: selected
			});
			if (
				disposed ||
				id !== request ||
				current !== session ||
				generation !== current.snapshotGeneration ||
				selected.channel !== channel.reference.channel
			)
				return;
			result =
				value.kind === 'PictureFile' && value.binding
					? { session: current, generation, binding: value.binding }
					: null;
			error = '';
		} catch (cause) {
			if (
				!disposed &&
				id === request &&
				current === session &&
				generation === current.snapshotGeneration
			)
				error = String(cause);
		}
	}
	$effect(() => {
		void channel.reference.channel;
		if (session) {
			void session.currentHistoryStateId;
			void session.snapshotGeneration;
			void session.status;
			void session.loading.visible;
			void session.hasActiveEditSession;
			void session.editSessionEpoch;
		}
		untrack(() => {
			void refresh();
		});
	});
	onDestroy(() => {
		disposed = true;
		++request;
	});
</script>

<div class="channel-file">
	{#if error}<p role="alert">{error}</p>{/if}
	{#if file}
		<NodeInspector nodes={[file]} level={1} order="solo" labelOverride="File" density="compact" />
	{:else if binding && !binding.parameter}
		<div class="unconfigured-file">
			<span>File</span>
			<FileInput
				enabled={!busy}
				oncommit={(file) =>
					edit({ kind: 'ConfigurePictureFile', reference: channel.reference, file })} />
		</div>
	{:else}<span class="loading">Loading File…</span>{/if}
	{#if chooseExisting && reference}
		<div class="asset-reference" role="group" aria-label="Picture asset binding">
			<NodeInspector nodes={[reference]} level={1} order="solo" density="compact" />
		</div>
	{/if}
</div>

<style>
	.channel-file {
		min-width: 0;
	}
	.unconfigured-file {
		display: grid;
		gap: 0.25rem;
		padding: 0.15rem 0;
		font-size: 0.8em;
	}
	.loading {
		font-size: 0.8em;
		opacity: 0.7;
	}
	.asset-reference {
		padding: 0.3rem 0;
	}
</style>
