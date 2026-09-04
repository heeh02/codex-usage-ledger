import { afterEach, describe, expect, it, vi } from 'vitest';
import { notifyNativeLanguageChanged, requestNativePngExport } from './nativeBridge';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('native bridge', () => {
  it('is a safe no-op outside the macOS shell', () => {
    vi.stubGlobal('window', {});
    expect(requestNativePngExport({ privacyMode: true, suggestedName: 'demo.png' })).toBe(false);
    expect(() => notifyNativeLanguageChanged('en')).not.toThrow();
  });

  it('sends only typed payloads to allowlisted handlers', () => {
    const exportPNG = vi.fn();
    const languageChanged = vi.fn();
    vi.stubGlobal('window', {
      webkit: {
        messageHandlers: {
          exportPNG: { postMessage: exportPNG },
          languageChanged: { postMessage: languageChanged },
        },
      },
    });

    const payload = { privacyMode: false, suggestedName: 'codex-usage-demo.png' };
    expect(requestNativePngExport(payload)).toBe(true);
    notifyNativeLanguageChanged('zh-CN');
    expect(exportPNG).toHaveBeenCalledWith(payload);
    expect(languageChanged).toHaveBeenCalledWith('zh-CN');
  });
});
