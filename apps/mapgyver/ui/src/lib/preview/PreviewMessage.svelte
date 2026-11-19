<script lang="ts">
	let { message }: { message: string } = $props();
</script>

<div class="message" role="status"><span>{message}</span></div>

<style>
	.message {
		position: absolute;
		inset: auto 0.5rem 0.5rem;
		display: flex;
		justify-content: center;
		pointer-events: none;
	}
	span {
		padding: 0.25em 0.55em;
		border-radius: 0.25em;
		background: #161616e8;
		color: #fff;
		font-size: 0.85em;
		overflow-wrap: anywhere;
	}
</style>
