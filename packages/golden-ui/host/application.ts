import type { UiApplicationInfoDto } from '../generated/rust_protocol/UiApplicationInfoDto';

export const validateApplicationInfo = (value: unknown): UiApplicationInfoDto => {
	if (typeof value !== 'object' || value === null) {
		throw new Error('Runtime application identity is missing');
	}
	const application = value as Partial<UiApplicationInfoDto>;
	for (const field of ['id', 'display_name', 'version', 'data_directory_name'] as const) {
		if (typeof application[field] !== 'string' || application[field].trim().length === 0) {
			throw new Error(`Runtime application ${field} must be a nonempty string`);
		}
	}
	return { ...application } as UiApplicationInfoDto;
};
