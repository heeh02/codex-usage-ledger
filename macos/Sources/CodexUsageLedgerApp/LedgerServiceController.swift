import AppKit
import Combine
import Darwin
import Foundation

@MainActor
final class LedgerServiceController: ObservableObject {
    static let shared = LedgerServiceController()

    static let dashboardURL = URL(string: "http://127.0.0.1:47127/")!
    private static let healthURL = URL(string: "http://127.0.0.1:47127/healthz")!
    private static let collectionDefaultsKey = "collectionEnabled"
    private static let pageZoomDefaultsKey = "pageZoom"
    private static let defaultPageZoom = 1.0
    private static let minimumPageZoom = 0.80
    private static let maximumPageZoom = 1.60
    private static let pageZoomSteps = [0.80, 0.90, 1.0, 1.10, 1.25, 1.40, 1.60]

    @Published private(set) var state: LedgerServiceState = .stopped
    @Published private(set) var collectionEnabled: Bool
    @Published private(set) var diagnostics = ""
    @Published private(set) var reloadToken = UUID()
    @Published private(set) var pageZoom: Double
    @Published private(set) var uiLanguage: String

    private let defaults: UserDefaults
    private var child: Process?
    private var childMode: LedgerServiceMode?
    private var standardOutputPipe: Pipe?
    private var standardErrorPipe: Pipe?
    private var pendingMode: LedgerServiceMode?
    private var generation = 0
    private var expectedTermination = false
    private var failureAfterTermination: String?
    private var applicationIsTerminating = false
    private var diagnosticBuffer = LedgerProcessDiagnostics()

