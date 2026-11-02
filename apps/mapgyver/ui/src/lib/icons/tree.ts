import type { NodeIconSet } from 'golden_ui';
import folder from 'golden_ui/style/icons/node/folder.svg';
import application from 'golden_ui/style/icons/settings.svg';
import screen from './screen.svg';
import surface from './surface.svg';
import media from './media.svg';
import channel from './channel.svg';

export const treeIcons: NodeIconSet = {
	nodeTypes: {
		'mapgyver.media.folder': folder,
		'mapgyver.media.v3': media,
		'mapgyver.media.channel': channel,
		'mapgyver.screen': screen,
		'mapgyver.surface': surface,
		'mapgyver.application': application
	}
};
