import type {
	UiBrowserPreview,
	UiParamControlInfo,
	UiProjectFileLoadResult,
	UiProjectLoadOptions,
	UiReferenceTargets,
	UiScriptConfig,
	UiScriptState
} from '../types';
import type { ScriptUiState as RustScriptState } from '../generated/rust_protocol/ScriptUiState';
import type { UiParamControlInfoDto as RustUiParamControlInfoResponse } from '../generated/rust_protocol/UiParamControlInfoDto';
import type { UiParamControlInfoRequest as RustParamControlInfoRequest } from '../generated/rust_protocol/UiParamControlInfoRequest';
import type { UiOscAddressDto } from '../generated/rust_protocol/UiOscAddressDto';
import type { UiOscAddressRequest } from '../generated/rust_protocol/UiOscAddressRequest';
import type { UiBrowserPreviewExtent } from '../generated/rust_protocol/UiBrowserPreviewExtent';
import type { UiProjectPathDto as RustProjectPathResponse } from '../generated/rust_protocol/UiProjectPathDto';
import type { UiProjectPathRequest as RustProjectPathRequest } from '../generated/rust_protocol/UiProjectPathRequest';
import type { UiProjectUploadRequest as RustProjectUploadRequest } from '../generated/rust_protocol/UiProjectUploadRequest';
import type { UiReferenceTargetsDto as RustReferenceTargetsResponse } from '../generated/rust_protocol/UiReferenceTargetsDto';
import type { UiReferenceTargetsRequest as RustReferenceTargetsRequest } from '../generated/rust_protocol/UiReferenceTargetsRequest';
import type { UiScriptConfigRequest as RustScriptConfigRequest } from '../generated/rust_protocol/UiScriptConfigRequest';
import type { UiScriptReloadRequest as RustScriptReloadRequest } from '../generated/rust_protocol/UiScriptReloadRequest';
import type { UiScriptStateRequest as RustScriptStateRequest } from '../generated/rust_protocol/UiScriptStateRequest';
import { getUiClientInstanceId } from './client-instance';
import { formatHttpError } from './control-errors';
import { createUiSessionProvider, type UiSessionProvider } from './session';
import { fromRustReferenceTargets, fromRustParamControlInfo } from './codecs/values';
import {
	fromRustScriptState,
	toRustScriptConfig,
	toRustJsonValue,
	fromRustProjectPathResponse
} from './codecs/projects';

const DEFAULT_BASE_URL = 'http://localhost:7010/api/ui';
const DEFAULT_POST_TIMEOUT_MS = 15000;
const PROJECT_LOAD_TIMEOUT_MS = 120000;

interface HttpClientOptions {
	applicationId?: string;
	baseUrl?: string;
	fetchImpl?: typeof fetch;
	sessionProvider?: UiSessionProvider;
}

interface PostJsonOptions {
	timeoutMs?: number;
	signal?: AbortSignal;
}

export interface UiHttpAuxiliaryClient {
	browserPreview: UiBrowserPreview;
	referenceTargets(paramNodeId: number): Promise<UiReferenceTargets>;
	oscAddress(request: UiOscAddressRequest, signal?: AbortSignal): Promise<UiOscAddressDto>;
	paramControlInfo(paramNodeId: number): Promise<UiParamControlInfo>;
	scriptState(nodeId: number): Promise<UiScriptState>;
	setScriptConfig(nodeId: number, config: UiScriptConfig, forceReload?: boolean): Promise<void>;
	reloadScript(nodeId: number): Promise<void>;
	projectNew(): Promise<void>;
	projectSave(path: string, uiState?: unknown): Promise<void>;
	projectLoad(path: string, options?: UiProjectLoadOptions): Promise<UiProjectFileLoadResult>;
	projectUploadLoad(
		fileName: string,
		contents: string,
		options?: UiProjectLoadOptions
	): Promise<UiProjectFileLoadResult>;
}

