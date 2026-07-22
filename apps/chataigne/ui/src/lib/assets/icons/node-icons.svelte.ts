import type { NodeIconSet } from 'golden_ui';
import moduleCategoryNetworkIcon from './module/network.svg';
import moduleCategoryHardwareIcon from './module/hardware.svg';
import moduleCategoryControllersIcon from './module/controllers.svg';
import moduleCategoryAudioIcon from './module/audio.svg';
import moduleCategoryVideoIcon from './module/video.svg';
import moduleCategoryGeneratorsIcon from './module/generators.svg';
import moduleCategorySystemIcon from './module/system.svg';
import formulaIcon from './formula.svg';
import formulaLibraryIcon from './formula_library.svg';

const nodeIconModules = import.meta.glob('./nodes/*.{svg,png}', {
	eager: true,
	import: 'default'
}) as Record<string, string>;

const nodeIcons = Object.fromEntries(
	Object.entries(nodeIconModules).map(([path, iconUrl]) => {
		const fileName = path.split('/').at(-1) ?? '';
		return [fileName.replace(/\.[^.]+$/, ''), iconUrl];
	})
);

export const appIcons: NodeIconSet = {
	nodeTypes: {
		...nodeIcons,
		alchemist_formula: formulaIcon,
		alchemist_formula_library: formulaLibraryIcon
	},
	categories: {
		Network: moduleCategoryNetworkIcon,
		Hardware: moduleCategoryHardwareIcon,
		Controllers: moduleCategoryControllersIcon,
		Audio: moduleCategoryAudioIcon,
		Video: moduleCategoryVideoIcon,
		System: moduleCategorySystemIcon,
		Generators: moduleCategoryGeneratorsIcon
	}
};
