<script lang="ts">
	import type { UiNodeDto } from '../../../types';
	let { targetNode, insideLabel }: { targetNode: UiNodeDto; insideLabel?: string } = $props();
</script>

<div data-fixture-node-content>
	{insideLabel ?? ''} Registered renderer: {targetNode.meta.label}
</div>