export const createHttpAuxiliaryClient = (
	options: HttpClientOptions = {}
): UiHttpAuxiliaryClient => {
	const fetchImpl = options.fetchImpl ?? fetch;
	const baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, '');
	const session =
		options.sessionProvider ??
		createUiSessionProvider({
			baseUrl,
			fetchImpl,
			applicationId: options.applicationId
		});
	const clientInstanceId = getUiClientInstanceId(options.applicationId);
	const authenticatedFetch = async (
		path: string,
		init: RequestInit,
		renew = false
	): Promise<Response> => {
		const credential = await session.credentials(renew);
		const headers = new Headers(init.headers);
		headers.set('x-gc-ui-client-instance', clientInstanceId);
		headers.set('authorization', `Bearer ${credential.token}`);
		const response = await fetchImpl(`${baseUrl}${path}`, { ...init, headers });
		if (response.status !== 401 || renew) return response;
		session.invalidate(credential.token);
		return authenticatedFetch(path, init, true);
	};

	const postJson = async <TResponse>(
		path: string,
		payload: unknown,
		options: PostJsonOptions = {}
	): Promise<TResponse> => {
		const timeoutMs = Math.max(1000, options.timeoutMs ?? DEFAULT_POST_TIMEOUT_MS);
		const controller = typeof AbortController !== 'undefined' ? new AbortController() : null;
		const signal =
			controller && options.signal
				? AbortSignal.any([controller.signal, options.signal])
				: (controller?.signal ?? options.signal);
		const timeoutId =
			controller !== null
				? setTimeout(() => {
						controller.abort();
					}, timeoutMs)
				: null;

		try {
			const response = await authenticatedFetch(path, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(payload),
				signal
			});

			let body: unknown = undefined;
			const text = await response.text();
			if (text.length > 0) {
				try {
					body = JSON.parse(text) as unknown;
				} catch {
					body = text;
				}
			}

			if (!response.ok) {
				throw formatHttpError(`POST ${path}`, response, body);
			}

			return body as TResponse;
		} catch (error) {
			if (controller?.signal.aborted) {
				throw new Error(`POST ${path} timed out after ${Math.round(timeoutMs / 1000)}s`);
			}
			throw error;
		} finally {
			if (timeoutId !== null) {
				clearTimeout(timeoutId);
			}
		}
	};

	const client: UiHttpAuxiliaryClient = {
		async browserPreview(pull, signal) {
			const response = await authenticatedFetch('/browser-preview', {
				method: 'POST',
				cache: 'no-store',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(pull),
				signal: AbortSignal.any([signal, AbortSignal.timeout(3000)])
			});
			if (response.status === 204) return null;
			if (!response.ok)
				throw formatHttpError('POST browser preview', response, await response.text());
			const tag = response.headers.get('etag');
			const extentHeader = response.headers.get('x-gc-preview-extent');
			if (!tag || !extentHeader)
				throw new Error('Browser preview response has incomplete metadata');
			const extent = JSON.parse(extentHeader) as Partial<UiBrowserPreviewExtent> | null;
			if (
				!extent?.width ||
				!extent.height ||
				!Number.isSafeInteger(extent.width) ||
				!Number.isSafeInteger(extent.height) ||
				extent.width < 0 ||
				extent.height < 0
			) {
				throw new Error('Browser preview response has an invalid extent');
			}
			return {
				tag,
				image: await response.blob(),
				extent: { width: extent.width, height: extent.height }
			};
		},
		oscAddress(request, signal) {
			return postJson<UiOscAddressDto>('/osc-address', request, { signal });
		},

		async referenceTargets(paramNodeId: number): Promise<UiReferenceTargets> {
			const request: RustReferenceTargetsRequest = { param: paramNodeId };
			const response = await postJson<RustReferenceTargetsResponse>('/reference-targets', request);
			return fromRustReferenceTargets(response);
		},

		async paramControlInfo(paramNodeId: number): Promise<UiParamControlInfo> {
			const request: RustParamControlInfoRequest = { param: paramNodeId };
			const response = await postJson<RustUiParamControlInfoResponse>(
				'/param-control-info',
				request
			);
			return fromRustParamControlInfo(response);
		},

		async scriptState(nodeId: number): Promise<UiScriptState> {
			const request: RustScriptStateRequest = { node: nodeId };
			const response = await postJson<RustScriptState>('/script-state', request);
			return fromRustScriptState(response);
		},

		async setScriptConfig(
			nodeId: number,
			config: UiScriptConfig,
			forceReload = false
		): Promise<void> {
			const request: RustScriptConfigRequest = {
				node: nodeId,
				config: toRustScriptConfig(config),
				force_reload: forceReload
			};
			await postJson<{ ok?: boolean }>('/script-config', request);
		},

		async reloadScript(nodeId: number): Promise<void> {
			const request: RustScriptReloadRequest = { node: nodeId };
			await postJson<{ ok?: boolean }>('/script-reload', request);
		},

		async projectNew(): Promise<void> {
			await postJson<{ ok?: boolean }>('/project-new', {});
		},

		async projectSave(path: string, uiState?: unknown): Promise<void> {
			const request: RustProjectPathRequest = { path };
			const jsonUiState = toRustJsonValue(uiState);
			if (jsonUiState !== undefined) {
				request.ui_state = jsonUiState;
			}
			await postJson<{ ok?: boolean }>('/project-save', request);
		},

		async projectLoad(
			path: string,
			options: UiProjectLoadOptions = {}
		): Promise<UiProjectFileLoadResult> {
			const request: RustProjectPathRequest = { path };
			if (options.recover === true) {
				request.recover = true;
			}
			const response = await postJson<RustProjectPathResponse>('/project-load', request, {
				timeoutMs: PROJECT_LOAD_TIMEOUT_MS
			});
			return fromRustProjectPathResponse('/project-load', response);
		},

		async projectUploadLoad(
			fileName: string,
			contents: string,
			options: UiProjectLoadOptions = {}
		): Promise<UiProjectFileLoadResult> {
			const request: RustProjectUploadRequest = {
				file_name: fileName,
				contents
			};
			if (options.recover === true) {
				request.recover = true;
			}
			const response = await postJson<RustProjectPathResponse>('/project-upload-load', request, {
				timeoutMs: PROJECT_LOAD_TIMEOUT_MS
			});
			return fromRustProjectPathResponse('/project-upload-load', response);
		}
	};

	return client;
};
