/** Presentation only: keep ordinary widgets and expose the fields relevant to this owner. */
import { appState, registerNodeInspectorMatcher, type UiNodeDto } from 'golden_ui';
import { COMMAND_PARAMETER_FIELDS } from './media/generated/command-fields';
import { ASSET_NODE_TYPE } from './media/generated/constants';

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
	if (
		field === 'fit_mode' &&
		(owner.node_type === 'mapgyver.media.v8' || owner.node_type === 'mapgyver.media.reference.v4')
	) {
		return folder.children.some((id) => {
			const parameter = session.getNodeData(id);
			return parameter?.decl_id === 'parameters/resolution' && parameter.meta.enabled;
		});
	}
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
	if (owner.node_type === 'mapgyver.app_control.v1') {
		function value(name: string) {
			const parameter = folder!.children
				.map((id) => session!.getNodeData(id))
				.find((node) => node?.decl_id === `parameters/${name}`);
			return parameter?.data.kind === 'parameter' ? parameter.data.param.value : null;
		}
		if (field === 'app_file') return true;
		const file = value('app_file');
		if (file?.kind !== 'file' || !file.value) return false;
		if (['sender', 'first_frame', 'timeout_ms'].includes(field ?? ''))
			return choice('readiness') === 'Spout';
		if (['initial_ms', 'maximum_ms'].includes(field ?? '')) {
			const enabled = value('restart_enabled');
			return enabled?.kind === 'bool' && enabled.value;
		}
		return true;
	}
	if (owner.node_type === ASSET_NODE_TYPE) return field === 'file';
	if (owner.node_type === 'mapgyver.media.command.v6') {
		if (field === 'command_kind' || field === 'trigger') return true;
		const key = choice('command_kind') as keyof typeof COMMAND_PARAMETER_FIELDS;
		const fields: readonly string[] = COMMAND_PARAMETER_FIELDS[key] ?? [];
		if (!fields.includes(field ?? '')) return false;
		if (field === 'project_path') return choice('locator_kind') === 'ProjectRelative';
		if (field === 'absolute_path') return choice('locator_kind') === 'Absolute';
		return true;
	}
	if (owner.node_type === 'mapgyver.screen') {
		if (field === 'target_kind' || field === 'anti_aliasing' || field === 'frame_rate_mode')
			return true;
		if (field === 'frames_per_second') return choice('frame_rate_mode') === 'Explicit';
		const target = choice('target_kind');
		if (target === 'Fullscreen') return field === 'display';
		if (target === 'Rectangle') return ['left', 'top', 'width', 'height'].includes(field ?? '');
		if (target === 'Shared Texture') return ['sender', 'width', 'height'].includes(field ?? '');
		return false;
	}
	if (owner.node_type === 'mapgyver.media.v8') {
		if (field === 'media_kind') return false;
		return (
			!['empty_cell', 'transition_ms', 'transition_curve'].includes(field ?? '') ||
			choice('media_kind') === 'Matrix'
		);
	}
	if (owner.node_type !== 'mapgyver.media.channel.v2') return true;
	if (field === 'source_kind') return false;
	const source = choice('source_kind');
	if (source === 'SolidColor') return field === 'rgba';
	if (source === 'Picture') return field === 'file';
	if (source === 'SharedTexture') {
		const media = parent(owner);
		const parameters = media?.children
			.map((id) => session.getNodeData(id))
			.find((node) => node?.decl_id === 'parameters');
		const mode = parameters?.children
			.map((id) => session.getNodeData(id))
			.find((node) => node?.decl_id === 'parameters/sender_selection');
		const auto =
			mode?.data.kind === 'parameter' &&
			mode.data.param.value.kind === 'enum' &&
			mode.data.param.value.value === 'Auto';
		return field === (auto ? 'sender_choice' : 'sender');
	}
	if (source === 'Composition') return false;
	if (source === 'Matrix') {
		if (field === 'output_kind') return true;
		return choice('output_kind') !== 'Final' && field === 'output_item';
	}
	return false;
}

export function registerParameterVisibility() {
	registerNodeInspectorMatcher(
		'mapgyver.channel-parameters',
		(node) =>
			node.node_type === 'folder' &&
			node.decl_id === 'parameters' &&
			parent(node)?.node_type === 'mapgyver.media.channel.v2',
		{ childFilter: visible, inlineChildren: true, childDensity: 'compact' }
	);
	registerNodeInspectorMatcher(
		'mapgyver.parameter-visibility',
		(node) =>
			node.node_type === 'folder' &&
			node.decl_id === 'parameters' &&
			!!parent(node)?.node_type.startsWith('mapgyver.'),
		{ childFilter: visible }
	);
}
