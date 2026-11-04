<script lang="ts">
	import {
		appState,
		ContextMenu,
		IconButton,
		type ContextMenuItem,
		type UiNodeDto
	} from 'golden_ui';
	import detectIcon from 'golden_ui/style/icons/select-node.svg';
	let { node, readOnly }: { node: UiNodeDto; readOnly: boolean } = $props();
	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	const session = $derived(appState.session);
	const choices = $derived.by(() => {
		const parentId = session?.graph.state.parentById.get(node.node_id);
		const folder = parentId == null ? null : session?.getNodeData(parentId);
		const selector = folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters/sender_choice');
		return selector?.data.kind === 'parameter'
			? (selector.data.param.constraints.enum_options ?? [])
			: [];
	});
	const items: ContextMenuItem[] = $derived(
		choices
			.filter(
				(choice: { variant_id: string; tags?: string[] }) =>
					choice.variant_id && choice.tags?.includes('available')
			)
			.map((choice: { variant_id: string; label: string }) => ({
				id: choice.variant_id,
				label: choice.label,
				disabled: readOnly,
				action: async () => {
					if (readOnly || session?.getNodeData(node.node_id)?.uuid !== node.uuid) return;
					await session.sendIntent({
						kind: 'setParam',
						node: node.node_id,
						value: { kind: 'str', value: choice.variant_id },
						behaviour: 'Append'
					});
				}
			}))
	);
</script>

<IconButton
	icon={detectIcon}
	label="Detect sender"
	disabled={readOnly}
	onclick={(event) => {
		trigger = event.currentTarget;
		open = !open;
	}} />
<ContextMenu
	bind:open
	items={items.length ? items : [{ label: 'No senders available', disabled: true }]}
	anchor={{ kind: 'element', element: trigger, placement: 'bottom-end' }}
	insideElements={trigger ? [trigger] : []} />
