import Foundation

struct LedgerRuntimePaths {
    let binary: URL
    let webRoot: URL
    let applicationSupportDirectory: URL
    let database: URL
    let codexHome: URL?
    let isolatedProfile: String?

    static func resolve(
        bundle: Bundle = .main,
        fileManager: FileManager = .default,
        arguments: [String] = ProcessInfo.processInfo.arguments
    ) throws -> LedgerRuntimePaths {
        guard let resources = bundle.resourceURL else {
            throw ServiceConfigurationError.missingResources
        }
        let isolatedProfile = try LedgerLaunchProfile.identifier(arguments: arguments)
        guard let applicationSupport = fileManager.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first else {
            throw ServiceConfigurationError.missingApplicationSupport
        }

        let isolatedRoot = isolatedProfile.map { identifier in
            fileManager.temporaryDirectory.resolvingSymlinksInPath()
                .appendingPathComponent("CodexUsageLedgerPreview-\(identifier)", isDirectory: true)
        }
        let supportDirectory = isolatedRoot?.appendingPathComponent("data", isDirectory: true) ?? applicationSupport.appendingPathComponent(
            "Codex Usage Ledger",
            isDirectory: true
        )
        return LedgerRuntimePaths(
            binary: resources.appendingPathComponent("bin/codex-usage-ledger", isDirectory: false),
            webRoot: resources.appendingPathComponent("web/dist", isDirectory: true),
            applicationSupportDirectory: supportDirectory,
            database: supportDirectory.appendingPathComponent("ledger.sqlite3", isDirectory: false),
            codexHome: isolatedRoot?.appendingPathComponent("codex", isDirectory: true),
            isolatedProfile: isolatedProfile
        )
    }

    func validateBundledResources(fileManager: FileManager = .default) throws {
        var isDirectory: ObjCBool = false
        guard fileManager.fileExists(atPath: binary.path, isDirectory: &isDirectory),
              !isDirectory.boolValue,
              fileManager.isExecutableFile(atPath: binary.path) else {
            throw ServiceConfigurationError.missingBinary(binary.path)
        }
        guard fileManager.fileExists(
            atPath: webRoot.appendingPathComponent("index.html").path,
            isDirectory: nil
        ) else {
            throw ServiceConfigurationError.missingWebRoot(webRoot.path)
        }
    }

    func serviceArguments(mode: LedgerServiceMode) -> [String] {
        var arguments = [mode.rawValue, "--db", database.path,
                         "--listen", "127.0.0.1:47127", "--web-root", webRoot.path]
        if let codexHome { arguments.append(contentsOf: ["--codex-home", codexHome.path]) }
        return arguments
    }

    func serviceEnvironment(inherited: [String: String]) -> [String: String] {
        var environment = inherited
        environment["NO_COLOR"] = "1"
        environment["RUST_LOG"] = environment["RUST_LOG"] ?? "codex_usage_ledger=info"
        if isolatedProfile != nil {
            for key in ["CODEX_HOME", "CODEX_USAGE_LEDGER_DB", "CODEX_USAGE_LEDGER_WEB_ROOT"] {
                environment.removeValue(forKey: key)
            }
        }
        return environment
    }

    func secureApplicationSupportDirectory(fileManager: FileManager = .default) throws {
        if isolatedProfile != nil {
            guard let codexHome else { throw ServiceConfigurationError.invalidIsolatedProfile }
            // Refuse links even when their destination does not exist. The
            // UUID-only profile name cannot supply arbitrary paths or traversal.
            for path in [applicationSupportDirectory.deletingLastPathComponent(), applicationSupportDirectory, database, codexHome]
            where (try? fileManager.destinationOfSymbolicLink(atPath: path.path)) != nil {
                throw ServiceConfigurationError.linkedIsolatedProfile
            }
            let root = applicationSupportDirectory.deletingLastPathComponent()
            try fileManager.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            try fileManager.setAttributes([.posixPermissions: 0o700], ofItemAtPath: root.path)
        }
        try fileManager.createDirectory(
            at: applicationSupportDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try fileManager.setAttributes(
            [.posixPermissions: 0o700],
            ofItemAtPath: applicationSupportDirectory.path
        )
        if let codexHome {
            try fileManager.createDirectory(at: codexHome, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        }
    }

    func secureDatabasePermissionsIfPresent(fileManager: FileManager = .default) {
        guard fileManager.fileExists(atPath: database.path) else { return }
        try? fileManager.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: database.path
        )
    }
}

enum ServiceConfigurationError: LocalizedError {
    case missingResources
    case missingApplicationSupport
    case missingBinary(String)
    case missingWebRoot(String)
    case invalidIsolatedProfile
    case linkedIsolatedProfile

    var errorDescription: String? {
        switch self {
        case .invalidIsolatedProfile:
            return NativeLocalization.text("隔离模式需要 --isolated-profile 和一个 UUID；不会回退到真实账本。", "Isolated mode requires --isolated-profile followed by a UUID; the real ledger will not be used.")
        case .linkedIsolatedProfile:
            return NativeLocalization.text("隔离目录包含符号链接，已拒绝启动。", "The isolated profile contains a symbolic link; startup was refused.")
        case .missingResources:
            return NativeLocalization.text("应用包缺少 Resources 目录。", "The application bundle is missing its Resources directory.")
        case .missingApplicationSupport:
            return NativeLocalization.text("无法定位用户的 Application Support 目录。", "The user Application Support directory could not be located.")
        case .missingBinary(let path):
            return NativeLocalization.text("内置用量服务缺失或不可执行：\(path)。", "The bundled usage service is missing or not executable: \(path).")
        case .missingWebRoot(let path):
            return NativeLocalization.text("内置看板缺少 index.html：\(path)。", "The bundled dashboard is missing index.html: \(path).")
        }
    }
}
