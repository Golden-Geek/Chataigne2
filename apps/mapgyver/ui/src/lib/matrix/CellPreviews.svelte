<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import { usePreview } from '../preview/context.svelte';
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import { cellPreviewGroups } from './cell-previews';
	import { gridSize, type Metrics } from './viewport';
	let { snapshot, metrics }: { snapshot: MatrixViewportSnapshot; metrics: Metrics } = $props();
	const owner = usePreview();
	const groups = $derived(cellPreviewGroups(snapshot, metrics));
</script>

<div
	class="cell-previews"
	aria-hidden="true"
	style:width={`${metrics.width}px`}
	style:height={`${metrics.height}px`}
	style:margin-bottom={`-${metrics.height}px`}>
	<div class="data-area" style:inset={`${gridSize.header}rem 0 0 ${gridSize.label}rem`}>
		{#each groups as group (group.source)}
			<div class="source-group" data-matrix-preview-source={group.source}>
				<NativePreview
					session={owner.session}
					source={group.source}
					tiles={group.tiles}
					tileCapacity={group.capacity}
					lifetime={snapshot.project}
					label="Matrix Cell previews" />
			</div>
		{/each}
	</div>
</div>

<style>
	.cell-previews {
		position: sticky;
		top: 0;
		left: 0;
		pointer-events: none;
	}
	.data-area,
	.source-group {
		position: absolute;
		overflow: hidden;
	}
	.source-group {
		inset: 0;
	}
</style>
