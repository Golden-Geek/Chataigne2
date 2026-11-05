<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import { usePreview } from '../preview/context.svelte';
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import { cellPreviewGroups } from './cell-previews';
	import { gridSize, groupHeaderHeight, type Metrics } from './viewport';
	let { snapshot, metrics }: { snapshot: MatrixViewportSnapshot; metrics: Metrics } = $props();
	const owner = usePreview();
	const groups = $derived(cellPreviewGroups(snapshot, metrics));
	const visible = $derived(groups.length > 0);
	const source = $derived(
		JSON.stringify({ kind: 'Channel', reference: snapshot.final_output.reference })
	);
</script>

<div
	class="cell-previews"
	data-matrix-preview-source-count={groups.length}
	aria-hidden="true"
	style:width={`${metrics.width}px`}
	style:height={`${metrics.height}px`}>
	<div
		class="data-area"
		style:inset={`${gridSize.header + groupHeaderHeight(metrics, snapshot.axis_depth)}rem 0 0 ${gridSize.label}rem`}>
		<div class="source-group">
			<NativePreview
				session={owner.session}
				{source}
				tileGroups={groups}
				{visible}
				lifetime={snapshot.project}
				label="Matrix Cell previews" />
		</div>
	</div>
</div>

<style>
	.cell-previews {
		/* The measured overlay must not contribute to the scroll extent it measures. */
		position: absolute;
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
