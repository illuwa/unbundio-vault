import LocalAuthentication
import SwiftUI

/// Onboarding / unlock screen. Biometrics gate the master password so the
/// user does not type it on every launch.
struct UnlockView: View {
    @StateObject var model: VaultModel

    @State private var masterPassword = ""
    @State private var showingPassword = false

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Image(systemName: "lock.shield")
                .font(.system(size: 56))
                .foregroundStyle(.tint)

            VStack(spacing: 6) {
                Text("unbundio-vault")
                    .font(.title.bold())
                Text("local · no account · no cloud")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if model.needsSetup {
                TextField("Master password", text: $masterPassword)
                    .textContentType(.newPassword)
                    .textFieldStyle(.roundedBorder)
                Text("Pick this once — it encrypts the vault on this device.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Button {
                    Task { await unlockWithBiometrics() }
                } label: {
                    Label("Unlock with Face ID", systemImage: "faceid")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                HStack {
                    Button("Use password") { showingPassword.toggle() }
                        .font(.caption)
                }
                if showingPassword {
                    SecureField("Master password", text: $masterPassword)
                        .textContentType(.password)
                        .textFieldStyle(.roundedBorder)
                }
            }

            if let error = model.errorMessage {
                Text(error)
                    .font(.footnote)
                    .foregroundStyle(.red)
                    .multilineTextAlignment(.center)
            }

            Button(model.needsSetup ? "Create vault" : "Unlock") {
                Task { await model.unlock(password: masterPassword) }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(masterPassword.isEmpty || model.isWorking)

            if model.isWorking {
                ProgressView()
            }

            Spacer()

            Text("Synced vault files and decryption live in the Rust core.")
                .font(.caption2)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(28)
    }

    /// Ask Face ID, then let Keychain hand back the stored master password.
    private func unlockWithBiometrics() async {
        let ok = await Biometrics.authenticate(reason: "Unlock your vault")
        guard ok else {
            model.errorMessage = "Biometric unlock was cancelled."
            return
        }
        guard let stored = KeychainVault.shared.masterPassword() else {
            model.errorMessage = "No stored password for Face ID. Use your master password once to enable it."
            return
        }
        await model.unlock(password: stored)
    }
}