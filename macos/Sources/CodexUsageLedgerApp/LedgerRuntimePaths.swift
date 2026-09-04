import Foundation

struct LedgerRuntimePaths {
    let binary: URL
    let webRoot: URL
    let applicationSupportDirectory: URL
    let database: URL

    static func resolve(
        bundle: Bundle = .main,
        fileManager: FileManager = .default
    ) throws -> LedgerRuntimePaths {
        guard let resources = bundle.resourceURL else {
            throw ServiceConfigurationError.missingResources
        }
        guard let applicationSupport = fileManager.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first else {
            throw ServiceConfigurationError.missingApplicationSupport
        }

        let supportDirectory = applicationSupport.appendingPathComponent(
            "Codex Usage Ledger",
            isDirectory: true
        )
        return LedgerRuntimePaths(
            binary: resources.appendingPathComponent("bin/codex-usage-ledger", isDirectory: false),
            webRoot: resources.appendingPathComponent("web/dist", isDirectory: true),
            applicationSupportDirectory: supportDirectory,
            database: supportDirectory.appendingPathComponent("ledger.sqlite3", isDirectory: false)
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

    func secureApplicationSupportDirectory(fileManager: FileManager = .default) throws {
        try fileManager.createDirectory(
            at: applicationSupportDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try fileManager.setAttributes(
            [.posixPermissions: 0o700],
            ofItemAtPath: applicationSupportDirectory.path
        )
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

    var errorDescription: String? {
        switch self {
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
