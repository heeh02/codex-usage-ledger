import Foundation

struct LedgerProcessDiagnostics {
    private static let maximumCharacters = 16_000
    private(set) var text = ""

    mutating func reset() {
        text = ""
    }

    mutating func append(_ fragment: String) -> String {
        text.append(fragment)
        if text.count > Self.maximumCharacters {
            text = String(text.suffix(Self.maximumCharacters))
        }
        return text
    }

    func tail(lineCount: Int) -> String {
        text
            .split(separator: "\n", omittingEmptySubsequences: true)
            .suffix(max(0, lineCount))
            .joined(separator: " ")
    }
}
