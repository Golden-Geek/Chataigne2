export const supportedAudioExtensions = ['wav', 'wave', 'aif', 'aiff', 'aifc', 'caf', 'flac', 'mp3', 'm4a', 'mp4', 'ogg', 'oga', 'mka', 'mkv', 'webm'] as const;
export type SupportedAudioExtension = (typeof supportedAudioExtensions)[number];
