import type { NodeIconSet } from 'golden_ui';
import moduleIcon from './nodes/module.svg';
import moduleManagerIcon from './nodes/module_manager.svg';
import moduleCategoryNetworkIcon from './module/network.svg';
import moduleCategoryHardwareIcon from './module/hardware.svg';
import moduleCategoryControllersIcon from './module/controllers.svg';
import moduleCategoryAudioIcon from './module/audio.svg';
import moduleCategoryVideoIcon from './module/video.svg';
import moduleCategorySystemIcon from './module/system.svg';
import formulaIcon from '../../golden_alchemist_ui/icons/formula.svg';
import formulaLibraryIcon from '../../golden_alchemist_ui/icons/formula_library.svg';

export const appIcons: NodeIconSet = {
	nodeTypes: {
		module_manager: moduleManagerIcon,
		module: moduleIcon,
		state_processor: formulaIcon,
		alchemist_formula: formulaIcon,
		alchemist_formula_library: formulaLibraryIcon
	},
	categories: {
		Network: moduleCategoryNetworkIcon,
		Hardware: moduleCategoryHardwareIcon,
		Controllers: moduleCategoryControllersIcon,
		Audio: moduleCategoryAudioIcon,
		Video: moduleCategoryVideoIcon,
		System: moduleCategorySystemIcon
	}
};
