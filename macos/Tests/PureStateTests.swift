import Foundation
import SwiftUI
import WebKit

@main
struct PureStateTests {
    static func main() {
        testIsolatedProfile()
        testLanguageBootstrap()
        testWebViewBootstrapScripts()
        for mode in [LedgerServiceMode.serve, .daemon] {
            precondition(LedgerServiceLifecycle.decision(applicationIsTerminating: false, processIsRunning: true,
                currentMode: .serve, requestedMode: mode, stopInProgress: true) == .updatePendingMode(mode))
        }
        precondition(LedgerServiceLifecycle.decision(applicationIsTerminating: true, processIsRunning: true,
            currentMode: .serve, requestedMode: .daemon, stopInProgress: true) == .ignore)
        let own = Data(#"{"service":"codex-usage-ledger","status":"ok","processId":1234}"#.utf8)
        precondition(LedgerHealthIdentity.matches(own, statusCode: 200, expectedProcessId: 1234))
        precondition(!LedgerHealthIdentity.matches(own, statusCode: 503, expectedProcessId: 1234))
        precondition(!LedgerHealthIdentity.matches(own, statusCode: 200, expectedProcessId: 4321))
        precondition(!LedgerHealthIdentity.matches(own, statusCode: 200, expectedProcessId: 0))
        for text in [#"{"service":"codex-usage-ledger","status":"ok"}"#,
                     #"{"service":"other","status":"ok","processId":1234}"#,
                     #"{"service":"codex-usage-ledger","status":"failed","processId":1234}"#,
                     #"{"service":"codex-usage-ledger","status":"ok","processId":"1234"}"#] {
            precondition(!LedgerHealthIdentity.matches(Data(text.utf8), statusCode: 200, expectedProcessId: 1234))
        }
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

    static func testLanguageBootstrap() {
        var bootstrap = DashboardLanguageBootstrap()
        precondition(bootstrap.language == nil)
        let english = bootstrap.update("en")!
        precondition(english.contains("'ledger.language', 'en'"))
        precondition(bootstrap.language == .english)
        precondition(bootstrap.update("en") == nil, "unrelated SwiftUI updates must not replace scripts")
        let chinese = bootstrap.update("zh-CN")!
        precondition(chinese.contains("'ledger.language', 'zh-CN'"))
        precondition(bootstrap.language == .simplifiedChinese)
        precondition(bootstrap.update("zh-CN") == nil)
        precondition(bootstrap.update("invalid'); alert(1); //") == nil)
        precondition(bootstrap.update("en") == english)
        var reopened = DashboardLanguageBootstrap()
        precondition(reopened.update("en") == english, "a new view must receive the saved language at document start")
        var invalid = DashboardLanguageBootstrap()
        let fallback = invalid.update("invalid'); alert(1); //")!
        precondition(fallback == chinese && !fallback.contains("alert("))
    }

    static func testWebViewBootstrapScripts() {
        let controller = WKUserContentController()
        let coordinator = LockedDashboardWebView.Coordinator(
            allowedURL: URL(string: "http://127.0.0.1:47127/")!,
            isLoaded: .constant(false), onLanguageChange: { _ in }
        )
        coordinator.updateBootstrapLanguage("en", in: controller)
        let original = controller.userScripts
        precondition(original.count == 2)
        precondition(original[0].source.contains("'ledger.language', 'en'"))
        let policy = original[1].source
        precondition(policy.contains("frame-src 'none'"))
        precondition(policy.contains("connect-src 'self' http://127.0.0.1:47127"))
        coordinator.updateBootstrapLanguage("en", in: controller)
        precondition(controller.userScripts[0] === original[0], "unchanged language must retain scripts")
        coordinator.updateBootstrapLanguage("zh-CN", in: controller)
        precondition(controller.userScripts.count == 2)
        precondition(controller.userScripts[0].source.contains("'ledger.language', 'zh-CN'"))
        precondition(controller.userScripts[1].source == policy, "language update must preserve CSP")
        for script in controller.userScripts {
            precondition(script.injectionTime == .atDocumentStart && script.isForMainFrameOnly)
        }
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
            ["app", "--isolated-union"],
            ["app", "--isolated-profile", identifier, "--isolated-union", "--isolated-union"],
        ] {
            do { _ = try LedgerLaunchProfile.identifier(arguments: arguments); preconditionFailure("invalid profile accepted") }
            catch ServiceConfigurationError.invalidIsolatedProfile { }
            catch { preconditionFailure("unexpected profile error") }
        }
        let normal = try! LedgerRuntimePaths.resolve(arguments: ["app"])
        let isolated = try! LedgerRuntimePaths.resolve(arguments: ["app", "--isolated-profile", identifier])
        let union = try! LedgerRuntimePaths.resolve(arguments: ["app", "--isolated-profile", identifier, "--isolated-union"])
        precondition(union.unionPreview && union.database == isolated.database)
        precondition(!normal.unionPreview && !isolated.unionPreview)
        for mode in [LedgerServiceMode.serve, .daemon] {
            let args = union.serviceArguments(mode: mode)
            precondition(args.first == "serve" && args.contains("--union-preview"))
            precondition(!args.contains("--codex-home") && !args.contains(normal.database.path))
        }
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
