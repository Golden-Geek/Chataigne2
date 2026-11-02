import type { NodeIconSet } from 'golden_ui';
import folder from 'golden_ui/style/icons/node/folder.svg';
import application from 'golden_ui/style/icons/settings.svg';
import screen from './screen.svg';
import surface from './surface.svg';
import media from './media.svg';
import channel from './channel.svg';
import reference from 'golden_ui/style/icons/parameter/reference.svg';

export const treeIcons: NodeIconSet = {
	nodeTypes: {
		'mapgyver.media.folder': folder,
		'mapgyver.media.v5': media,
		'mapgyver.media.reference.v1': reference,
		'mapgyver.media.channel': channel,
		'mapgyver.screen': screen,
		'mapgyver.surface': surface,
		'mapgyver.application': application
	}
};
