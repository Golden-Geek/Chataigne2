// Generated from golden-protocol. Do not edit.

export const PROTOCOL_VERSION = 1 as const;

export type ProtocolPlane = "control" | "authoring" | "observation" | "values";

export type ClientId = string;

export type ViewId = string;

export type ScopeId = string;

export type PreviewKey = { scope: ScopeId, entity: string, field: string, };

export type ProtocolValue = { "kind": "bool", "value": boolean } | { "kind": "integer", "value": number } | { "kind": "float", "value": number } | { "kind": "string", "value": string };

export type ControlRequest = { "kind": "ping", nonce: number, } | { "kind": "load_project", path: string, } | { "kind": "save_project", path: string | null, };

export type ControlResponse = { "kind": "pong", nonce: number, } | { "kind": "accepted", request_id: number, } | { "kind": "rejected", code: string, message: string, };

export type AuthoringChange = { entity: string, operation: string, };

export type AuthoringEvent = { revision: number, changes: Array<AuthoringChange>, };

export type ObservationInterest = { client: ClientId, view: ViewId, scopes: Array<ScopeId>, };

export type CatalogEntry = { id: string, label: string, kind: string, };

export type CatalogSnapshot = { revision: number, entries: Array<CatalogEntry>, };

export type PreviewChange = { key: PreviewKey, value: ProtocolValue, };

export type PreviewDelta = { sequence: number, changes: Array<PreviewChange>, };

export type ObservationMessage = { "kind": "catalog", "payload": CatalogSnapshot } | { "kind": "preview", "payload": PreviewDelta } | { "kind": "resync_required", "payload": { scope: ScopeId, after_sequence: number, } };

export type ServerMessage = { "plane": "control", "payload": ControlResponse } | { "plane": "authoring", "payload": AuthoringEvent } | { "plane": "observation", "payload": ObservationMessage };
