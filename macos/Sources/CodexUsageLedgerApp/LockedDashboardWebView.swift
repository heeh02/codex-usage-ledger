import SwiftUI
import UniformTypeIdentifiers
import WebKit

struct LockedDashboardWebView: NSViewRepresentable {
    let url: URL
    let reloadToken: UUID
    let pageZoom: Double
    let initialLanguage: String
    let onLanguageChange: (String) -> Void
    @Binding var isLoaded: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator(allowedURL: url, isLoaded: $isLoaded, onLanguageChange: onLanguageChange)
    }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        let safeLanguage = initialLanguage == "en" ? "en" : "zh-CN"
        configuration.userContentController.addUserScript(
            WKUserScript(
                source: "try { window.localStorage.setItem('ledger.language', '\(safeLanguage)'); } catch (_) {}",
                injectionTime: .atDocumentStart,
                forMainFrameOnly: true
            )
        )
        configuration.userContentController.addUserScript(
            WKUserScript(
                source: Self.contentSecurityPolicyScript,
                injectionTime: .atDocumentStart,
                forMainFrameOnly: true
            )
        )
        for message in DashboardBridgeMessage.allCases {
            configuration.userContentController.add(context.coordinator, name: message.rawValue)
        }

        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = false
        webView.allowsMagnification = true
        webView.pageZoom = pageZoom
        webView.customUserAgent = "CodexUsageLedger/0.1 WKWebView"
        webView.menu = nil
        if #available(macOS 13.3, *) {
            webView.isInspectable = false
        }

        context.coordinator.load(url, reloadToken: reloadToken, in: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        if abs(webView.pageZoom - pageZoom) > 0.001 {
            webView.pageZoom = pageZoom
        }
        context.coordinator.load(url, reloadToken: reloadToken, in: webView)
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        webView.stopLoading()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
        webView.configuration.userContentController.removeAllUserScripts()
        for message in DashboardBridgeMessage.allCases {
            webView.configuration.userContentController.removeScriptMessageHandler(forName: message.rawValue)
        }
    }

    private static let contentSecurityPolicyScript = #"""
    (() => {
      const policy = "default-src 'self'; base-uri 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; form-action 'none'; connect-src 'self' http://127.0.0.1:47127; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'";
      const install = () => {
        if (document.querySelector('meta[http-equiv="Content-Security-Policy"]')) return true;
        const parent = document.head || document.documentElement;
        if (!parent) return false;
        const meta = document.createElement('meta');
        meta.httpEquiv = 'Content-Security-Policy';
        meta.content = policy;
        parent.appendChild(meta);
        return true;
      };
      if (!install()) {
        const observer = new MutationObserver(() => {
          if (install()) observer.disconnect();
        });
        observer.observe(document, { childList: true, subtree: true });
      }
    })();
    """#

    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
        private let allowedScheme: String
        private let allowedHost: String
        private let allowedPort: Int
        private var isLoaded: Binding<Bool>
        private let onLanguageChange: (String) -> Void
        private var loadedReloadToken: UUID?

        init(allowedURL: URL, isLoaded: Binding<Bool>, onLanguageChange: @escaping (String) -> Void) {
            allowedScheme = allowedURL.scheme ?? "http"
            allowedHost = allowedURL.host ?? "127.0.0.1"
            allowedPort = allowedURL.port ?? 47127
            self.isLoaded = isLoaded
            self.onLanguageChange = onLanguageChange
        }

        func load(_ url: URL, reloadToken: UUID, in webView: WKWebView) {
            guard loadedReloadToken != reloadToken else { return }
            loadedReloadToken = reloadToken
            isLoaded.wrappedValue = false
            var request = URLRequest(url: url)
            request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            request.timeoutInterval = 8
            webView.load(request)
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            isLoaded.wrappedValue = true
        }

        func webView(
            _ webView: WKWebView,
            didFail navigation: WKNavigation!,
            withError error: Error
        ) {
            isLoaded.wrappedValue = false
        }

        func webView(
            _ webView: WKWebView,
            didFailProvisionalNavigation navigation: WKNavigation!,
            withError error: Error
        ) {
            isLoaded.wrappedValue = false
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            guard navigationAction.targetFrame != nil,
                  let requestedURL = navigationAction.request.url,
                  isAllowed(requestedURL) else {
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationResponse: WKNavigationResponse,
            decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void
        ) {
            guard let responseURL = navigationResponse.response.url,
                  isAllowed(responseURL) else {
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }

        func webView(
            _ webView: WKWebView,
            createWebViewWith configuration: WKWebViewConfiguration,
            for navigationAction: WKNavigationAction,
            windowFeatures: WKWindowFeatures
        ) -> WKWebView? {
            nil
        }

        func userContentController(
            _ userContentController: WKUserContentController,
            didReceive message: WKScriptMessage
        ) {
            guard let sourceURL = message.frameInfo.request.url,
                  isAllowed(sourceURL),
                  let kind = DashboardBridgeMessage(rawValue: message.name) else { return }

            if kind == .languageChanged,
               let rawLanguage = message.body as? String,
               let language = DashboardLanguage(rawValue: rawLanguage) {
                onLanguageChange(language.rawValue)
                return
            }
            guard kind == .exportPNG,
                  let webView = message.webView else { return }
            let suggestedName = (message.body as? [String: Any])?["suggestedName"] as? String
                ?? "codex-usage.png"
            let panel = NSSavePanel()
            panel.allowedContentTypes = [.png]
            panel.canCreateDirectories = true
            panel.nameFieldStringValue = suggestedName
            guard panel.runModal() == .OK, let destination = panel.url else { return }

            webView.takeSnapshot(with: nil) { image, error in
                guard error == nil,
                      let image,
                      let tiff = image.tiffRepresentation,
                      let representation = NSBitmapImageRep(data: tiff),
                      let png = representation.representation(using: .png, properties: [:]) else {
                    Self.showExportError(NativeLocalization.text("无法渲染看板快照。", "The dashboard snapshot could not be rendered."))
                    return
                }
                do {
                    try png.write(to: destination, options: .atomic)
                } catch {
                    Self.showExportError(error.localizedDescription)
                }
            }
        }

        private static func showExportError(_ detail: String) {
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = NativeLocalization.text("PNG 导出失败", "PNG export failed")
            alert.informativeText = detail
            alert.runModal()
        }

        private func isAllowed(_ url: URL) -> Bool {
            guard url.scheme == allowedScheme,
                  url.host == allowedHost else { return false }
            return (url.port ?? 80) == allowedPort
        }
    }
}
