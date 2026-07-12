export const copyTextToClipboard = async (text: string): Promise<boolean> => {
	const value = String(text ?? '');
	if (value.length === 0) {
		return false;
	}

	try {
		if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
			await navigator.clipboard.writeText(value);
			return true;
		}
	} catch (error) {
		console.warn('[ui] navigator clipboard write failed', error);
	}

	if (typeof document === 'undefined') {
		return false;
	}

	const textarea = document.createElement('textarea');
	textarea.value = value;
	textarea.setAttribute('readonly', 'readonly');
	textarea.style.position = 'fixed';
	textarea.style.opacity = '0';
	document.body.appendChild(textarea);
	textarea.select();

	try {
		return document.execCommand('copy');
	} catch (error) {
		console.warn('[ui] document clipboard write failed', error);
		return false;
	} finally {
		document.body.removeChild(textarea);
	}
};
