<script lang="ts">
	import { selectOnPress, viewportContour } from 'golden_ui';
	import GridLabel from './GridLabel.svelte';
	import ParameterTrigger from './ParameterTrigger.svelte';
	import playIcon from 'golden_ui/style/icons/play.svg';
	import previousIcon from '../icons/step-previous.svg';
	import nextIcon from '../icons/step-next.svg';
	import { gridSize, groupHeaderRow } from './viewport';
	import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
	let {
		identity,
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
		isSelected,
		itemColor,
		isEnabled,
		disabled,
		select,
		edit,
		drag
	}: {
		identity: string;
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
		isSelected(uuid: string): boolean;
		itemColor(uuid: string): string | null;
		isEnabled(uuid: string): boolean;
		disabled: boolean;
		select(group: ColumnGroupSnapshot, event: PointerEvent | MouseEvent | KeyboardEvent): void;
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
					use:selectOnPress={{ disabled, select: (event) => select(group, event) }}
					use:viewportContour={{
						selected: isSelected(group.id),
						enabled: isEnabled(group.id),
						identity
					}}
					class:disabled-item={!isEnabled(group.id)}
					aria-selected={isSelected(group.id)}
					style:--matrix-item-color={itemColor(group.id)}
					class:custom-color={itemColor(group.id) != null}
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
						{disabled}
						select={(event) => select(group, event)}
						rename={async (label) =>
							Boolean(await edit({ kind: 'RenameColumnGroup', group: group.id, label }))} />
					<ParameterTrigger
						owner={group.id}
						field="previous_child"
						label={`Previous Child of ${group.label}`}
						icon={previousIcon}
						{disabled}
						tinted />
					<ParameterTrigger
						owner={group.id}
						field="play"
						label={`Play ${group.label}`}
						icon={playIcon}
						{disabled}
						tinted />
					<ParameterTrigger
						owner={group.id}
						field="next_child"
						label={`Next Child of ${group.label}`}
						icon={nextIcon}
						{disabled}
						tinted />
					<ParameterTrigger
						owner={group.id}
						field="stop"
						label={`Stop ${group.label}`}
						{disabled} />
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
	.band.custom-color {
		background: color-mix(in srgb, var(--matrix-item-color) 18%, var(--gc-color-header));
	}
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
	.band :global(.gc-icon-button) {
		inline-size: 1.25rem;
	}
	.band :global(.matrix-tinted-trigger::before) {
		inline-size: 1rem;
		block-size: 1rem;
	}
</style>
