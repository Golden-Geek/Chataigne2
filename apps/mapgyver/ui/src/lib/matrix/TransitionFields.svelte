<script lang="ts">
	import type { TransitionDefinition } from '../media/generated/TransitionDefinition';
	let { value }: { value: TransitionDefinition } = $props();
</script>

<label
	>Duration (ms)<input
		name="duration"
		type="number"
		min="0"
		max="4294967295"
		step="1"
		required
		value={value.duration_ms} /></label>
<label
	>Curve<select name="curve" value={value.curve}
		><option>Linear</option><option>SmoothStep</option></select
	></label>
