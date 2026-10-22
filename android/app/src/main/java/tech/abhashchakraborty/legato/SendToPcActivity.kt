package tech.abhashchakraborty.legato

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * "Send to PC" in any app's share menu and on selected text: puts the text
 * on the PC's clipboard. Picked from the system share sheet it sends at once;
 * started any other way it asks first, so another app can't slip text onto
 * the PC's clipboard (a swapped address, a command) without you knowing. A
 * short toast says how it went.
 */
class SendToPcActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = when (intent?.action) {
            Intent.ACTION_PROCESS_TEXT -> intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
            Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)
            else -> null
        }?.trim().orEmpty()
        val legato = legato
        val pc = legato.state.value.pcName ?: "your PC"
        if (text.isEmpty() || !legato.state.value.paired) {
            Toast.makeText(this, if (text.isEmpty()) "Nothing to send" else "Pair with your PC in Legato first", Toast.LENGTH_SHORT).show()
            finish()
            return
        }
        val fromShareSheet = referrer?.host in setOf("android", "com.android.intentresolver")
        if (fromShareSheet) {
            send(text, pc)
            finish()
            return
        }
        val preview = if (text.length > 160) text.take(160) + "…" else text
        AlertDialog.Builder(this)
            .setTitle("Send to $pc?")
            .setMessage(preview)
            .setPositiveButton("Send") { _, _ -> send(text, pc) }
            .setNegativeButton("Cancel", null)
            .setOnDismissListener { finish() }
            .show()
    }

    private fun send(text: String, pc: String) {
        val legato = legato
        legato.scope.launch {
            // Not connected yet (the app was closed): connect, and give it a moment.
            if (!legato.state.value.online) {
                runCatching { legato.core.connect() }
                repeat(16) { if (!legato.core.state().online) delay(250) }
                legato.refresh()
            }
            val sent = runCatching { legato.core.sendClip(text) }.getOrDefault(false)
            withContext(Dispatchers.Main) {
                Toast.makeText(applicationContext, if (sent) "Sent to $pc" else "Couldn't reach $pc", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