    private init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        uiLanguage = defaults.string(forKey: NativeLocalization.defaultsKey) ?? NativeLocalization.language
        collectionEnabled = defaults.bool(forKey: Self.collectionDefaultsKey)
        if defaults.object(forKey: Self.pageZoomDefaultsKey) != nil {
            pageZoom = min(
                max(defaults.double(forKey: Self.pageZoomDefaultsKey), Self.minimumPageZoom),
                Self.maximumPageZoom
            )
        } else {
            pageZoom = Self.defaultPageZoom
        }
    }

    var isCollecting: Bool {
        switch state {
        case .starting(.daemon), .running(.daemon): return true
        default: return false
        }
    }

    var collectionMenuTitle: String {
        collectionEnabled
            ? NativeLocalization.text("停止采集", "Stop collection")
            : NativeLocalization.text("开始采集…", "Start collection…")
    }

    func updateUILanguage(_ language: String) {
        guard language == "zh-CN" || language == "en", language != uiLanguage else { return }
        defaults.set(language, forKey: NativeLocalization.defaultsKey)
        uiLanguage = language
    }

    var pageZoomPercent: Int {
        Int((pageZoom * 100).rounded())
    }

    var canZoomIn: Bool { pageZoom < Self.maximumPageZoom }
    var canZoomOut: Bool { pageZoom > Self.minimumPageZoom }

    func zoomIn() {
        if let next = Self.pageZoomSteps.first(where: { $0 > pageZoom + 0.001 }) {
            setPageZoom(next)
        }
    }

    func zoomOut() {
        if let next = Self.pageZoomSteps.last(where: { $0 < pageZoom - 0.001 }) {
            setPageZoom(next)
        }
    }

    func resetPageZoom() {
        setPageZoom(1.0)
    }

    private func setPageZoom(_ value: Double) {
        let rounded = (value * 20).rounded() / 20
        let next = min(max(rounded, Self.minimumPageZoom), Self.maximumPageZoom)
        guard next != pageZoom else { return }
        pageZoom = next
        defaults.set(next, forKey: Self.pageZoomDefaultsKey)
    }

    var failureMessage: String? {
        if case .failed(let message) = state { return message }
        return nil
    }

    func startPreferredMode() {
        guard child == nil || child?.isRunning == false else { return }
        transition(to: collectionEnabled ? .daemon : .serve)
    }

    func retryCurrentPreference() {
        transition(to: collectionEnabled ? .daemon : .serve)
    }

    func reloadDashboard() {
        guard state.isReady else { return }
        reloadToken = UUID()
    }

    func toggleCollectionWithConfirmation() {
        if collectionEnabled {
            defaults.set(false, forKey: Self.collectionDefaultsKey)
            collectionEnabled = false
            transition(to: .serve)
            return
        }

        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = NativeLocalization.text("开始采集 Codex 用量？", "Start collecting Codex usage?")
        alert.informativeText = NativeLocalization.text(
            "首次采集可能需要扫描约 73 GB 的本机 Codex 会话日志，耗时和磁盘读取量都可能较大。\n\n应用会把内置服务从只读看板切换到持续采集模式；Swift 外壳本身不会读取或写入 Codex 登录凭据。",
            "The first collection may scan about 73 GB of local Codex session logs and can take significant time and disk I/O.\n\nThe bundled service switches from read-only dashboard mode to continuous collection. The Swift shell never reads or writes Codex login credentials."
        )
        alert.addButton(withTitle: NativeLocalization.text("开始采集", "Start collection"))
        alert.addButton(withTitle: NativeLocalization.text("取消", "Cancel"))
        alert.buttons.first?.hasDestructiveAction = true

        guard alert.runModal() == .alertFirstButtonReturn else { return }

        defaults.set(true, forKey: Self.collectionDefaultsKey)
        collectionEnabled = true
        transition(to: .daemon)
    }

    func stopForApplicationExit() {
        applicationIsTerminating = true
        pendingMode = nil
        expectedTermination = true
        detachPipeHandlers()

        guard let child, child.isRunning else {
            self.child = nil
            state = .stopped
            return
        }

        child.terminate()
        let deadline = Date().addingTimeInterval(0.8)
        while child.isRunning && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.025))
        }
        if child.isRunning {
            Darwin.kill(child.processIdentifier, SIGKILL)
        }
        self.child = nil
        childMode = nil
        state = .stopped
    }

    private func transition(to mode: LedgerServiceMode) {
        switch LedgerServiceLifecycle.decision(
            applicationIsTerminating: applicationIsTerminating,
            processIsRunning: child?.isRunning == true,
            currentMode: childMode,
            requestedMode: mode
        ) {
        case .ignore:
            return
        case .stopThenLaunch(let nextMode):
            guard let child else { return }
            pendingMode = nextMode
            expectedTermination = true
            state = .stopping(next: nextMode)
            child.terminate()
        case .launch(let launchMode):
            launch(launchMode)
        }
    }

    private func launch(_ mode: LedgerServiceMode) {
        do {
            let paths = try LedgerRuntimePaths.resolve()
            try paths.validateBundledResources()
            try paths.secureApplicationSupportDirectory()

            let process = Process()
            process.executableURL = paths.binary
            process.currentDirectoryURL = paths.applicationSupportDirectory
            process.arguments = [
                mode.rawValue,
                "--db", paths.database.path,
                "--listen", "127.0.0.1:47127",
                "--web-root", paths.webRoot.path,
            ]

            var environment = ProcessInfo.processInfo.environment
            environment["NO_COLOR"] = "1"
            environment["RUST_LOG"] = environment["RUST_LOG"] ?? "codex_usage_ledger=info"
            process.environment = environment

            let outputPipe = Pipe()
            let errorPipe = Pipe()
            process.standardOutput = outputPipe
            process.standardError = errorPipe
            installReadabilityHandler(on: outputPipe)
            installReadabilityHandler(on: errorPipe)

            generation += 1
            let launchedGeneration = generation
            process.terminationHandler = { [weak self] terminatedProcess in
                Task { @MainActor [weak self] in
                    self?.handleTermination(of: terminatedProcess, generation: launchedGeneration)
                }
            }

            diagnosticBuffer.reset()
            diagnostics = ""
            standardOutputPipe = outputPipe
            standardErrorPipe = errorPipe
            child = process
            childMode = mode
            pendingMode = nil
            expectedTermination = false
            failureAfterTermination = nil
            state = .starting(mode)

            try process.run()

            Task { @MainActor [weak self] in
                await self?.waitUntilHealthy(generation: launchedGeneration)
            }
        } catch {
            detachPipeHandlers()
            child = nil
            childMode = nil
            state = .failed(error.localizedDescription)
        }
    }

    private func waitUntilHealthy(generation expectedGeneration: Int) async {
        for _ in 0..<40 {
            guard generation == expectedGeneration,
                  let child,
                  child.isRunning,
                  let mode = childMode else { return }

            var request = URLRequest(url: Self.healthURL)
            request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            request.timeoutInterval = 0.45
            do {
                let (data, response) = try await URLSession.shared.data(for: request)
                let identity = try? JSONDecoder().decode(HealthIdentity.self, from: data)
                if let http = response as? HTTPURLResponse,
                   (200..<300).contains(http.statusCode),
                   identity?.service == "codex-usage-ledger",
                   identity?.status == "ok",
                   child.isRunning {
                    state = .running(mode)
                    reloadToken = UUID()
                    try? LedgerRuntimePaths.resolve().secureDatabasePermissionsIfPresent()
                    return
                }
            } catch {
                // Startup commonly refuses connections for a short period. Retry locally.
            }

            try? await Task.sleep(nanoseconds: 200_000_000)
        }

        guard generation == expectedGeneration, let child, child.isRunning else { return }
        let message = NativeLocalization.text("内置服务未能在 127.0.0.1:47127 就绪；该端口可能正被其他进程占用。", "The bundled service did not become ready at 127.0.0.1:47127; another process may be using the port.")
        failureAfterTermination = message
        expectedTermination = true
        state = .failed(message)
        child.terminate()
    }

    private struct HealthIdentity: Decodable {
        let service: String
        let status: String
    }

    private func handleTermination(of terminatedProcess: Process, generation terminatedGeneration: Int) {
        guard terminatedGeneration == generation else { return }

        detachPipeHandlers()
        child = nil
        let previousMode = childMode
        childMode = nil

        if applicationIsTerminating {
            state = .stopped
            return
        }

        if let nextMode = pendingMode {
            pendingMode = nil
            expectedTermination = false
            launch(nextMode)
            return
        }

        if let failureAfterTermination {
            self.failureAfterTermination = nil
            expectedTermination = false
            state = .failed(failureAfterTermination)
            return
        }

        if expectedTermination {
            expectedTermination = false
            state = .stopped
            return
        }

        let modeName = previousMode?.displayName ?? NativeLocalization.text("本地服务", "Local service")
        let detail = diagnosticBuffer.tail(lineCount: 2)
        let suffix = detail.isEmpty ? "" : " \(detail)"
        let message = NativeLocalization.language == "zh-CN"
            ? "\(modeName)异常退出（状态 \(terminatedProcess.terminationStatus)）。\(suffix)"
            : "\(modeName) exited unexpectedly (status \(terminatedProcess.terminationStatus)).\(suffix)"
        state = .failed(message)
    }

    private func installReadabilityHandler(on pipe: Pipe) {
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                return
            }
            guard let text = String(data: data, encoding: .utf8) else { return }
            Task { @MainActor [weak self] in
                self?.appendDiagnostic(text)
            }
        }
    }

    private func appendDiagnostic(_ text: String) {
        diagnostics = diagnosticBuffer.append(text)
    }

    private func detachPipeHandlers() {
        standardOutputPipe?.fileHandleForReading.readabilityHandler = nil
        standardErrorPipe?.fileHandleForReading.readabilityHandler = nil
        standardOutputPipe = nil
        standardErrorPipe = nil
    }

}
