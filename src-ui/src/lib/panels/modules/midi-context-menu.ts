import type { NodeContextMenuContributorEntry } from 'golden_ui';

const MIDI_CC_ROTARY_TAG_PREFIX = 'midi:cc:rotary:';
const ROTARY_ABSOLUTE = 'absolute';
const MIDI_CC_ROTARY_OPTIONS = [
	{ id: ROTARY_ABSOLUTE, label: 'Absolute' },
	{ id: 'twos_complement', label: "Two's Complement" },
	{ id: 'binary_offset', label: 'Binary Offset' },
	{ id: 'sign_magnitude', label: 'Sign Magnitude' }
] as const;

const midiCcControllerFromDeclId = (declId: string): number | null => {
	const match = /^cc_(\d+)$/.exec(declId);
	if (!match) {
		return null;
	}
	const controller = Number(match[1]);
	return Number.isInteger(controller) && controller >= 0 && controller <= 127 ? controller : null;
};

const stripMidiCcConfigTags = (tags: readonly string[]): string[] => {
	return tags.filter((tag) => !tag.startsWith(MIDI_CC_ROTARY_TAG_PREFIX));
};

const midiCcConfigFromTags = (tags: readonly string[]): { rotaryMode: string } => {
	const rotaryMode =
		tags.find((tag) => tag.startsWith(MIDI_CC_ROTARY_TAG_PREFIX))?.slice(
			MIDI_CC_ROTARY_TAG_PREFIX.length
		) ?? ROTARY_ABSOLUTE;
	return { rotaryMode };
};

export const midiCcContextMenuContributor = {
	match: ({ node, parentNode }) => {
		return (
			node.data.kind === 'parameter' &&
			parentNode?.decl_id === 'control_change' &&
			midiCcControllerFromDeclId(node.decl_id) !== null
		);
	},
	items: ({ node, patchMeta, closeMenu }) => {
		const config = midiCcConfigFromTags(node.meta.tags);
		return [
			{
				id: 'midi-cc-options',
				label: 'MIDI CC Options',
				submenu: MIDI_CC_ROTARY_OPTIONS.map((option) => ({
					id: `midi-cc-rotary-${option.id}`,
					label: option.label,
					hint: config.rotaryMode === option.id ? 'Current' : undefined,
					action: () => {
						const nextTags = stripMidiCcConfigTags(node.meta.tags);
						if (option.id !== ROTARY_ABSOLUTE) {
							nextTags.push(`${MIDI_CC_ROTARY_TAG_PREFIX}${option.id}`);
						}
						void patchMeta(node.node_id, { tags: nextTags });
						closeMenu();
					}
				}))
			}
		];
	}
} satisfies NodeContextMenuContributorEntry;