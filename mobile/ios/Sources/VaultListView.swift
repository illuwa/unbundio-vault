import SwiftUI

/// The list screen: search, tap to reveal, one-tap copy, generator.
struct VaultListView: View {
    @EnvironmentObject var model: VaultModel
    @State private var revealed: VaultCore.Entry?
    @State private var generated = ""
    @State private var showingGenerator = false

    var body: some View {
        NavigationStack {
            List {
                if let audit = model.audit {
                    Section {
                        HStack {
                            Label("\(audit.total)", systemImage: "key.fill")
                            Spacer()
                            if !audit.weak.isEmpty {
                                Label("\(audit.weak.count) weak", systemImage: "exclamationmark.triangle.fill")
                                    .foregroundStyle(.orange)
                            }
                            if audit.reusedGroups > 0 {
                                Label("\(audit.reusedGroups) reused", systemImage: "arrow.triangle.2.circlepath")
                                    .foregroundStyle(.orange)
                            }
                            if audit.weak.isEmpty && audit.reusedGroups == 0 {
                                Text("clean").foregroundStyle(.green)
                            }
                        }
                        .font(.caption)
                    }
                }

                Section {
                    ForEach(model.filtered) { entry in
                        Button {
                            revealed = model.reveal(id: entry.id)
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text((entry.favorite ? "★ " : "") + entry.title)
                                        .font(.body.weight(.medium))
                                        .foregroundStyle(.primary)
                                    Text(entry.username)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                if entry.url != nil {
                                    Image(systemName: "globe").font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            .contentShape(Rectangle())
                        }
                        // Rows are tappable but should read as content, not as
                        // accent-coloured buttons.
                        .buttonStyle(.plain)
                    }
                } header: {
                    Text("\(model.filtered.count) of \(model.entries.count)")
                }

                if model.isEmpty {
                    Section {
                        Text("Vault is empty. Add entries with the desktop CLI, or import a CSV.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            // Pin the field to the navigation bar: without an explicit placement iOS 26
// drops it to the bottom of the list, away from the content it filters.
.searchable(
                text: $model.search,
                placement: .navigationBarDrawer(displayMode: .always),
                prompt: "Search logins"
            )
            .navigationTitle("unbundio-vault")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button { model.lock() } label: {
                        Image(systemName: "lock.fill")
                    }
                    .accessibilityLabel("Lock vault")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showingGenerator.toggle() } label: {
                        Image(systemName: "wand.and.stars")
                    }
                    .accessibilityLabel("Password generator")
                }
            }
            .sheet(item: $revealed) { entry in
                EntryDetailView(entry: entry)
            }
            .sheet(isPresented: $showingGenerator) {
                GeneratorView(
                    password: Binding(
                        get: { generated },
                        set: { generated = $0 }
                    ),
                    onGenerate: { model.generatePassword() }
                )
            }
            .overlay(alignment: .bottom) {
                if let copied = model.copiedTitle {
                    Text("Copied \(copied) — clears in 30s")
                        .font(.caption)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(.thinMaterial, in: Capsule())
                        .padding(.bottom, 12)
                        .transition(.opacity)
                        .task {
                            try? await Task.sleep(for: .seconds(2))
                            withAnimation { model.copiedTitle = nil }
                        }
                }
            }
        }
    }
}

struct EntryDetailView: View {
    let entry: VaultCore.Entry
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject var model: VaultModel
    @State private var showPassword = false

    var body: some View {
        NavigationStack {
            List {
                Section("Account") {
                    LabeledContent("User", value: entry.username)
                    LabeledContent("Password") {
                        HStack {
                            Text(showPassword ? entry.password : String(repeating: "•", count: min(entry.password.count, 20)))
                                .font(.system(.body, design: .monospaced))
                            Spacer()
                            Button {
                                showPassword.toggle()
                            } label: {
                                Image(systemName: showPassword ? "eye.slash" : "eye")
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    Button {
                        model.copy(entry.password, title: entry.title)
                    } label: {
                        Label("Copy password", systemImage: "doc.on.doc")
                    }
                }
                if let url = entry.url, let link = URL(string: url) {
                    Section("Site") {
                        Link(url, destination: link)
                    }
                }
                if let notes = entry.notes {
                    Section("Notes") { Text(notes) }
                }
            }
            .navigationTitle(entry.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }
}

struct GeneratorView: View {
    @Binding var password: String
    let onGenerate: () -> String
    @Environment(\.dismiss) private var dismiss
    @State private var length = 20

    var body: some View {
        NavigationStack {
            VStack(spacing: 18) {
                Text(password)
                    .font(.system(.title3, design: .monospaced))
                    .padding()
                    .frame(maxWidth: .infinity)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 10))
                    .textSelection(.enabled)

                Stepper("Length: \(length)", value: $length, in: 8...64)

                Button("Copy") {
                    UIPasteboard.general.string = password
                }
                .buttonStyle(.borderedProminent)

                Spacer()
            }
            .padding()
            .navigationTitle("Generator")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button("Regenerate") { password = onGenerate() }
                }
            }
            .onAppear { password = onGenerate() }
            .onChange(of: length) { _, _ in password = onGenerate() }
        }
    }
}