import { platform } from './platform.svelte';

export type CommandId =
	| 'edit.deleteSelection'
	| 'view.frame'
	| 'view.home'
	| 'dashboard.toggleEditMode'
	| 'select.all'
	| 'edit.copy'
	| 'edit.cut'
	| 'edit.duplicate'
	| 'edit.paste'
	| 'edit.undo'
	| 'edit.redo'
	| 'file.new'
	| 'file.save'
	| 'file.saveAs'
	| 'file.open'
	| 'file.reopenLast'
	| 'file.preferences';

export interface CommandExecuteContext {
	source: 'keyboard' | 'menu' | 'api';
	event?: KeyboardEvent | null;
}

type CommandHandlerResult = boolean | void | Promise<boolean | void>;
type CommandHandler = (context: CommandExecuteContext) => CommandHandlerResult;

interface CommandHandlerEntry {
	id: number;
	priority: number;
	handler: CommandHandler;
}

interface Shortcut {
	key?: string;
	code?: string;
	displayKey?: string;
	mod?: boolean;
	shift?: boolean;
	alt?: boolean;
}

interface CommandDefinition {
	id: CommandId;
	label: string;
	shortcuts: Shortcut[];
}

const commandDefinitions: CommandDefinition[] = [
	{
		id: 'edit.deleteSelection',
		label: 'Delete Selection',
		shortcuts: [{ key: 'delete' }, { key: 'backspace' }]
	},
	{
		id: 'view.frame',
		label: 'Frame Selection',
		shortcuts: [{ key: 'f' }]
	},
	{
		id: 'view.home',
		label: 'Home',
		shortcuts: [{ key: 'h' }]
	},
	{
		id: 'dashboard.toggleEditMode',
		label: 'Toggle Dashboard Edit Mode',
		shortcuts: [{ key: 'e', mod: true }]
	},
	{
		id: 'select.all',
		label: 'Select All',
		shortcuts: [{ key: 'a', mod: true }]
	},
	{
		id: 'edit.copy',
		label: 'Copy',
		shortcuts: [{ key: 'c', mod: true }]
	},
	{
		id: 'edit.cut',
		label: 'Cut',
		shortcuts: [{ key: 'x', mod: true }]
	},
	{
		id: 'edit.duplicate',
		label: 'Duplicate',
		shortcuts: [{ key: 'd', mod: true }]
	},
	{
		id: 'edit.paste',
		label: 'Paste',
		shortcuts: [{ key: 'v', mod: true }]
	},
	{
		id: 'edit.undo',
		label: 'Undo',
		shortcuts: [{ key: 'z', mod: true }]
	},
	{
		id: 'edit.redo',
		label: 'Redo',
		shortcuts: [
			{ key: 'z', mod: true, shift: true },
			{ key: 'y', mod: true }
		]
	},
	{
		id: 'file.new',
		label: 'New',
		shortcuts: [{ key: 'n', mod: true }]
	},
	{
		id: 'file.save',
		label: 'Save',
		shortcuts: [{ key: 's', mod: true }]
	},
	{
		id: 'file.saveAs',
		label: 'Save As',
		shortcuts: [{ key: 's', mod: true, shift: true }]
	},
	{
		id: 'file.open',
		label: 'Open',
		shortcuts: [{ key: 'o', mod: true }]
	},
	{
		id: 'file.reopenLast',
		label: 'Reopen Last',
		shortcuts: [{ key: 'o', mod: true, shift: true }]
	},
	{
		id: 'file.preferences',
		label: 'Preferences',
		shortcuts: [{ key: ',', code: 'Comma', displayKey: ',', mod: true }]
	}
];

const commandDefinitionById = new Map<CommandId, CommandDefinition>(
	commandDefinitions.map((entry) => [entry.id, entry])
);

const handlersByCommand = new Map<CommandId, CommandHandlerEntry[]>();
let nextHandlerId = 1;

const normalizeKey = (rawKey: string): string => {
	const key = rawKey.trim().toLowerCase();
	if (key === 'del') {
		return 'delete';
	}
	return key;
};

