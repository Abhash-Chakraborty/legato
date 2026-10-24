package tech.abhashchakraborty.legato

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Secrets sealed with a key that lives in the Android Keystore (in the
 * phone's secure hardware when it has StrongBox): AES-256-GCM, and the key
 * can't be read out of the Keystore, so a copy of the app's files is useless
 * on its own. Nothing here is ever shown, logged or backed up; the app reads
 * a secret back only to use it.
 */
class Vault(context: Context) {
    private val prefs = context.getSharedPreferences("vault", Context.MODE_PRIVATE)

    fun put(name: String, value: String) {
        prefs.edit().putString(name, Base64.encodeToString(seal(value.toByteArray()), Base64.NO_WRAP)).apply()
    }

    /** The secret, or null when there's none or it can't be opened (the Keystore key was reset). */
    fun get(name: String): String? {
        val sealed = prefs.getString(name, null) ?: return null
        return runCatching { String(open(Base64.decode(sealed, Base64.NO_WRAP))) }.getOrNull()
    }

    fun has(name: String): Boolean = prefs.contains(name)

    fun remove(name: String) {
        prefs.edit().remove(name).apply()
    }

    /** Keeps exactly `values` under names starting with `prefix`, dropping the rest. */
    fun replace(prefix: String, values: Map<String, String>) {
        val edit = prefs.edit()
        prefs.all.keys.filter { it.startsWith(prefix) && it !in values }.forEach { edit.remove(it) }
        values.forEach { (name, value) -> edit.putString(name, Base64.encodeToString(seal(value.toByteArray()), Base64.NO_WRAP)) }
        edit.apply()
    }

    /** `plain` sealed with the Keystore key, or null when the Keystore can't. */
    fun sealBytes(plain: ByteArray): ByteArray? = runCatching { seal(plain) }.getOrNull()

    /** What `sealBytes` sealed, or null when it can't be opened. */
    fun openBytes(sealed: ByteArray): ByteArray? = runCatching { open(sealed) }.getOrNull()

    private fun seal(plain: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        return cipher.iv + cipher.doFinal(plain)
    }

    private fun open(sealed: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV_BYTES))
        return cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES)
    }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        fun make(strongBox: Boolean): SecretKey {
            val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .setRandomizedEncryptionRequired(true)
                .apply { if (strongBox) setIsStrongBoxBacked(true) }
                .build()
            return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply { init(spec) }.generateKey()
        }
        return try {
            make(strongBox = true)
        } catch (e: StrongBoxUnavailableException) {
            make(strongBox = false)
        }
    }

    companion object {
        private const val ALIAS = "legato-vault"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val IV_BYTES = 12
    }
}
