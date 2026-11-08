package dev.mwt.biometric

import android.app.Activity
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import java.security.UnrecoverableKeyException
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class KeyArgs {
  lateinit var alias: String
}

@InvokeArg
class SealArgs {
  lateinit var alias: String
  lateinit var hex: String
}

private const val IV_LEN = 12
private const val TAG_BITS = 128

/**
 * Quick unlock key. AES-256-GCM in the AndroidKeyStore, usable only after a
 * BiometricPrompt (BIOMETRIC_STRONG) that is bound to the key through a
 * CryptoObject, and dropped by the system when a new biometric is enrolled.
 * Every answer resolves with `status` (ok | cancelled | invalidated |
 * unavailable | failed); the invoke is never rejected for an expected outcome.
 */
@TauriPlugin
class BiometricPlugin(private val activity: Activity) : Plugin(activity) {
  private val store: KeyStore
    get() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

  private fun answer(invoke: Invoke, status: String, bytes: ByteArray? = null) {
    val out = JSObject().put("status", status)
    if (bytes != null) out.put("hex", hex(bytes))
    invoke.resolve(out)
  }

  @Command
  fun available(invoke: Invoke) {
    val ok = BiometricManager.from(activity).canAuthenticate(BIOMETRIC_STRONG) ==
      BiometricManager.BIOMETRIC_SUCCESS
    invoke.resolve(JSObject().put("available", ok))
  }

  @Command
  fun deleteKey(invoke: Invoke) {
    val args = invoke.parseArgs(KeyArgs::class.java)
    try {
      store.deleteEntry(args.alias)
    } catch (ex: Exception) {
      Logger.error("delete key: ${ex.message}")
    }
    invoke.resolve(JSObject())
  }

  private fun generateKey(alias: String): SecretKey {
    val spec = { strongBox: Boolean ->
      KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .setUserAuthenticationRequired(true)
        .setInvalidatedByBiometricEnrollment(true)
        .apply {
          if (Build.VERSION.SDK_INT >= 30) {
            // 0 = authentication for every use of the key, biometric only.
            setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
          } else {
            @Suppress("DEPRECATION")
            setUserAuthenticationValidityDurationSeconds(-1)
          }
          if (strongBox && Build.VERSION.SDK_INT >= 28) setIsStrongBoxBacked(true)
        }
        .build()
    }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
    return try {
      generator.init(spec(true))
      generator.generateKey()
    } catch (ex: StrongBoxUnavailableException) {
      Logger.info("no StrongBox, using the TEE: ${ex.message}")
      generator.init(spec(false))
      generator.generateKey()
    }
  }

  private fun aad(alias: String): ByteArray = alias.toByteArray(Charsets.UTF_8)

  @Command
  fun encrypt(invoke: Invoke) {
    val args = invoke.parseArgs(SealArgs::class.java)
    if (!isAvailable()) return answer(invoke, "unavailable")
    val cipher: Cipher
    try {
      store.deleteEntry(args.alias)
      val key = generateKey(args.alias)
      cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.ENCRYPT_MODE, key)
    } catch (ex: Exception) {
      Logger.error("encrypt init: ${ex.message}")
      store.deleteEntry(args.alias)
      return answer(invoke, "failed")
    }
    val plain = unhex(args.hex)
    prompt(invoke, args.alias, cipher) { done ->
      done.updateAAD(aad(args.alias))
      done.iv + done.doFinal(plain)
    }
  }

  @Command
  fun decrypt(invoke: Invoke) {
    val args = invoke.parseArgs(SealArgs::class.java)
    if (!isAvailable()) return answer(invoke, "unavailable")
    val sealed = unhex(args.hex)
    if (sealed.size <= IV_LEN) {
      store.deleteEntry(args.alias)
      return answer(invoke, "invalidated")
    }
    val cipher: Cipher
    try {
      val key = store.getKey(args.alias, null) as? SecretKey
      if (key == null) return answer(invoke, "invalidated")
      cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, sealed.copyOfRange(0, IV_LEN)))
    } catch (ex: KeyPermanentlyInvalidatedException) {
      // A fingerprint was added or the biometrics were reset: the key is dead.
      store.deleteEntry(args.alias)
      return answer(invoke, "invalidated")
    } catch (ex: UnrecoverableKeyException) {
      store.deleteEntry(args.alias)
      return answer(invoke, "invalidated")
    } catch (ex: Exception) {
      Logger.error("decrypt init: ${ex.message}")
      return answer(invoke, "failed")
    }
    val body = sealed.copyOfRange(IV_LEN, sealed.size)
    prompt(invoke, args.alias, cipher) { done ->
      done.updateAAD(aad(args.alias))
      done.doFinal(body)
    }
  }

  private fun isAvailable(): Boolean =
    BiometricManager.from(activity).canAuthenticate(BIOMETRIC_STRONG) == BiometricManager.BIOMETRIC_SUCCESS

  /** The prompt is the only way to use `cipher`: it is wrapped in a CryptoObject. */
  private fun prompt(invoke: Invoke, alias: String, cipher: Cipher, finish: (Cipher) -> ByteArray) {
    val host = activity as? FragmentActivity ?: return answer(invoke, "failed")
    host.runOnUiThread {
      val callback = object : BiometricPrompt.AuthenticationCallback() {
        override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
          val bound = result.cryptoObject?.cipher
          if (bound == null) return answer(invoke, "failed")
          try {
            answer(invoke, "ok", finish(bound))
          } catch (ex: AEADBadTagException) {
            // Not the ciphertext this key made: the binding is stale.
            store.deleteEntry(alias)
            answer(invoke, "invalidated")
          } catch (ex: Exception) {
            Logger.error("cipher: ${ex.message}")
            answer(invoke, "failed")
          }
        }

        override fun onAuthenticationError(code: Int, message: CharSequence) {
          when (code) {
            BiometricPrompt.ERROR_USER_CANCELED,
            BiometricPrompt.ERROR_NEGATIVE_BUTTON,
            BiometricPrompt.ERROR_CANCELED -> answer(invoke, "cancelled")
            BiometricPrompt.ERROR_NO_BIOMETRICS,
            BiometricPrompt.ERROR_HW_NOT_PRESENT,
            BiometricPrompt.ERROR_HW_UNAVAILABLE -> answer(invoke, "unavailable")
            else -> {
              Logger.error("biometric error $code: $message")
              answer(invoke, "failed")
            }
          }
        }
        // onAuthenticationFailed (a finger not recognised): the prompt stays and lets the user retry.
      }
      val info = BiometricPrompt.PromptInfo.Builder()
        .setTitle(host.getString(R.string.mwt_bio_title))
        .setSubtitle(host.getString(R.string.mwt_bio_subtitle))
        .setNegativeButtonText(host.getString(R.string.mwt_bio_negative))
        .setAllowedAuthenticators(BIOMETRIC_STRONG)
        .setConfirmationRequired(false)
        .build()
      try {
        BiometricPrompt(host, ContextCompat.getMainExecutor(host), callback)
          .authenticate(info, BiometricPrompt.CryptoObject(cipher))
      } catch (ex: Exception) {
        Logger.error("prompt: ${ex.message}")
        answer(invoke, "failed")
      }
    }
  }

  private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

  private fun unhex(text: String): ByteArray =
    ByteArray(text.length / 2) { index -> text.substring(index * 2, index * 2 + 2).toInt(16).toByte() }
}
