import AppKit
import SwiftUI

struct LedgerMenuBarContent: View {
    @ObservedObject var service: LedgerServiceController
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button(NativeLocalization.text("打开看板", "Open dashboard")) {
            openDashboard()
        }
        .keyboardShortcut("o")

        Divider()

        Button(service.collectionMenuTitle) {
            service.toggleCollectionWithConfirmation()
        }

        Button(NativeLocalization.text("刷新看板", "Refresh dashboard")) {
            service.reloadDashboard()
        }
        .disabled(!service.state.isReady)

        Divider()

        Label("\(NativeLocalization.text("状态", "Status")): \(service.state.statusText)", systemImage: service.state.symbolName)
            .foregroundStyle(.secondary)

        if service.collectionEnabled && !service.isCollecting {
            Text(NativeLocalization.text("采集已启用，但本地服务当前未运行", "Collection is enabled, but the local service is not running"))
                .foregroundStyle(.secondary)
        }

        Divider()

        Button(NativeLocalization.text("退出 Codex Usage Ledger", "Quit Codex Usage Ledger")) {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }

    private func openDashboard() {
        openWindow(id: "dashboard")
        NSApplication.shared.activate(ignoringOtherApps: true)
        NSApplication.shared.windows
            .first(where: { $0.title == "Codex Usage Ledger" })?
            .makeKeyAndOrderFront(nil)
    }
}
