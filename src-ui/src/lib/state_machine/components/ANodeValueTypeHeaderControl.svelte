<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import EnableButton from 'golden_ui/components/common/EnableButton.svelte';
	import DropdownEditor from 'golden_ui/components/panels/inspector/parameters/DropdownEditor.svelte';

	let { parameter }: { parameter: UiNodeDto } = $props();
</script>

<div
	class="value-type-header-control"
	data-no-node-select
	role="group"
	aria-label="Value Type"
	title="Value Type"
	onpointerdown={(event) => event.stopPropagation()}>
	<EnableButton node={parameter} />
	<div class="value-type-select">
		<DropdownEditor node={parameter} />
	</div>
</div>

<style>
	.value-type-header-control {
		display: flex;
		align-items: center;
		gap: 0.28rem;
		min-inline-size: 0;
		max-inline-size: 7.6rem;
		padding-inline: 0.2rem 0.34rem;
	}

	.value-type-select {
		min-inline-size: 4.3rem;
		max-inline-size: 6.6rem;
		block-size: 1.05rem;
	}

	.value-type-select :global(.dropdown-editor) {
		block-size: 100%;
		inline-size: 100%;
		min-inline-size: 0;
		padding: 0 0.22rem;
		border-radius: 0.28rem;
		font-size: 0.62rem;
	}
</style>
