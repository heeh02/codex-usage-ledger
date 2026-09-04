export type DashboardLanguage = 'zh-CN' | 'en';

export interface PngExportRequest {
  privacyMode: boolean;
  suggestedName: string;
}

type NativeMessageHandlers = {
  exportPNG?: { postMessage: (payload: PngExportRequest) => void };
  languageChanged?: { postMessage: (language: DashboardLanguage) => void };
};

function handlers(): NativeMessageHandlers | undefined {
  return (window as Window & {
    webkit?: { messageHandlers?: NativeMessageHandlers };
  }).webkit?.messageHandlers;
}

export function requestNativePngExport(payload: PngExportRequest): boolean {
  const handler = handlers()?.exportPNG;
  if (!handler) return false;
  handler.postMessage(payload);
  return true;
}

export function notifyNativeLanguageChanged(language: DashboardLanguage): void {
  handlers()?.languageChanged?.postMessage(language);
}
