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

    /// Face ID straight after launch, the way the competition does it: you
    /// should not have to tap a button before the prompt appears.
    func autoUnlockOnLaunch() async {
        guard !isUnlocked, !needsSetup else { return }
        guard Biometrics.isAvailable, KeychainVault.shared.masterPassword() != nil else { return }
        await unlockWithBiometrics()
    }

    func unlockWithBiometrics() async {
        guard await Biometrics.authenticate(reason: "Unlock your vault") else {
            errorMessage = "Biometric unlock was cancelled."
            return
        }
        guard let stored = KeychainVault.shared.masterPassword() else {
            errorMessage = "No stored password for Face ID. Use your master password once to enable it."
            return
        }
        await unlock(password: stored)
    }

    /// DEBUG-only hook so the UI can be verified in a simulator, where there is
    /// no keyboard to type a master password into. Compiled out of release
    /// builds entirely, and only active when the launch environment names it.
    #if DEBUG
    func unlockForUITestIfNeeded() async {
        guard !isUnlocked,
              let pw = ProcessInfo.processInfo.environment["UV_UI_TEST_MASTER_PASSWORD"]
        else { return }
        await unlock(password: pw)
    }
    #endif
}