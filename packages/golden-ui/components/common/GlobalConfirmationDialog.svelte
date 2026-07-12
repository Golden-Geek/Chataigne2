<script lang="ts">
	import ConfirmationDialog from './ConfirmationDialog.svelte';
	import {
		cancelConfirmation,
		confirmationDialogState,
		resolveConfirmation
	} from '../../store/confirmation-dialog.svelte';
</script>

<ConfirmationDialog
	open={confirmationDialogState.open}
	title={confirmationDialogState.title}
	message={confirmationDialogState.message}
	errorMessage={confirmationDialogState.errorMessage}
	actions={[
		...confirmationDialogState.actions.map((action) => ({
			...action,
			action: () => resolveConfirmation(action.id)
		})),
		{
			id: 'cancel',
			label: 'Cancel',
			shortcut: 'Escape',
			action: cancelConfirmation
		}
	]} />
