import Foundation

enum LedgerServiceMode: String, Equatable {
    case serve
    case daemon

    var displayName: String {
        switch self {
        case .serve: return NativeLocalization.text("只读看板", "Read-only dashboard")
        case .daemon: return NativeLocalization.text("正在采集", "Collecting")
        }
    }
}

enum LedgerServiceState: Equatable {
    case stopped
    case starting(LedgerServiceMode)
    case running(LedgerServiceMode)
    case stopping(next: LedgerServiceMode?)
    case failed(String)

    var mode: LedgerServiceMode? {
        switch self {
        case .starting(let mode), .running(let mode): return mode
        case .stopping(let next): return next
        case .stopped, .failed: return nil
        }
    }

    var isReady: Bool {
        if case .running = self { return true }
        return false
    }

    var statusText: String {
        switch self {
        case .stopped:
            return NativeLocalization.text("已停止", "Stopped")
        case .starting(let mode):
            return "\(NativeLocalization.text("正在启动", "Starting")) \(mode.displayName)…"
        case .running(let mode):
            return mode.displayName
        case .stopping(let next):
            return next == nil ? NativeLocalization.text("正在停止…", "Stopping…") : NativeLocalization.text("正在切换模式…", "Switching mode…")
        case .failed:
            return NativeLocalization.text("本地服务不可用", "Local service unavailable")
        }
    }

    var symbolName: String {
        switch self {
        case .running(.daemon): return "waveform.path.ecg"
        case .running(.serve): return "rectangle.connected.to.line.below"
        case .starting, .stopping: return "clock.arrow.circlepath"
        case .failed: return "exclamationmark.triangle"
        case .stopped: return "stop.circle"
        }
    }
}
