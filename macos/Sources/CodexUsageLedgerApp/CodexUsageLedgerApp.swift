import AppKit
import SwiftUI

@MainActor
final class CodexUsageLedgerAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.regular)
        LedgerServiceController.shared.startPreferredMode()
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationWillTerminate(_ notification: Notification) {
        LedgerServiceController.shared.stopForApplicationExit()
    }
}

@main
@MainActor
struct CodexUsageLedgerApp: App {
    @NSApplicationDelegateAdaptor(CodexUsageLedgerAppDelegate.self) private var appDelegate
    @StateObject private var service = LedgerServiceController.shared

    var body: some Scene {
        Window("Codex Usage Ledger", id: "dashboard") {
            DashboardWindowView(service: service)
        }
        .defaultSize(width: 1280, height: 820)
        .windowResizability(.automatic)
        .windowToolbarStyle(.unifiedCompact)
        .commands {
            CommandGroup(replacing: .newItem) { }
            CommandGroup(after: .toolbar) {
                Divider()
                Button(NativeLocalization.text("放大", "Zoom in")) {
                    service.zoomIn()
                }
                .keyboardShortcut("+", modifiers: .command)
                .disabled(!service.canZoomIn)

                Button(NativeLocalization.text("缩小", "Zoom out")) {
                    service.zoomOut()
                }
                .keyboardShortcut("-", modifiers: .command)
                .disabled(!service.canZoomOut)

                Button(NativeLocalization.text("实际大小", "Actual size")) {
                    service.resetPageZoom()
                }
                .keyboardShortcut("0", modifiers: .command)
            }
            CommandMenu(NativeLocalization.text("采集", "Collection")) {
                Button(service.collectionMenuTitle) {
                    service.toggleCollectionWithConfirmation()
                }
            }
        }

        MenuBarExtra {
            LedgerMenuBarContent(service: service)
        } label: {
            Label("Codex Usage Ledger", systemImage: service.state.symbolName)
        }
        .menuBarExtraStyle(.menu)
    }
}
