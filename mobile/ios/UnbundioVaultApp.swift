import SwiftUI

@main
struct UnbundioVaultApp: App {
    @StateObject private var model = VaultModel()

    var body: some Scene {
        WindowGroup {
            Group {
                if model.isUnlocked {
                    VaultListView()
                } else {
                    UnlockView(model: model)
                }
            }
            .environmentObject(model)
        }
    }
}