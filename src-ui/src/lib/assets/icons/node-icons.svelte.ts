import type { NodeIconSet } from '$lib/golden_ui';
import { generateIconWithText } from '$lib/golden_ui/store/node-types';
import moduleIcon from './nodes/module.svg';
import moduleManagerIcon from './nodes/module_manager.svg';

const oscIcon = generateIconWithText('OSC', '#4CAF50');
const midiIcon = generateIconWithText('MIDI', '#FF5722');

export const appIcons: NodeIconSet = {
	nodeTypes: {
		module_manager: moduleManagerIcon,
		module: moduleIcon,
		osc_module: oscIcon,
		midi_module: midiIcon
	}
};
