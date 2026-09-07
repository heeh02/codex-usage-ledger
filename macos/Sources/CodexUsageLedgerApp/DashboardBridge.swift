import Foundation

enum DashboardBridgeMessage: String, CaseIterable {
    case exportPNG
    case languageChanged
}

enum DashboardLanguage: String {
    case simplifiedChinese = "zh-CN"
    case english = "en"
}

struct DashboardLanguageBootstrap {
    private(set) var language: DashboardLanguage?

    mutating func update(_ rawValue: String) -> String? {
        let next = DashboardLanguage(rawValue: rawValue) ?? .simplifiedChinese
        guard next != language else { return nil }
        language = next
        return "try { window.localStorage.setItem('ledger.language', '\(next.rawValue)'); } catch (_) {}"
    }
}
