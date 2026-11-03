<script lang="ts">
	import { IconButton } from 'golden_ui';
	import referenceIcon from 'golden_ui/style/icons/parameter/reference.svg';
	import { useDocumentSelection } from '../selection.svelte';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	let { column, disabled }: { column: ColumnSnapshot; disabled: boolean } = $props();
	const selection = useDocumentSelection();
</script>

<IconButton
	icon={referenceIcon}
	label="Inspect Column settings"
	{disabled}
	onclick={() => selection.select(column.id)} />
