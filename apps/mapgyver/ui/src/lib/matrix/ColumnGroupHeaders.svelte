<script lang="ts">
	import GridLabel from './GridLabel.svelte';
	import { gridSize, groupHeaderRow } from './viewport';
	import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
	let {
		groups,
		requested,
		observation,
		applicationRuntime,
		start,
		end,
		depth,
		height,
		width,
		top,
		left,
		origin,
		columnSize,
		selected,
		disabled,
		select,
		edit,
		drag
	}: {
		groups: ColumnGroupSnapshot[];
		requested: string[];
		observation: MatrixRenderStatus | null;
		applicationRuntime: ApplicationRuntimeSnapshot | null;
		start: number;
		end: number;
		depth: number;
		height: number;
		width: number;
		top: number;
		left: number;
		origin: number;
		columnSize: number;
		selected: string | null;
		disabled: boolean;
		select(group: ColumnGroupSnapshot): void;
		edit(value: MatrixEdit): Promise<unknown>;
		drag(event: DragEvent, group: string): void;
	} = $props();
	const stopping = $derived(
		new Set(
			(applicationRuntime?.applications ?? [])
				.filter((status) => status.phase === 'Stopping' || status.phase === 'RetirementUnverified')
				.flatMap((status) => status.consumers.map((consumer) => consumer.application))
		)
	);
</script>

{#if depth > 0}
	<div
		class="group-headers"
		aria-label="Column Groups"
		style:top={`${top}rem`}
		style:left={`${left + gridSize.label}rem`}
		style:width={`${Math.max(0, width - gridSize.label)}rem`}
		style:height={`${height}rem`}>
		<div class="bands" style:height={`${depth * groupHeaderRow}rem`}>
			{#each groups as group (group.id)}
				{@const first = Math.max(start, group.axis_start)}
				{@const last = Math.min(end, group.axis_start + group.axis_count)}
				<div
					class="band"
					role="columnheader"
					tabindex="0"
					data-column-group-id={group.id}
					data-group-requested={requested.includes(group.id)}
					data-group-installed={observation?.installed?.column_groups.includes(group.id) === true}
					data-group-submitted={observation?.submitted?.column_groups.includes(group.id) === true}
					data-group-stopping={stopping.has(group.id)}
					data-matrix-drop="ColumnGroup"
					draggable={!disabled}
					ondragstart={(event) => drag(event, group.id)}
					style:top={`${group.depth * groupHeaderRow}rem`}
					style:left={`${origin + first * columnSize - left - gridSize.label}rem`}
					style:width={`${(last - first) * columnSize}rem`}>
					<GridLabel
						label={group.label}
						selected={selected === group.id}
						{disabled}
						select={() => select(group)}
						rename={async (label) =>
							Boolean(await edit({ kind: 'RenameColumnGroup', group: group.id, label }))} />
					{#if requested.includes(group.id)}<span
							class="marker requested"
							role="img"
							aria-label="Requested group"
							title="Requested group"></span
						>{/if}
					{#if observation?.submitted?.column_groups.includes(group.id)}<span
							class="marker submitted"
							role="img"
							aria-label="Renderer submitted group content"
							title="Renderer submitted group content"></span
						>{/if}
					{#if stopping.has(group.id)}<span
							class="marker stopping"
							role="img"
							aria-label="Group application stopping"
							title="Group application stopping"></span
						>{/if}
				</div>
			{/each}
		</div>
	</div>
{/if}

<style>
	.marker {
		width: 0.35rem;
		height: 0.35rem;
		flex: none;
		margin-inline-start: 0.3rem;
		border: 0.08rem solid var(--gc-color-focus);
		border-radius: 50%;
	}
	.submitted {
		background: var(--gc-color-node-enabled);
		border-color: var(--gc-color-node-enabled);
	}
	.stopping {
		background: var(--gc-color-warning);
		border-color: var(--gc-color-warning);
		border-radius: 0.05rem;
	}
	.group-headers {
		position: absolute;
		overflow-x: hidden;
		overflow-y: auto;
		z-index: 4;
		background: var(--gc-color-header);
	}
	.bands {
		position: relative;
		min-width: 0;
	}
	.band {
		position: absolute;
		height: 1.8rem;
		display: flex;
		align-items: center;
		padding-inline: 0.3rem;
		box-sizing: border-box;
		border: 0.05rem solid var(--gc-color-border);
		overflow: hidden;
	}
	.band :global(button) {
		max-width: 100%;
	}
</style>
