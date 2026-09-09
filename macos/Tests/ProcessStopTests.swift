import Foundation
import Darwin

@main
struct ProcessStopTests {
    enum Failure: Error { case assertion(String) }
    static func check(_ condition: Bool, _ message: String) throws {
        if !condition { throw Failure.assertion(message) }
    }
    @MainActor
    static func main() async throws {
        for scenario in ["graceful", "ignores-term", "cancelled", "not-owned", "ownership-revoked"] {
            let process = Process()
            let ready = Pipe()
            process.standardOutput = ready
            process.executableURL = URL(fileURLWithPath: "/bin/sh")
            process.arguments = ["-c", scenario == "graceful" ? "printf ready; exec /bin/sleep 30" : "trap '' TERM; printf ready; exec /bin/sleep 30"]
            try process.run()
            defer {
                if process.isRunning { Darwin.kill(process.processIdentifier, SIGKILL) }
                process.waitUntilExit()
            }
            try check(ready.fileHandleForReading.readData(ofLength: 5) == Data("ready".utf8), "child readiness")
            var stopError: Int32?
            var owned = scenario != "not-owned"
            let task = LedgerProcessStopDeadline.stop(process, graceNanoseconds: 100_000_000,
                stillOwned: { owned }, onFailure: { stopError = $0 })
            if scenario == "cancelled" { task.cancel() }
            if scenario == "ownership-revoked" { owned = false }
            await task.value
            if scenario == "cancelled" || scenario == "not-owned" || scenario == "ownership-revoked" {
                try check(process.isRunning, "\(scenario) must not be killed")
            } else {
                for _ in 0..<100 where process.isRunning {
                    try await Task.sleep(nanoseconds: 10_000_000)
                }
                try check(!process.isRunning, "\(scenario) must finish")
                try check(process.terminationStatus == (scenario == "graceful" ? SIGTERM : SIGKILL), "expected stop signal")
            }
            try check(stopError == nil, "no stop errors")
        }
        print("Swift owned-process stop tests passed.")
    }
}
