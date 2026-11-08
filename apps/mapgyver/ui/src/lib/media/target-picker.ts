import { registerReferenceChoiceProvider, type UiNodeDto, type WorkbenchSession } from 'golden_ui';
import { invokeMediaEdit } from './operations';
import { MEDIA_NAMESPACE } from './generated/constants';

function matches(node: UiNodeDto, session: WorkbenchSession) {
	const parents = session.graph.state.parentById;
	const folder = session.getNodeData(parents.get(node.node_id) ?? -1);
	const owner = folder && session.getNodeData(parents.get(folder.node_id) ?? -1);
	return (
		folder?.decl_id === 'parameters' &&
		((owner?.node_type === 'mapgyver.media.reference.v4' &&
			['parameters/target', 'parameters/channel'].includes(node.decl_id)) ||
			(owner?.node_type === 'mapgyver.surface' &&
				['parameters/source_media', 'parameters/source_channel'].includes(node.decl_id)))
	);
}
export function registerMediaTargetPicker() {
	return registerReferenceChoiceProvider({
		id: 'mapgyver.media-target',
		matches,
		revision: (session) => session.getCustomEventSequence(MEDIA_NAMESPACE),
		async read(session, node, request) {
			const result = await invokeMediaEdit(session, {
				kind: 'InspectTargetPicker',
				query: {
					parameter: node.uuid,
					folder: request.location,
					cursor: request.cursor?.key ?? null,
					backwards: request.cursor?.direction === 'previous',
					limit: request.expanded ? 64 : 0
				}
			});
			if (result.kind !== 'TargetPicker') throw new Error('Unexpected target picker result');
			const page = result.page;
			if (!page) return null;
			return {
				name: page.location,
				location: page.folder,
				parent: page.parent,
				previous: page.before,
				next: page.after,
				emptyName: page.empty_label,
				selection: page.current && { name: page.current.label, missing: !page.current.present },
				choices: page.entries.map((entry) => ({
					key: entry.id,
					name: entry.label,
					browse: entry.folder
				}))
			};
		},
		async select(session, node, key) {
			await invokeMediaEdit(session, {
				kind: 'SelectTargetPickerChoice',
				parameter: node.uuid,
				target: key
			});
		}
	});
}
