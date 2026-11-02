import { appState, registerNodeContextMenuContributor } from 'golden_ui';
import { invokeMediaEdit } from './operations';
import type { ChannelMoveDirection } from './generated/ChannelMoveDirection';

export function registerChannelContextMenu() {
	registerNodeContextMenuContributor('mapgyver.channel-order', {
		match: ({ node }) => node.node_type === 'mapgyver.media.channel',
		items: ({ node, parentNode, session, closeMenu }) => {
			if (
				!parentNode?.creatable_user_items?.some(
					(item) => item.node_type === 'mapgyver.media.channel'
				)
			)
				return [];
			const reference = { media: parentNode.uuid, channel: node.uuid };
			const generation = session?.snapshotGeneration;
			const move = (direction: ChannelMoveDirection) => {
				closeMenu();
				if (
					!session ||
					appState.session !== session ||
					session.snapshotGeneration !== generation ||
					session.getNodeData(node.node_id)?.uuid !== node.uuid
				)
					return;
				void invokeMediaEdit(session, { kind: 'MoveChannel', reference, direction }).catch(
					(error) => console.error('Channel move rejected', error)
				);
			};
			return [
				{
					id: 'mapgyver-channel-earlier',
					label: 'Move earlier',
					disabled:
						session?.status !== 'connected' ||
						session.loading.visible ||
						session.hasActiveEditSession,
					action: () => move('Earlier')
				},
				{
					id: 'mapgyver-channel-later',
					label: 'Move later',
					disabled:
						session?.status !== 'connected' ||
						session.loading.visible ||
						session.hasActiveEditSession,
					action: () => move('Later')
				}
			];
		}
	});
}
