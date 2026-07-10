<script lang="ts">
	let {
		checked,
		onchange
	}: {
		checked: boolean;
		onchange?: (checked: boolean) => void;
	} = $props();

	const handleChange = (event: Event): void => {
		onchange?.((event.currentTarget as HTMLInputElement).checked);
	};
</script>

<label class="auto-wire-toggle" title="Automatically route wires around nodes">
	<input type="checkbox" {checked} onchange={handleChange} />
	<span>Auto Wire</span>
</label>

<style>
	.auto-wire-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.28rem;
		min-block-size: 1.45rem;
		padding-inline: 0.4rem;
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-background) 84%, transparent);
		color: color-mix(in srgb, var(--gc-color-text) 78%, transparent);
		font-size: 0.66rem;
		cursor: pointer;
		backdrop-filter: blur(0.5rem);
	}

	.auto-wire-toggle input {
		inline-size: 0.8rem;
		block-size: 0.8rem;
		margin: 0;
	}
</style>
