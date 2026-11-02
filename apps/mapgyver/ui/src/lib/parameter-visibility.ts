/** Presentation only: keep ordinary widgets and expose the fields relevant to this owner. */
import { appState, registerNodeInspectorMatcher, type UiNodeDto } from 'golden_ui';
import { COMMAND_PARAMETER_FIELDS } from './media/generated/command-fields';

function parent(node: UiNodeDto) {
	const session = appState.session;
	const id = session?.graph.state.parentById.get(node.node_id);
	return id == null ? null : session?.getNodeData(id);
}

function visible(child: UiNodeDto) {
	const folder = parent(child),
		owner = folder && parent(folder),
		session = appState.session;
	if (!folder || !owner || !session) return true;
	const field = child.decl_id.split('/').at(-1);
	function choice(name: string) {
		for (const id of folder!.children) {
			const node = session!.getNodeData(id);
			if (node?.decl_id === `parameters/${name}` && node.data.kind === 'parameter') {
				const value = node.data.param.value;
				return value.kind === 'enum' ? value.value : null;
			}
		}
		return null;
	}
	if (owner.node_type === 'mapgyver.media.asset') {
		return (
			field === 'locator_kind' ||
			field ===
				(choice('locator_kind') === 'ProjectRelative'
					? 'project_path'
					: choice('locator_kind') === 'Absolute'
						? 'absolute_path'
						: '')
		);
	}
	if (owner.node_type === 'mapgyver.media.command.v2') {
		if (field === 'command_kind' || field === 'trigger') return true;
		const key = choice('command_kind') as keyof typeof COMMAND_PARAMETER_FIELDS;
		const fields: readonly string[] = COMMAND_PARAMETER_FIELDS[key] ?? [];
		if (!fields.includes(field ?? '')) return false;
		if (field === 'project_path') return choice('locator_kind') === 'ProjectRelative';
		if (field === 'absolute_path') return choice('locator_kind') === 'Absolute';
		return true;
	}
	if (owner.node_type === 'mapgyver.screen') {
		if (field === 'target_kind') return true;
		const target = choice('target_kind');
		if (target === 'Fullscreen') return field === 'display';
		if (target === 'Rectangle') return ['left', 'top', 'width', 'height'].includes(field ?? '');
		if (target === 'Shared Texture') return ['sender', 'width', 'height'].includes(field ?? '');
		return false;
	}
	if (owner.node_type === 'mapgyver.media.v5') {
		return (
			!['empty_cell', 'transition_ms', 'transition_curve'].includes(field ?? '') ||
			choice('media_kind') === 'Matrix'
		);
	}
	if (owner.node_type !== 'mapgyver.media.channel') return true;
	if (field === 'source_kind') return true;
	const source = choice('source_kind');
	if (source === 'SolidColor') return field === 'rgba';
	if (source === 'Picture') return field === 'asset';
	if (source === 'SharedTexture') return field === 'sender';
	if (source === 'Composition') return false;
	if (source === 'Matrix') {
		if (field === 'output_kind') return true;
		return choice('output_kind') !== 'Final' && field === 'output_item';
	}
	return false;
}

export function registerParameterVisibility() {
	registerNodeInspectorMatcher(
		'mapgyver.parameter-visibility',
		(node) =>
			node.node_type === 'folder' &&
			node.decl_id === 'parameters' &&
			!!parent(node)?.node_type.startsWith('mapgyver.'),
		{ childFilter: visible }
	);
}
