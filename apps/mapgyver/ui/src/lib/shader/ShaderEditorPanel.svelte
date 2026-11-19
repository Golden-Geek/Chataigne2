<script lang="ts">
	import { untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState, type UiNodeDto } from 'golden_ui';
	import ParameterInspector from 'golden_ui/components/panels/inspector/ParameterInspector.svelte';
	import { mediaIconKind } from '../icons/media-kind';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import EmbeddedShaderEditor from './EmbeddedShaderEditor.svelte';

	let props: PanelProps = $props();
	let title = $state(untrack(() => props.title));
	export const setPanelState = (next: PanelState) => {
		title = next.title;
	};
	const session = $derived(appState.session);
	const read = (id: number) => session?.getNodeData(id);
	const selected = $derived(session?.selectedNodeId != null ? read(session.selectedNodeId) : null);
	const selectedChannel = $derived(
		selected?.node_type === 'mapgyver.media.channel.v2' ? selected : null
	);
	const parent = $derived(
		selectedChannel ? session?.graph.state.parentById.get(selectedChannel.node_id) : null
	);
	const owner = $derived(parent != null ? read(parent) : selected);
	const media = $derived(owner && mediaIconKind(owner, read) === 'Shader' ? owner : null);
	const channels = $derived(
		(media?.children ?? []).flatMap((id) => {
			const node = read(id);
			return node?.node_type === 'mapgyver.media.channel.v2' ? [node] : [];
		})
	);
	const channel = $derived(media ? (selectedChannel ?? channels[0]) : null);
	const parameters = $derived(
		channel?.children.map(read).find((node) => node?.decl_id === 'parameters')
	);
	const find = (decl: string): UiNodeDto | undefined =>
		parameters?.children
			.map(read)
			.find((node) => node?.decl_id === `parameters/${decl}` && node.data.kind === 'parameter') ??
		undefined;
	const input = $derived(find('shader_input'));
	const format = $derived(find('shader_format'));
	const code = $derived(find('shader_code'));
	const file = $derived(find('shader_file'));
	const fileMode = $derived(
		input?.data.kind === 'parameter' &&
			input.data.param.value.kind === 'enum' &&
			input.data.param.value.value === 'File'
	);
</script>

<section class="workbench-editor shader-panel" aria-label={title} data-panel-id={props.panelId}>
	{#if session && media && channel && input}
		<div class="shader-settings">
			<span class="shader-label" title={media.meta.label}>{media.meta.label}</span>
			{#if channels.length > 1}
				<select
					aria-label="Shader channel"
					value={channel.node_id}
					onchange={(event) => {
						const selectedId = Number(event.currentTarget.value);
						if (channels.some((item) => item.node_id === selectedId))
							session?.selectNode(selectedId);
					}}>
					{#each channels as item (item.uuid)}
						<option value={item.node_id}>{item.meta.label}</option>
					{/each}
				</select>
			{/if}
			<ParameterInspector node={input} level={0} order="solo" density="compact" />
			{#if format}
				<ParameterInspector node={format} level={0} order="solo" density="compact" />
			{/if}
			{#if fileMode && file}
				<ParameterInspector node={file} level={0} order="solo" density="compact" />
			{/if}
		</div>
		<div class="shader-workspace" class:embedded={!fileMode}>
			{#if !fileMode && code}
				{#key `${session.snapshotGeneration}:${code.uuid}`}
					<EmbeddedShaderEditor node={code} {session} />
				{/key}
			{/if}
			<ChannelPreview
				channel={{
					label: channel.meta.label,
					reference: { media: media.uuid, channel: channel.uuid }
				}}
				fill />
		</div>
	{:else}
		<div class="editor-empty">Select a Shader Media or Channel.</div>
	{/if}
</section>

<style>
	.shader-panel {
		container-type: inline-size;
	}
	.shader-settings {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.4rem;
		padding: 0.25rem 0.4rem;
	}
	.shader-label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.shader-workspace {
		flex: 1;
		min-height: 0;
		display: grid;
		gap: 0.4rem;
	}
	.shader-workspace.embedded {
		grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
	}
	@container (width < 30rem) {
		.shader-workspace.embedded {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: minmax(0, 2fr) minmax(0, 1fr);
		}
	}
</style>
