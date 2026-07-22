export const DEFAULT_LOG_UI_UPDATE_HZ = 60;
export const MIN_LOG_UI_UPDATE_HZ = 1;
export const MAX_LOG_UI_UPDATE_HZ = 240;

export const normalizeLogUiUpdateHz = (value: number): number => {
	if (!Number.isFinite(value)) {
		return DEFAULT_LOG_UI_UPDATE_HZ;
	}
	return Math.min(MAX_LOG_UI_UPDATE_HZ, Math.max(MIN_LOG_UI_UPDATE_HZ, Math.round(value)));
};
