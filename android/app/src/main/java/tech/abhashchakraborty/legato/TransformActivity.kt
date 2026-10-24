package tech.abhashchakraborty.legato

import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.bubble.BubbleAccessibilityService

/**
 * "Transform" on selected text in any app: pick one of your PC's transforms
 * and the selection is replaced with the result (or, in a field that can't
 * be changed, the result goes on the clipboard). Runs on the PC while
 * connected, else here with the AI key the PC shared.
 */
class TransformActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = intent?.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString().orEmpty()
        val readOnly = intent?.getBooleanExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false) ?: true
        val app = referrer?.host
        val legato = legato
        if (text.isBlank()) return done("Nothing selected")
        if (!legato.state.value.paired) return done("Pair with your PC in Legato first")
        legato.scope.launch {
            // Opened from another app with Legato closed: reach the PC first.
            if (!legato.state.value.online) {
                runCatching { legato.core.connect() }
                repeat(12) { if (!legato.core.state().online) delay(250) }
                legato.refresh()
            }
            if (legato.core.pcTransforms()) legato.core.pullAi()
            val transforms = runCatching { legato.core.transforms() }.getOrDefault(emptyList())
            withContext(Dispatchers.Main) {
                if (transforms.isEmpty()) return@withContext done("No transforms yet. Switch some on in Legato on your PC.")
                AlertDialog.Builder(this@TransformActivity)
                    .setTitle("Transform")
                    .setItems(transforms.map { it.name }.toTypedArray()) { _, which -> run(transforms[which].id, transforms[which].name, text, app, readOnly) }
                    .setNegativeButton("Cancel", null)
                    .setOnCancelListener { finish() }
                    .setOnDismissListener { if (!working) finish() }
                    .show()
            }
        }
    }

    private var working = false

    private fun run(id: String, name: String, text: String, app: String?, readOnly: Boolean) {
        working = true
        val wait = AlertDialog.Builder(this).setMessage("$name…").setCancelable(false).show()
        val legato = legato
        val started = System.currentTimeMillis()
        legato.scope.launch {
            val result = runCatching { legato.transform(id, text, app) }
            val took = "%.1f".format((System.currentTimeMillis() - started) / 1000f)
            legato.onLine(
                result.fold(
                    { out -> if (out == null || out == text) "$name on selected text: nothing to change ($took s)" else "$name on selected text: done in $took s" },
                    { e -> "$name on selected text didn't run: ${e.message}" },
                ),
            )
            withContext(Dispatchers.Main) {
                wait.dismiss()
                result.fold(
                    onSuccess = { out ->
                        when {
                            out == null || out == text -> done("$name left it as it was")
                            readOnly -> {
                                getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(name, out))
                                done("Copied: this text can't be changed here")
                            }
                            else -> {
                                setResult(RESULT_OK, Intent().putExtra(Intent.EXTRA_PROCESS_TEXT, out))
                                finish()
                                // Chrome drops the result once the selection is gone, and the
                                // picker over it clears the selection; then Legato puts it in.
                                Handler(Looper.getMainLooper()).postDelayed({
                                    if (BubbleAccessibilityService.instance?.replaceText(text, out) == true) {
                                        legato.onLine("$name: put the result in the field")
                                    }
                                }, 700)
                            }
                        }
                    },
                    onFailure = { e -> done(e.message ?: "The transform didn't run") },
                )
            }
        }
    }

    private fun done(message: String) {
        Toast.makeText(applicationContext, message, Toast.LENGTH_LONG).show()
        finish()
    }
}
