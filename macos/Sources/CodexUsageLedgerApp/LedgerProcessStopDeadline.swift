import Foundation
import Darwin

@MainActor
enum LedgerProcessStopDeadline {
    static func stop(
        _ process: Process,
        graceNanoseconds: UInt64 = 800_000_000,
        stillOwned: @escaping @MainActor () -> Bool,
        onFailure: @escaping @MainActor (Int32) -> Void
    ) -> Task<Void, Never> {
        if process.isRunning && stillOwned() { process.terminate() }
        return Task { @MainActor in
            do { try await Task.sleep(nanoseconds: graceNanoseconds) }
            catch { return }
            guard !Task.isCancelled, stillOwned(), process.isRunning else { return }
            // Only the original, still-owned live child can be escalated.
            // Its termination handler owns the subsequent mode transition.
            if Darwin.kill(process.processIdentifier, SIGKILL) != 0 {
                let code = errno
                if process.isRunning { onFailure(code) }
            }
        }
    }
}
