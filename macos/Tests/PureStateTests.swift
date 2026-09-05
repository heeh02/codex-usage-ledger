import Foundation

@main
struct PureStateTests {
    static func main() {
        testIsolatedProfile()
        var diagnostics = LedgerProcessDiagnostics()
        _ = diagnostics.append("first\nsecond\nthird")
        precondition(diagnostics.tail(lineCount: 2) == "second third")
        _ = diagnostics.append(String(repeating: "x", count: 20_000))
        precondition(diagnostics.text.count == 16_000)
        diagnostics.reset()
        precondition(diagnostics.text.isEmpty)

        precondition(
            LedgerServiceLifecycle.decision(
                applicationIsTerminating: true,
                processIsRunning: false,
                currentMode: nil,
                requestedMode: .serve
            ) == .ignore
        )
        precondition(
            LedgerServiceLifecycle.decision(
                applicationIsTerminating: false,
                processIsRunning: true,
                currentMode: .serve,
                requestedMode: .serve
            ) == .ignore
        )
        precondition(
            LedgerServiceLifecycle.decision(
                applicationIsTerminating: false,
                processIsRunning: true,
                currentMode: .serve,
                requestedMode: .daemon
            ) == .stopThenLaunch(.daemon)
        )
        precondition(
            LedgerServiceLifecycle.decision(
                applicationIsTerminating: false,
                processIsRunning: false,
                currentMode: nil,
                requestedMode: .daemon
            ) == .launch(.daemon)
        )

        precondition(Set(DashboardBridgeMessage.allCases.map(\.rawValue)) == ["exportPNG", "languageChanged"])
        print("Swift pure-state tests passed.")
    }

    static func testIsolatedProfile() {
        let identifier = UUID().uuidString.lowercased()
        precondition(try! LedgerLaunchProfile.identifier(arguments: ["app"]) == nil)
        precondition(try! LedgerLaunchProfile.identifier(arguments: ["app", "--isolated-profile", identifier]) == identifier)
        for arguments in [
            ["app", "--isolated-profile"],
            ["app", "--isolated-profile", "../real-ledger"],
            ["app", "--isolated-profile=\(identifier)"],
            ["app", "--isolated-profiel", identifier],
            ["app", "--isolated-profile", identifier, "--isolated-profile", identifier],
        ] {
            do { _ = try LedgerLaunchProfile.identifier(arguments: arguments); preconditionFailure("invalid profile accepted") }
            catch ServiceConfigurationError.invalidIsolatedProfile { }
            catch { preconditionFailure("unexpected profile error") }
        }
        let normal = try! LedgerRuntimePaths.resolve(arguments: ["app"])
        let isolated = try! LedgerRuntimePaths.resolve(arguments: ["app", "--isolated-profile", identifier])
        precondition(normal.codexHome == nil && normal.isolatedProfile == nil)
        precondition(isolated.database != normal.database)
        precondition(isolated.binary == normal.binary && isolated.webRoot == normal.webRoot)
        let root = isolated.applicationSupportDirectory.deletingLastPathComponent()
        precondition(root.lastPathComponent == "CodexUsageLedgerPreview-\(identifier)")
        precondition(isolated.codexHome!.deletingLastPathComponent() == root)
        for mode in [LedgerServiceMode.serve, .daemon] {
            let arguments = isolated.serviceArguments(mode: mode)
            precondition(arguments.first == mode.rawValue)
            precondition(arguments.contains("127.0.0.1:47127"))
            precondition(arguments.suffix(2) == ["--codex-home", isolated.codexHome!.path])
            precondition(!arguments.contains(normal.database.path))
        }
        let inherited = ["CODEX_HOME": "/synthetic/source", "CODEX_USAGE_LEDGER_DB": "/synthetic/ledger", "CODEX_USAGE_LEDGER_WEB_ROOT": "/synthetic/web", "PATH": "/synthetic/bin"]
        let isolatedEnvironment = isolated.serviceEnvironment(inherited: inherited)
        precondition(isolatedEnvironment["CODEX_HOME"] == nil)
        precondition(isolatedEnvironment["CODEX_USAGE_LEDGER_DB"] == nil)
        precondition(isolatedEnvironment["CODEX_USAGE_LEDGER_WEB_ROOT"] == nil)
        precondition(isolatedEnvironment["PATH"] == inherited["PATH"])
        precondition(normal.serviceEnvironment(inherited: inherited)["CODEX_HOME"] == inherited["CODEX_HOME"])
        defer { try? FileManager.default.removeItem(at: root) }
        try! isolated.secureApplicationSupportDirectory()
        let permissions = try! FileManager.default.attributesOfItem(atPath: root.path)[.posixPermissions] as! NSNumber
        precondition(permissions.intValue == 0o700)
        try! FileManager.default.createSymbolicLink(atPath: isolated.database.path, withDestinationPath: root.appendingPathComponent("absent-fixture").path)
        do { try isolated.secureApplicationSupportDirectory(); preconditionFailure("linked profile accepted") }
        catch ServiceConfigurationError.linkedIsolatedProfile { }
        catch { preconditionFailure("unexpected link error") }
    }
}
