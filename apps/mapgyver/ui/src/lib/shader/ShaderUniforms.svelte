<script lang="ts">
	import { IconButton, type UiNodeDto, type WorkbenchSession } from 'golden_ui';
	import ParameterInspector from 'golden_ui/components/panels/inspector/ParameterInspector.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { invokeMediaEdit } from '../media/operations';
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import type { ShaderUniformRow } from '../media/generated/ShaderUniformRow';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';

	let {
		session,
		reference,
		sourceNode,
		fileMode,
		format,
		folder
	}: {
		session: WorkbenchSession;
		reference: ChannelReference;
		sourceNode: UiNodeDto | undefined;
		fileMode: boolean;
		format: UiNodeDto | undefined;
		folder: UiNodeDto | null | undefined;
	} = $props();

	const source = $derived(
		sourceNode?.data.kind === 'parameter' ? sourceNode.data.param.value : null
	);
	const preview = usePreview();
	const fileRevision = $derived(
		fileMode
			? (preview.statuses.get(previewKey({ kind: 'Channel', reference }))?.shader_file_revision ??
					null)
			: null
	);
	const dialect = $derived(format?.data.kind === 'parameter' ? format.data.param.value : null);
	const membership = $derived(folder?.children.join(',') ?? '');
	let rows = $state<ShaderUniformRow[]>([]);
	let error = $state('');
	let busy = $state(false);
	let expanded = $state(false);
	let revision = $state<string | null>(null);

	$effect(() => {
		const generation = session.snapshotGeneration;
		const current = reference;
		// Refresh after source changes, file preparation, Add and Undo.
		void source;
		void dialect;
		void membership;
		void fileRevision;
		let stale = false;
		void invokeMediaEdit(session, { kind: 'InspectShaderUniforms', reference: current }).then(
			(result) => {
				if (stale || session.snapshotGeneration !== generation) return;
				if (result.kind === 'ShaderUniforms') {
					rows = result.rows;
					revision = result.revision;
				}
				error = '';
			},
			(problem: unknown) => {
				if (stale || session.snapshotGeneration !== generation) return;
				rows = [];
				revision = null;
				error = problem instanceof Error ? problem.message : String(problem);
			}
		);
		return () => {
			stale = true;
		};
	});

	async function add(name: string | null) {
		if (busy) return;
		busy = true;
		error = '';
		try {
			await invokeMediaEdit(session, { kind: 'AddShaderUniforms', reference, name, revision });
		} catch (problem) {
			error = problem instanceof Error ? problem.message : String(problem);
		} finally {
			busy = false;
		}
	}
</script>

{#if rows.length || error}
	<details class="uniforms" bind:open={expanded}>
		<summary>Uniforms</summary>
		<div class="rows">
			{#if rows.some((row) => row.parameter === null)}
				<button class="add-all" type="button" disabled={busy} onclick={() => void add(null)}>
					Add All
				</button>
			{/if}
			{#each rows as row (row.name)}
				{@const node = row.parameter === null ? null : session.getNodeData(row.parameter)}
				{#if node}
					<ParameterInspector {node} level={0} order="solo" density="compact" />
				{:else if row.parameter === null}
					<div class="pending">
						<span title={row.name}>{row.label}</span>
						<IconButton
							icon={addIcon}
							label={`Add uniform ${row.name}`}
							disabled={busy}
							onclick={() => void add(row.name)} />
					</div>
				{/if}
			{/each}
			{#if error}<span role="status">{error}</span>{/if}
		</div>
	</details>
{/if}

<style>
	.uniforms {
		padding: 0.25rem 0.4rem;
	}

	summary {
		cursor: pointer;
		font-size: 0.8rem;
	}

	.rows {
		display: grid;
		align-content: start;
		gap: 0.25rem;
		max-height: 12rem;
		padding-block: 0.25rem;
		overflow: auto;
	}

	.add-all {
		justify-self: end;
	}

	.pending {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}
</style>
