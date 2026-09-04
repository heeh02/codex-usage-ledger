import Foundation

enum NativeLocalization {
    static let defaultsKey = "uiLanguage"

    static var language: String {
        if let stored = UserDefaults.standard.string(forKey: defaultsKey), stored == "en" || stored == "zh-CN" {
            return stored
        }
        return Locale.preferredLanguages.first?.lowercased().hasPrefix("zh") == true ? "zh-CN" : "en"
    }

    static func text(_ chinese: String, _ english: String) -> String {
        language == "zh-CN" ? chinese : english
    }
}
