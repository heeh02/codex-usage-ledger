import AppKit
import SwiftUI

struct DashboardWindowView: View {
    @ObservedObject var service: LedgerServiceController
    @State private var dashboardLoaded = false

    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)
                .ignoresSafeArea()

            LockedDashboardWebView(
                url: LedgerServiceController.dashboardURL,
                reloadToken: service.reloadToken,
                pageZoom: service.pageZoom,
                initialLanguage: service.uiLanguage,
                onLanguageChange: service.updateUILanguage,
                isLoaded: $dashboardLoaded
            )
            .opacity(service.state.isReady && dashboardLoaded ? 1 : 0)

            if !service.state.isReady || !dashboardLoaded {
                servicePlaceholder
            }
        }
        .frame(minWidth: 560, minHeight: 520)
        .onAppear {
            DispatchQueue.main.async {
                NSApplication.shared.windows
                    .first(where: { $0.identifier?.rawValue == "dashboard" })?
                    .minSize = NSSize(width: 560, height: 560)
            }
        }
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Image(systemName: service.state.symbolName)
                    .font(.caption)
                    .foregroundStyle(statusColor)
                    .accessibilityLabel("\(NativeLocalization.text("本地服务状态", "Local service status")): \(service.state.statusText)")
                    .help(service.state.statusText)
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    service.reloadDashboard()
                } label: {
                    Label(NativeLocalization.text("刷新看板", "Refresh dashboard"), systemImage: "arrow.clockwise")
                }
                .disabled(!service.state.isReady)

                Menu {
                    Button(NativeLocalization.text("放大", "Zoom in")) { service.zoomIn() }
                        .disabled(!service.canZoomIn)
                    Button(NativeLocalization.text("缩小", "Zoom out")) { service.zoomOut() }
                        .disabled(!service.canZoomOut)
                    Divider()
                    Button(NativeLocalization.text("实际大小", "Actual size")) { service.resetPageZoom() }
                } label: {
                    Label("\(NativeLocalization.text("页面缩放", "Page zoom")) \(service.pageZoomPercent)%", systemImage: "textformat.size")
                }
                .accessibilityLabel("\(NativeLocalization.text("页面缩放", "Page zoom")) \(service.pageZoomPercent)%")
                .help("\(NativeLocalization.text("页面缩放", "Page zoom")) \(service.pageZoomPercent)%")

                Button {
                    service.toggleCollectionWithConfirmation()
                } label: {
                    Label(
                        service.collectionMenuTitle,
                        systemImage: service.collectionEnabled ? "stop.fill" : "play.fill"
                    )
                }
            }
        }
    }

    private var servicePlaceholder: some View {
        VStack(spacing: 16) {
            Image(systemName: service.state.symbolName)
                .font(.system(size: 38, weight: .light))
                .foregroundStyle(statusColor)

            VStack(spacing: 7) {
                Text(service.state.statusText)
                    .font(.title2.weight(.semibold))
                Text(placeholderDetail)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 540)
            }

            if service.failureMessage != nil || service.state == .stopped {
                Button(NativeLocalization.text("重试本地服务", "Retry local service")) {
                    service.retryCurrentPreference()
                }
                .buttonStyle(.borderedProminent)
            } else {
                ProgressView()
                    .controlSize(.small)
            }

            if !service.diagnostics.isEmpty, service.failureMessage != nil {
                DisclosureGroup(NativeLocalization.text("服务诊断", "Service diagnostics")) {
                    ScrollView {
                        Text(service.diagnostics)
                            .font(.system(.caption2, design: .monospaced))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(10)
                    }
                    .frame(maxWidth: 620, maxHeight: 150)
                    .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 8))
                }
                .frame(maxWidth: 620)
            }
        }
        .padding(36)
    }

    private var placeholderDetail: String {
        if let failure = service.failureMessage {
            return failure
        }
        switch service.state {
        case .stopped:
            return NativeLocalization.text("本地看板服务已停止；重试会恢复上次确认的运行模式。", "The local dashboard service has stopped. Retry restores the last confirmed mode.")
        case .starting(.serve):
            return NativeLocalization.text("正在打开已有账本，不扫描 Codex 会话日志。", "Opening the existing ledger without scanning Codex session logs.")
        case .starting(.daemon):
            return NativeLocalization.text("正在按已保存的授权启动采集；看板会在历史增量同步完成前先行可用。", "Starting collection with saved authorization. The dashboard becomes available before historical sync completes.")
        case .stopping:
            return NativeLocalization.text("正在等待先前的本地进程释放 127.0.0.1:47127。", "Waiting for the previous local process to release 127.0.0.1:47127.")
        case .running:
            return dashboardLoaded ? "" : NativeLocalization.text("正在准备本机看板…", "Preparing the local dashboard…")
        case .failed:
            return ""
        }
    }

    private var statusColor: Color {
        switch service.state {
        case .running(.serve): return .blue
        case .running(.daemon): return .green
        case .failed: return .red
        case .starting, .stopping: return .orange
        case .stopped: return .secondary
        }
    }
}
