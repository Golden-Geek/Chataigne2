import { appState, registerParameterAdornment } from 'golden_ui';
import SenderDetect from './SenderDetect.svelte';

export function registerSenderAdornment() {
	return registerParameterAdornment({
		id: 'mapgyver.sender-detect',
		component: SenderDetect,
		matches(node, nodes) {
			if (
				node.decl_id !== 'parameters/sender' ||
				node.data.kind !== 'parameter' ||
				node.data.param.value.kind !== 'str'
			)
				return false;
			const parents = appState.session?.graph.state.parentById;
			const folderId = parents?.get(node.node_id);
			const folder = folderId == null ? null : nodes.get(folderId);
			const channelId = folder && parents?.get(folder.node_id);
			const channel = channelId == null ? null : nodes.get(channelId);
			return (
				channel?.node_type === 'mapgyver.media.channel.v2' &&
				folder?.children.some((id) => nodes.get(id)?.decl_id === 'parameters/sender_choice') ===
					true
			);
		}
	});
}