const normalizeCode = (rawCode: string): string => rawCode.trim().toLowerCase();

const keyDisplayLabel = (key: string): string => {
	switch (key) {
		case 'delete':
			return 'Del';
		case 'backspace':
			return 'Backspace';
		default:
			return key.length === 1 ? key.toUpperCase() : key;
	}
};

const isShortcutKeyMatch = (event: KeyboardEvent, shortcut: Shortcut): boolean => {
	const matchesKey =
		shortcut.key !== undefined && normalizeKey(event.key) === normalizeKey(shortcut.key);
	const matchesCode =
		shortcut.code !== undefined && normalizeCode(event.code) === normalizeCode(shortcut.code);
	return matchesKey || matchesCode;
};

const isShortcutMatch = (event: KeyboardEvent, shortcut: Shortcut): boolean => {
	if (!isShortcutKeyMatch(event, shortcut)) {
		return false;
	}

	const wantsMod = shortcut.mod === true;
	const hasMod = event.ctrlKey || event.metaKey;
	if (hasMod !== wantsMod) {
		return false;
	}

	const wantsShift = shortcut.shift === true;
	if (event.shiftKey !== wantsShift) {
		return false;
	}

	const wantsAlt = shortcut.alt === true;
	return event.altKey === wantsAlt;
};

const commandIdForKeyboardEvent = (event: KeyboardEvent): CommandId | null => {
	for (const definition of commandDefinitions) {
		for (const shortcut of definition.shortcuts) {
			if (isShortcutMatch(event, shortcut)) {
				return definition.id;
			}
		}
	}
	return null;
};

const normalizeHandlerResult = (result: boolean | void): boolean => result !== false;

export const registerCommandHandler = (
	commandId: CommandId,
	handler: CommandHandler,
	options: { priority?: number } = {}
): (() => void) => {
	const id = nextHandlerId++;
	const entry: CommandHandlerEntry = {
		id,
		priority: options.priority ?? 0,
		handler
	};
	const existing = handlersByCommand.get(commandId) ?? [];
	existing.push(entry);
	existing.sort((left, right) => right.priority - left.priority);
	handlersByCommand.set(commandId, existing);

	return (): void => {
		const current = handlersByCommand.get(commandId);
		if (!current) {
			return;
		}
		const next = current.filter((candidate) => candidate.id !== id);
		if (next.length === 0) {
			handlersByCommand.delete(commandId);
			return;
		}
		handlersByCommand.set(commandId, next);
	};
};

export const executeCommand = async (
	commandId: CommandId,
	context: CommandExecuteContext
): Promise<boolean> => {
	const handlers = handlersByCommand.get(commandId) ?? [];
	for (const entry of handlers) {
		const handled = normalizeHandlerResult(await entry.handler(context));
		if (handled) {
			return true;
		}
	}
	return false;
};

export const handleCommandShortcut = async (event: KeyboardEvent): Promise<boolean> => {
	const commandId = commandIdForKeyboardEvent(event);
	if (!commandId) {
		return false;
	}
	event.preventDefault();
	return executeCommand(commandId, { source: 'keyboard', event });
};

const shortcutLabel = (shortcut: Shortcut): string => {
	const parts: string[] = [];
	if (shortcut.mod) {
		parts.push(platform.isMac ? 'Cmd' : 'Ctrl');
	}
	if (shortcut.shift) {
		parts.push('Shift');
	}
	if (shortcut.alt) {
		parts.push('Alt');
	}
	const displayKey = shortcut.displayKey ?? shortcut.key ?? shortcut.code;
	if (displayKey) {
		parts.push(keyDisplayLabel(normalizeKey(displayKey)));
	}
	return parts.join('+');
};

export const getCommandLabel = (commandId: CommandId): string => {
	return commandDefinitionById.get(commandId)?.label ?? commandId;
};

export const getCommandShortcutHint = (commandId: CommandId): string | undefined => {
	const definition = commandDefinitionById.get(commandId);
	if (!definition || definition.shortcuts.length === 0) {
		return undefined;
	}
	return definition.shortcuts.map((shortcut) => shortcutLabel(shortcut)).join(' / ');
};
