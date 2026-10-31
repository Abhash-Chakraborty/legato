package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tech.abhashchakraborty.legato.BuildConfig
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.Updates

/**
 * Offers a new version found by the check on opening Legato, once per
 * version: Update downloads it, then Install hands it to Android.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UpdateOffer(legato: Legato) {
    val context = LocalContext.current
    val state by Updates.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { Updates.maybeCheck(context) }
    var open by remember { mutableStateOf(false) }
    LaunchedEffect(state) {
        val s = state
        if (s is Updates.State.Available && Updates.shouldOffer(context, s.version)) {
            Updates.offered(context, s.version)
            open = true
        }
    }
    if (!open) return
    val version = when (val s = state) {
        is Updates.State.Available -> s.version
        is Updates.State.Downloading -> s.version
        is Updates.State.Ready -> s.version
        else -> null
    }
    ModalBottomSheet(
        onDismissRequest = { open = false },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.padding(horizontal = 24.dp).padding(bottom = 28.dp)) {
            Text("Legato ${version ?: ""} is out", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(8.dp))
            Text(
                "You have ${BuildConfig.VERSION_NAME}. The update keeps your pairing and notes; what's new shows once it's installed.",
                style = MaterialTheme.typography.bodyLarge,
            )
            Spacer(Modifier.height(20.dp))
            UpdateAction(state, onLater = { open = false })
        }
    }
}

/** The button for where an update is: Update, a progress bar, Install, or Try again. */
@Composable
private fun UpdateAction(state: Updates.State, onLater: (() -> Unit)? = null) {
    val context = LocalContext.current
    when (state) {
        is Updates.State.Downloading -> {
            Text("Downloading… ${(state.progress * 100).toInt()}%", style = MaterialTheme.typography.bodyMedium)
            Spacer(Modifier.height(8.dp))
            LinearProgressIndicator(progress = { state.progress }, modifier = Modifier.fillMaxWidth())
        }
        is Updates.State.Failed -> {
            Text(state.message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            Spacer(Modifier.height(12.dp))
            Button(onClick = { Updates.check(context) }, modifier = Modifier.fillMaxWidth()) { Text("Try again") }
        }
        else -> Row {
            onLater?.let { TextButton(onClick = it) { Text("Later") } }
            Spacer(Modifier.weight(1f))
            when (state) {
                is Updates.State.Ready -> Button(onClick = { Updates.install(context) }) { Text("Install") }
                is Updates.State.Available -> Button(onClick = { Updates.download(context) }) { Text("Update") }
                else -> {}
            }
        }
    }
}

/** Settings › About: the update's state, and whether Legato checks on its own. */
@Composable
fun UpdateRows(legato: Legato) {
    val context = LocalContext.current
    val state by Updates.state.collectAsStateWithLifecycle()
    var auto by remember { mutableStateOf(Updates.autoCheck(context)) }
    val checked = Updates.lastChecked(context).takeIf { it > 0 }?.let { ago(it / 1000) }
    val (detail, action) = when (val s = state) {
        is Updates.State.Checking -> "Checking…" to null
        is Updates.State.UpToDate -> "You have the latest version." to null
        is Updates.State.Available -> "${s.version} is out. Tap to download it." to { Updates.download(context) }
        is Updates.State.Downloading -> "Downloading ${s.version}… ${(s.progress * 100).toInt()}%" to null
        is Updates.State.Ready -> "${s.version} is downloaded. Tap to install it." to { Updates.install(context) }
        is Updates.State.Failed -> s.message to { Updates.check(context) }
        is Updates.State.Idle -> (checked?.let { "Checked $it. Tap to check now." } ?: "Tap to check now.") to { Updates.check(context) }
    }
    SettingRow("Updates", detail, onClick = action ?: { Updates.check(context) })
    Hairline()
    SettingRow(
        "Check for updates automatically",
        "When Legato opens, at most every six hours, from GitHub Releases.",
        trailing = {
            Switch(checked = auto, onCheckedChange = {
                auto = it
                legato.prefs.edit().putBoolean(Updates.PREF_AUTO, it).apply()
            })
        },
    )
}
