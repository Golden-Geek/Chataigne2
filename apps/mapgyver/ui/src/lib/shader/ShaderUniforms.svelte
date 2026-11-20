<script lang="ts">
	import { IconButton, type UiNodeDto, type WorkbenchSession } from 'golden_ui';
	import ParameterInspector from 'golden_ui/components/panels/inspector/ParameterInspector.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { invokeMediaEdit } from '../media/operations';
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import type { ShaderUniformRow } from '../media/generated/ShaderUniformRow';

	let {
		session,
		reference,
		code,
		format,
		folder
	}: {
		session: WorkbenchSession;
		reference: ChannelReference;
		code: UiNodeDto | undefined;
		format: UiNodeDto | undefined;
		folder: UiNodeDto | null | undefined;
	} = $props();

	const source = $derived(code?.data.kind === 'parameter' ? code.data.param.value : null);
	const dialect = $derived(format?.data.kind === 'parameter' ? format.data.param.value : null);
	const membership = $derived(folder?.children.join(',') ?? '');
	let rows = $state<ShaderUniformRow[]>([]);
	let error = $state('');
	let busy = $state(false);

	$effect(() => {
		const generation = session.snapshotGeneration;
		const current = reference;
		// Refresh after code changes, additions and Undo. Parameter value changes need no query.
		void source;
		void dialect;
		void membership;
		let stale = false;
		void invokeMediaEdit(session, { kind: 'InspectShaderUniforms', reference: current }).then(
			(result) => {
				if (stale || session.snapshotGeneration !== generation) return;
				if (result.kind === 'ShaderUniforms') rows = result.rows;
				error = '';
			},
			(problem: unknown) => {
				if (stale || session.snapshotGeneration !== generation) return;
				rows = [];
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
			await invokeMediaEdit(session, { kind: 'AddShaderUniforms', reference, name });
		} catch (problem) {
			error = problem instanceof Error ? problem.message : String(problem);
		} finally {
			busy = false;
		}
	}
</script>

{#if rows.length || error}
	<details class="uniforms">
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
