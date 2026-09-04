enum LedgerLifecycleDecision: Equatable {
    case ignore
    case launch(LedgerServiceMode)
    case stopThenLaunch(LedgerServiceMode)
}

enum LedgerServiceLifecycle {
    static func decision(
        applicationIsTerminating: Bool,
        processIsRunning: Bool,
        currentMode: LedgerServiceMode?,
        requestedMode: LedgerServiceMode
    ) -> LedgerLifecycleDecision {
        if applicationIsTerminating {
            return .ignore
        }
        if processIsRunning {
            return currentMode == requestedMode ? .ignore : .stopThenLaunch(requestedMode)
        }
        return .launch(requestedMode)
    }
}
