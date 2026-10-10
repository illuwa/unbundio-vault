import Foundation

/// Swift side of the Rust C ABI (see `src/ffi.rs` and `UnbundioVault.h`).
///
/// Every Rust-owned string is copied into Swift and released immediately, so no
/// pointer outlives this file's scope.
enum VaultCore {

    // MARK: - Errors

    enum CoreError: LocalizedError {
        case openFailed(String)
        case notFound(String)
        case badJSON(String)
        case createFailed(String)

        var errorDescription: String? {
            switch self {
            case .openFailed(let m), .notFound(let m),
                 .badJSON(let m), .createFailed(let m):
                return m
            }
        }
    }

    private static func lastError() -> String {
        guard let c = uv_last_error() else { return "unknown error" }
        return String(cString: c)
    }

    /// Take ownership of a Rust string, copy it, and free it.
    /// Internal (not private) because VaultSession lives in this file too.
    static func take(_ ptr: UnsafeMutablePointer<CChar>?) throws -> String {
        guard let ptr else { throw CoreError.badJSON(lastError()) }
        defer { uv_string_free(ptr) }
        return String(cString: ptr)
    }

    // MARK: - Lifecycle

    /// Decrypt a vault and keep it in memory.
    /// - Note: this is the expensive call (Argon2id); do it once per unlock.
    static func open(path: String, password: String) throws -> VaultSession {
        guard let handle = uv_open(path, password) else {
            throw CoreError.openFailed(lastError())
        }
        return VaultSession(handle: handle)
    }

    /// Create a new vault file.
    static func create(path: String, password: String) throws {
        guard uv_create(path, password) == 0 else {
            throw CoreError.createFailed(lastError())
        }
    }

    /// A password without needing a vault.
    static func generate(length: Int = 20) -> String {
        guard let ptr = uv_generate(Int32(length)) else { return "" }
        return (try? take(ptr)) ?? ""
    }

    // MARK: - Models

    struct EntrySummary: Identifiable, Hashable {
        let id: String
        let title: String
        let username: String
        let url: String?
        let favorite: Bool
    }

    struct Entry: Identifiable, Hashable {
        let id: String
        let title: String
        let username: String
        let password: String
        let url: String?
        let notes: String?
    }

    struct Audit: Hashable {
        let total: Int
        let weak: [String]
        let reusedGroups: Int
        let oldCount: Int
    }
}

/// Owns the decrypted vault. `deinit` closes it, so the memory cannot leak
/// even if a caller forgets.
final class VaultSession {
    private let handle: UnsafeMutableRawPointer?

    init(handle: UnsafeMutableRawPointer?) {
        self.handle = handle
    }

    deinit {
        if let handle { uv_close(handle) }
    }

    private func json(_ fn: (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?) throws -> Data {
        let text = try VaultCore.take(fn(handle))
        return Data(text.utf8)
    }

    private struct ListRow: Decodable {
        let id: String
        let title: String
        let username: String
        let url: String?
        let favorite: Bool
    }

    private struct GetRow: Decodable {
        let id: String
        let title: String
        let username: String
        let password: String
        let url: String?
        let notes: String?
    }

    private struct AuditRow: Decodable {
        let total: Int
        let weak: [String]
        let reused: [[String]]
        let old: [String]
    }

    private struct StatusRow: Decodable {
        let entries: Int
    }

    /// Entries without passwords — safe to render as a list.
    func list() throws -> [VaultCore.EntrySummary] {
        let data = try json { uv_list_json($0) }
        return try JSONDecoder().decode([ListRow].self, from: data).map {
            VaultCore.EntrySummary(
                id: $0.id, title: $0.title,
                username: $0.username, url: $0.url, favorite: $0.favorite
            )
        }
    }

    /// One entry with its password.
    func entry(id: String) throws -> VaultCore.Entry {
        let data = try json { handle in
            guard let handle else { return nil }
            return uv_get_json(handle, id)
        }
        let row = try JSONDecoder().decode(GetRow.self, from: data)
        return VaultCore.Entry(
            id: row.id, title: row.title, username: row.username,
            password: row.password, url: row.url, notes: row.notes
        )
    }

    func audit() throws -> VaultCore.Audit {
        let data = try json { uv_audit_json($0) }
        let row = try JSONDecoder().decode(AuditRow.self, from: data)
        return VaultCore.Audit(
            total: row.total, weak: row.weak,
            reusedGroups: row.reused.count, oldCount: row.old.count
        )
    }

    func count() throws -> Int {
        let data = try json { uv_status_json($0) }
        return try JSONDecoder().decode(StatusRow.self, from: data).entries
    }
}