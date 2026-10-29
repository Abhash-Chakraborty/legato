package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import tech.abhashchakraborty.legato.BuildConfig
import tech.abhashchakraborty.legato.Legato

/**
 * What changed in each version, newest first. Add an entry with every
 * release; the sheet shows the one for the version just installed, once.
 */
val RELEASE_NOTES: Map<String, List<String>> = linkedMapOf(
    "0.2.0" to listOf(
        "What you copy on this phone goes to your PC by itself, not only when you open Legato.",
        "Tap ✕ by mistake and the dictation is kept: Home writes it up, or shows it ready to write up.",
        "The checkmark answers at once: a spinner shows while your words are written up.",
        "Choose whether a dictation keeps listening when the keyboard closes (Settings › Bubble over Gboard).",
        "Your PC is found again within half a minute while you're using the phone.",
    ),
)

/** Shows this version's notes once after an update; a fresh install just records the version. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WhatsNew(legato: Legato) {
    val version = BuildConfig.VERSION_NAME
    var open by remember {
        val seen = legato.prefs.getString(PREF_SEEN, null)
        val setUp = legato.prefs.getBoolean("setup_done", false)
        if (seen == null && !setUp) legato.prefs.edit().putString(PREF_SEEN, version).apply()
        mutableStateOf(seen != version && setUp && RELEASE_NOTES.containsKey(version))
    }
    if (!open) return
    val close = {
        legato.prefs.edit().putString(PREF_SEEN, version).apply()
        open = false
    }
    ModalBottomSheet(
        onDismissRequest = close,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.padding(horizontal = 24.dp).padding(bottom = 28.dp)) {
            Text("What's new in $version", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(16.dp))
            RELEASE_NOTES[version].orEmpty().forEach { line ->
                Row(Modifier.padding(vertical = 6.dp)) {
                    Text("•", color = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(10.dp))
                    Text(line, style = MaterialTheme.typography.bodyLarge)
                }
            }
            Spacer(Modifier.height(20.dp))
            Button(onClick = close, modifier = Modifier.fillMaxWidth()) { Text("Got it") }
        }
    }
}

private const val PREF_SEEN = "whats_new_seen"
