import Foundation

enum LedgerLaunchProfile {
    static func identifier(arguments: [String]) throws -> String? {
        let flags = arguments.indices.filter { arguments[$0].hasPrefix("--isolated") }
        guard !flags.isEmpty else { return nil }
        guard flags.count == 1, let index = flags.first,
              arguments[index] == "--isolated-profile", index + 1 < arguments.count,
              let identifier = UUID(uuidString: arguments[index + 1]) else {
            throw ServiceConfigurationError.invalidIsolatedProfile
        }
        return identifier.uuidString.lowercased()
    }

    static var isRequested: Bool {
        ProcessInfo.processInfo.arguments.contains { $0.hasPrefix("--isolated") }
    }

    static let preferences: UserDefaults = {
        guard isRequested else { return .standard }
        // An invalid profile must not fall back to production preferences while
        // the service presents its configuration error.
        let identifier = (try? LedgerLaunchProfile.identifier(arguments: ProcessInfo.processInfo.arguments)) ?? "invalid"
        guard let preferences = UserDefaults(suiteName: "com.heeh02.CodexUsageLedger.preview.\(identifier)") else {
            preconditionFailure("Isolated preference domain unavailable")
        }
        return preferences
    }()
}
