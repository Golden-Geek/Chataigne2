import type { NodeIconSet } from 'golden_ui';
import moduleIcon from './nodes/module.svg';
import moduleManagerIcon from './nodes/module_manager.svg';
import moduleCategoryNetworkIcon from './module/network.svg';
import moduleCategoryHardwareIcon from './module/hardware.svg';
import moduleCategoryAudioIcon from './module/audio.svg';
import moduleCategoryVideoIcon from './module/video.svg';
import moduleCategorySystemIcon from './module/system.svg';

export const appIcons: NodeIconSet = {
	nodeTypes: {
		module_manager: moduleManagerIcon,
		module: moduleIcon
	},
	categories: {
		Network: moduleCategoryNetworkIcon,
		Hardware: moduleCategoryHardwareIcon,
		Audio: moduleCategoryAudioIcon,
		Video: moduleCategoryVideoIcon,
		System: moduleCategorySystemIcon
	}
};
