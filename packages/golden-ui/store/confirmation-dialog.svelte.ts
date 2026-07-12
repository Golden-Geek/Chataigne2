export interface ConfirmationRequestAction {
	id: string;
	label: string;
	shortcut?: string;
	tone?: 'primary' | 'danger' | 'neutral';
	defaultFocus?: boolean;
}

export interface ConfirmationRequest {
	title: string;
	message: string;
	errorMessage?: string | null;
	actions: ConfirmationRequestAction[];
}

interface ConfirmationDialogState extends ConfirmationRequest {
	open: boolean;
}

export const confirmationDialogState = $state<ConfirmationDialogState>({
	open: false,
	title: '',
	message: '',
	errorMessage: null,
	actions: []
});

let resolveActiveConfirmation: ((result: string | null) => void) | null = null;

export const cancelConfirmation = (): void => {
	if (!confirmationDialogState.open) return;
	confirmationDialogState.open = false;
	const resolve = resolveActiveConfirmation;
	resolveActiveConfirmation = null;
	resolve?.(null);
};

export const resolveConfirmation = (actionId: string): void => {
	if (!confirmationDialogState.open) return;
	confirmationDialogState.open = false;
	const resolve = resolveActiveConfirmation;
	resolveActiveConfirmation = null;
	resolve?.(actionId);
};

export const requestConfirmation = (request: ConfirmationRequest): Promise<string | null> => {
	cancelConfirmation();
	confirmationDialogState.title = request.title;
	confirmationDialogState.message = request.message;
	confirmationDialogState.errorMessage = request.errorMessage ?? null;
	confirmationDialogState.actions = request.actions;
	confirmationDialogState.open = true;
	return new Promise((resolve) => {
		resolveActiveConfirmation = resolve;
	});
};
