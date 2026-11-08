import { appState, type NodeIconSet } from 'golden_ui';
import folder from 'golden_ui/style/icons/node/folder.svg';
import application from 'golden_ui/style/icons/settings.svg';
import screen from './screen.svg';
import surface from './surface.svg';
import media from './nodes/media.svg';
import picture from './nodes/picture.svg';
import solidColor from './nodes/solid_color.svg';
import sharedTexture from './nodes/shared_texture.svg';
import composition from './nodes/composition.svg';
import matrix from './nodes/matrix.svg';
import video from './nodes/video.svg';
import { mediaIconKind } from './media-kind';
import channel from './channel.svg';
import reference from 'golden_ui/style/icons/parameter/reference.svg';

export const mediaIcons = {
	Reference: reference,
	Picture: picture,
	SolidColor: solidColor,
	SharedTexture: sharedTexture,
	Composition: composition,
	Matrix: matrix,
	Video: video,
	Media: media
};
export const panelIcons = { media, screen, surface, composition, matrix };
export const treeIcons: NodeIconSet = {
	resolve: (node) => {
		const kind = mediaIconKind(node, (id) => appState.session?.getNodeData(id));
		return kind ? mediaIcons[kind] : null;
	},
	nodeTypes: {
		'mapgyver.media.solid_color': mediaIcons.SolidColor,
		'mapgyver.media.picture': mediaIcons.Picture,
		'mapgyver.media.video': mediaIcons.Video,
		'mapgyver.media.shared_texture': mediaIcons.SharedTexture,
		'mapgyver.media.composition': mediaIcons.Composition,
		'mapgyver.media.matrix': mediaIcons.Matrix,
		'mapgyver.media.folder': folder,
		'mapgyver.media.v8': media,
		'mapgyver.media.reference.v4': reference,
		'mapgyver.media.channel.v2': channel,
		'mapgyver.media.reference.output': channel,
		'mapgyver.screen': screen,
		'mapgyver.surface': surface,
		'mapgyver.app_control.v1': application
	}
};
