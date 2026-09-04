import Foundation

@main
struct PureStateTests {
    static func main() {
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
}
