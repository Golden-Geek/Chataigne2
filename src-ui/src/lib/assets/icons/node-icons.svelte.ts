import type { NodeIconSet } from 'golden_ui';
import moduleIcon from './nodes/module.svg';
import moduleManagerIcon from './nodes/module_manager.svg';

export const appIcons: NodeIconSet = {
	nodeTypes: {
		module_manager: moduleManagerIcon,
		module: moduleIcon
	}
};
