enum LedgerLifecycleDecision: Equatable {
    case ignore
    case launch(LedgerServiceMode)
    case stopThenLaunch(LedgerServiceMode)
    case updatePendingMode(LedgerServiceMode)
}

enum LedgerServiceLifecycle {
    static func decision(
        applicationIsTerminating: Bool,
        processIsRunning: Bool,
        currentMode: LedgerServiceMode?,
        requestedMode: LedgerServiceMode,
        stopInProgress: Bool = false
    ) -> LedgerLifecycleDecision {
        if applicationIsTerminating {
            return .ignore
        }
        if processIsRunning {
            if stopInProgress { return .updatePendingMode(requestedMode) }
            return currentMode == requestedMode ? .ignore : .stopThenLaunch(requestedMode)
        }
        return .launch(requestedMode)
    }
}
