import type {
	UiProjectFileSpec,
	UiProjectFileLoadResult,
	UiProjectLoadRecovery,
	UiScriptConfig,
	UiScriptManifest,
	UiScriptSource,
	UiScriptState
} from '../../types';
import type { ScriptUiState as RustScriptState } from '../../generated/rust_protocol/ScriptUiState';
import type { UiProjectPathDto as RustProjectPathResponse } from '../../generated/rust_protocol/UiProjectPathDto';
import type { UiScriptConfigRequest as RustScriptConfigRequest } from '../../generated/rust_protocol/UiScriptConfigRequest';
import type { JsonValue as RustJsonValue } from '../../generated/rust_protocol/serde_json/JsonValue';
import { isRecord } from './values';

const fromRustProjectLoadRecovery = (recovery: unknown): UiProjectLoadRecovery | undefined => {
	if (!isRecord(recovery)) {
		return undefined;
	}

	const rawProblems = Array.isArray(recovery.problems) ? recovery.problems : [];
	const problems = rawProblems
		.map((problem): UiProjectLoadRecovery['problems'][number] | null => {
			if (!isRecord(problem)) {
				return null;
			}
			const message = typeof problem.message === 'string' ? problem.message.trim() : '';
			if (message.length === 0) {
				return null;
			}
			const stage =
				typeof problem.stage === 'string' && problem.stage.trim().length > 0
					? problem.stage.trim()
					: 'project_load';
			return { stage, message };
		})
		.filter((problem): problem is UiProjectLoadRecovery['problems'][number] => problem !== null);

	return problems.length > 0 ? { problems } : undefined;
};

export const fromRustProjectPathResponse = (
	endpoint: string,
	response: RustProjectPathResponse
): UiProjectFileLoadResult => {
	if (typeof response.path !== 'string' || response.path.trim().length === 0) {
		throw new Error(`POST ${endpoint} returned an invalid project path`);
	}
	return {
		path: response.path,
		ui_state: response.ui_state,
		recovery: fromRustProjectLoadRecovery(response.recovery)
	};
};

export const toRustJsonValue = (value: unknown): RustJsonValue | undefined => {
	if (value === undefined) {
		return undefined;
	}
	if (
		value === null ||
		typeof value === 'string' ||
		typeof value === 'boolean' ||
		(typeof value === 'number' && Number.isFinite(value))
	) {
		return value;
	}
	if (Array.isArray(value)) {
		return value.map((entry) => toRustJsonValue(entry) ?? null);
	}
	if (!isRecord(value)) {
		return undefined;
	}

	const output: { [key: string]: RustJsonValue } = {};
	for (const [key, entry] of Object.entries(value)) {
		const jsonEntry = toRustJsonValue(entry);
		if (jsonEntry !== undefined) {
			output[key] = jsonEntry;
		}
	}
	return output;
};
type RustScriptConfig = RustScriptConfigRequest['config'];
type RustScriptSource = RustScriptConfig['source'];

export const fromRustProjectFileSpec = (projectFile: unknown): UiProjectFileSpec => {
	if (!isRecord(projectFile)) {
		return {
			display_name: 'Project',
			extension: 'json',
			current_path: null
		};
	}

	const displayName =
		typeof projectFile.display_name === 'string' && projectFile.display_name.trim().length > 0
			? projectFile.display_name.trim()
			: 'Project';
	const extension =
		typeof projectFile.extension === 'string' && projectFile.extension.trim().length > 0
			? projectFile.extension.trim().replace(/^\.+/, '').toLowerCase()
			: 'json';
	const currentPath =
		typeof projectFile.current_path === 'string' && projectFile.current_path.trim().length > 0
			? projectFile.current_path.trim()
			: null;

	return {
		display_name: displayName,
		extension: extension.length > 0 ? extension : 'json',
		current_path: currentPath
	};
};

const fromRustScriptSource = (source: unknown): UiScriptSource => {
	if (isRecord(source)) {
		if (source.kind === 'projectFile') {
			return { kind: 'projectFile', path: String(source.path ?? '') };
		}
		if (source.kind === 'inline') {
			return { kind: 'inline', text: String(source.text ?? '') };
		}
	}
	return { kind: 'inline', text: '' };
};

const toRustScriptSource = (source: UiScriptSource): RustScriptSource => {
	if (source.kind === 'projectFile') {
		return { kind: 'projectFile', path: source.path };
	}
	return { kind: 'inline', text: source.text };
};

const fromRustScriptConfig = (config: unknown): UiScriptConfig => {
	if (!isRecord(config)) {
		return {
			source: { kind: 'inline', text: '' }
		};
	}

	return {
		source: fromRustScriptSource(config.source)
	};
};

export const toRustScriptConfig = (config: UiScriptConfig): RustScriptConfig => ({
	source: toRustScriptSource(config.source)
});

const fromRustScriptManifest = (
	manifest: RustScriptState['manifest']
): UiScriptManifest | undefined => {
	if (!isRecord(manifest)) {
		return undefined;
	}

	const apiVersion =
		typeof manifest.api_version === 'number' && Number.isFinite(manifest.api_version)
			? Math.max(1, Math.round(manifest.api_version))
			: undefined;
	if (apiVersion === undefined) {
		return undefined;
	}

	const exports = Array.isArray(manifest.exports)
		? manifest.exports
				.map((value) => {
					const signature = isRecord(value.signature) ? value.signature : {};
					const argsRaw = signature.args;
					const args = Array.isArray(argsRaw)
						? argsRaw.filter((entry): entry is string => typeof entry === 'string')
						: [];
					const returns =
						typeof signature.returns === 'string' && signature.returns.length > 0
							? signature.returns
							: undefined;
					return {
						name: typeof value.name === 'string' ? value.name : '',
						signature: { args, returns }
					};
				})
				.filter((value) => value.name.length > 0)
		: [];

	return {
		api_version: apiVersion,
		update_rate_hz:
			typeof manifest.update_rate_hz === 'number' && Number.isFinite(manifest.update_rate_hz)
				? Math.max(1, Math.round(manifest.update_rate_hz))
				: undefined,
		exports
	};
};

export const fromRustScriptState = (state: RustScriptState): UiScriptState => ({
	config: fromRustScriptConfig(state.config),
	effective_update_rate_hz:
		typeof state.effective_update_rate_hz === 'number' &&
		Number.isFinite(state.effective_update_rate_hz)
			? Math.max(1, Math.round(state.effective_update_rate_hz))
			: undefined,
	export_names: Array.isArray(state.export_names)
		? state.export_names.filter((name): name is string => typeof name === 'string')
		: [],
	manifest: fromRustScriptManifest(state.manifest)
});
