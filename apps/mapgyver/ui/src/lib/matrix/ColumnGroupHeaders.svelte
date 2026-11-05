<script lang="ts">
	import GridLabel from './GridLabel.svelte';
	import { gridSize, groupHeaderRow } from './viewport';
	import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	let {
		groups,
		start,
		end,
		depth,
		height,
		width,
		top,
		left,
		origin,
		selected,
		disabled,
		select,
		edit,
		drag
	}: {
		groups: ColumnGroupSnapshot[];
		start: number;
		end: number;
		depth: number;
		height: number;
		width: number;
		top: number;
		left: number;
		origin: number;
		selected: string | null;
		disabled: boolean;
		select(group: ColumnGroupSnapshot): void;
		edit(value: MatrixEdit): Promise<unknown>;
		drag(event: DragEvent, group: string): void;
	} = $props();
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
					data-matrix-drop="ColumnGroup"
					draggable={!disabled}
					ondragstart={(event) => drag(event, group.id)}
					style:top={`${group.depth * groupHeaderRow}rem`}
					style:left={`${origin + first * gridSize.column - left - gridSize.label}rem`}
					style:width={`${(last - first) * gridSize.column}rem`}>
					<GridLabel
						label={group.label}
						selected={selected === group.id}
						{disabled}
						select={() => select(group)}
						rename={async (label) =>
							Boolean(await edit({ kind: 'RenameColumnGroup', group: group.id, label }))} />
				</div>
			{/each}
		</div>
	</div>
{/if}

<style>
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
