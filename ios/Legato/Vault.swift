import CryptoKit
import Foundation
import Security

/// Seals this iPhone's private key with AES-256-GCM under a key kept in the
/// Keychain, for this device only: it isn't in a backup and doesn't move to
/// a new phone, so a copy of the app's files can't pose as this iPhone.
/// Legato for Android does the same with its Keystore.
struct Vault {
    private static let service = "tech.abhashchakraborty.legato.vault"
    private static let account = "identity"

    /// A key to use in place of the Keychain's, for tests.
    private let fixedKey: SymmetricKey?

    init(fixedKey: SymmetricKey? = nil) {
        self.fixedKey = fixedKey
    }

    /// `plain` sealed, or nil when the Keychain won't give or keep a key.
    func seal(_ plain: Data) -> Data? {
        guard let key = key(makeOne: true) else { return nil }
        return try? AES.GCM.seal(plain, using: key).combined
    }

    /// What `seal` sealed, or nil when it can't be opened: the key is gone
    /// (the phone was reset) or the bytes aren't ours.
    func open(_ sealed: Data) -> Data? {
        guard let key = key(makeOne: false),
              let box = try? AES.GCM.SealedBox(combined: sealed)
        else { return nil }
        return try? AES.GCM.open(box, using: key)
    }

    private func key(makeOne: Bool) -> SymmetricKey? {
        if let fixedKey { return fixedKey }
        let find: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Vault.service,
            kSecAttrAccount as String: Vault.account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var found: CFTypeRef?
        let status = SecItemCopyMatching(find as CFDictionary, &found)
        if status == errSecSuccess, let data = found as? Data, data.count == 32 {
            return SymmetricKey(data: data)
        }
        guard makeOne, status == errSecItemNotFound else { return nil }
        let key = SymmetricKey(size: .bits256)
        let add: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Vault.service,
            kSecAttrAccount as String: Vault.account,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData as String: key.withUnsafeBytes { Data($0) },
        ]
        return SecItemAdd(add as CFDictionary, nil) == errSecSuccess ? key : nil
    }
}
