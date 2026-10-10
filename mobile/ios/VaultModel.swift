import Foundation
import SwiftUI

/// App state: one unlocked vault session, or none.
@MainActor
final class VaultModel: ObservableObject {
    @Published private(set) var entries: [VaultCore.EntrySummary] = []
    @Published private(set) var audit: VaultCore.Audit?
    @Published var search = ""
    @Published var errorMessage: String?
    @Published var isWorking = false
    @Published var copiedTitle: String?

    /// The core owns the decrypted vault; releasing this closes it.
    private var session: VaultSession?

    var isUnlocked: Bool { session != nil }
    var isEmpty: Bool { entries.isEmpty }

    var vaultURL: URL {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return docs.appendingPathComponent("unbundio-vault.vault")
    }

    var needsSetup: Bool { !FileManager.default.fileExists(atPath: vaultURL.path) }

    /// Case-insensitive filter over title / username / url.
    var filtered: [VaultCore.EntrySummary] {
        let q = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return entries }
        return entries.filter {
            $0.title.lowercased().contains(q)
                || $0.username.lowercased().contains(q)
                || ($0.url?.lowercased().contains(q) ?? false)
        }
    }

    func unlock(password: String) async {
        isWorking = true
        errorMessage = nil
        defer { isWorking = false }

        do {
            if needsSetup {
                try VaultCore.create(path: vaultURL.path, password: password)
                // Opt in to Face ID unlock for next time.
                try? KeychainVault.shared.storeMasterPassword(password)
            }
            let opened = try VaultCore.open(path: vaultURL.path, password: password)
            session = opened
            try reload()
        } catch {
            session = nil
            errorMessage = error.localizedDescription
        }
    }

    func reload() throws {
        guard let session else { return }
        entries = try session.list()
        audit = try session.audit()
    }

    /// Reveal one entry (the only call that returns a password).
    func reveal(id: String) -> VaultCore.Entry? {
        guard let session else { return nil }
        return try? session.entry(id: id)
    }

    func generatePassword(length: Int = 20) -> String {
        VaultCore.generate(length: length)
    }

    func copy(_ text: String, title: String) {
        UIPasteboard.general.string = text
        copiedTitle = title
        // Clear quickly, like the desktop extension does.
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
            if UIPasteboard.general.string == text {
                UIPasteboard.general.string = ""
            }
        }
    }

    func lock() {
        session = nil
        entries = []
        audit = nil
        search = ""
    }
}