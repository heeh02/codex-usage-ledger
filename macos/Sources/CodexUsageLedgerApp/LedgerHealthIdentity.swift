import Foundation

struct LedgerHealthIdentity: Decodable {
    let service: String
    let status: String
    let processId: Int32

    static func matches(_ data: Data, statusCode: Int, expectedProcessId: Int32) -> Bool {
        guard (200..<300).contains(statusCode), expectedProcessId > 0,
              let identity = try? JSONDecoder().decode(Self.self, from: data) else { return false }
        return identity.service == "codex-usage-ledger" && identity.status == "ok"
            && identity.processId == expectedProcessId
    }
}
