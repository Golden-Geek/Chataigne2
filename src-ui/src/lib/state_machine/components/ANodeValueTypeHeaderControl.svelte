<script lang="ts">
	import { DropdownEditor, EnableButton, type UiNodeDto } from 'golden_ui';

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
		justify-content: flex-end;
		gap: 0.28rem;
		block-size: 100%;
		min-inline-size: 0;
		max-inline-size: 7.6rem;
		padding-inline: 0.2rem 0.34rem;
	}

	.value-type-header-control :global(.enable-button) {
		flex: 0 0 auto;
		inline-size: 0.56rem;
		block-size: 0.56rem;
		margin: 0;
	}

	.value-type-select {
		display: flex;
		align-items: center;
		min-inline-size: 4.3rem;
		max-inline-size: 6.6rem;
		block-size: 1.12rem;
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
