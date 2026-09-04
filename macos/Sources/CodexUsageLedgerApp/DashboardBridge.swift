import Foundation

enum DashboardBridgeMessage: String, CaseIterable {
    case exportPNG
    case languageChanged
}

enum DashboardLanguage: String {
    case simplifiedChinese = "zh-CN"
    case english = "en"
}
