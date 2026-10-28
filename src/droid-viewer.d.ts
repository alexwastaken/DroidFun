export function mountDroid(host: HTMLElement, name: string, onReady: (ready: boolean) => void, onManual?: () => void): { setMotion: (playing: boolean) => void; dispose: () => void };
