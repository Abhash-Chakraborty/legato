package tech.abhashchakraborty.legato.ui

import android.Manifest
import android.content.ClipboardManager
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.bubble.BubbleAccessibilityService
import tech.abhashchakraborty.legato.core.coreVersion

/** Your PC, the speech model, the bubble's Android switches, and the rest. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onPair: () -> Unit, onDebug: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val state by legato.state.collectAsStateWithLifecycle()
    var confirmUnpair by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text("Settings") }, scrollBehavior = scrollBehavior) },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Group("Your PC") {
                if (state.paired) {
                    ListItem(
                        headlineContent = { Text(state.pcName ?: "PC") },
                        supportingContent = {
                            Text(
                                if (state.online) "Connected" + (state.lastSync?.let { " · synced ${ago(it)}" } ?: "")
                                else "Not reachable · changes wait on this phone" + (state.lastSync?.let { " · last synced ${ago(it)}" } ?: ""),
                            )
                        },
                        leadingContent = {
                            Box(Modifier.size(10.dp).background(if (state.online) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline, CircleShape))
                        },
                        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    )
                    Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilledTonalButton(onClick = {
                            legato.run { if (!syncNow()) legato.notice("Not connected to ${state.pcName ?: "the PC"} right now.") }
                        }) { Text("Sync now") }
                        if (!state.online) OutlinedButton(onClick = { legato.run { connect() } }) { Text("Reconnect") }
                        TextButton(onClick = { confirmUnpair = true }) { Text("Unpair") }
                    }
                } else {
                    ListItem(
                        headlineContent = { Text("Pair with your PC") },
                        supportingContent = { Text("Sync your Dictionary, snippets, notes and history with Legato on Windows.") },
                        leadingContent = { Icon(Icons.Default.Computer, contentDescription = null) },
                        trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null) },
                        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                        modifier = Modifier.clickable(onClick = onPair),
                    )
                }
            }
            Group("Speech model") { ModelRow(legato) }
            Group("Bubble over Gboard") { BubbleRows() }
            Group("Clipboard") {
                ListItem(
                    headlineContent = { Text("Send the clipboard to your PC") },
                    supportingContent = { Text(if (state.paired) "What you copied here lands on the PC's clipboard." else "Pair with your PC first.") },
                    trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null) },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    modifier = Modifier.clickable(enabled = state.paired) {
                        val cm = context.getSystemService(ClipboardManager::class.java)
                        val text = cm.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString().orEmpty()
                        if (text.isBlank()) scope.launch { snackbar.showSnackbar("Nothing on the clipboard") }
                        else legato.run {
                            legato.notice(if (sendClip(text)) "Sent to ${state.pcName ?: "your PC"}" else "Not connected; the clip wasn't sent.")
                        }
                    },
                )
            }
            Group("About") {
                ListItem(
                    headlineContent = { Text("Legato for Android") },
                    supportingContent = { Text("Core ${coreVersion()}") },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                )
                ListItem(
                    headlineContent = { Text("Advanced") },
                    supportingContent = { Text("The connection log and test tools.") },
                    trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null) },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    modifier = Modifier.clickable(onClick = onDebug),
                )
            }
        }
    }

    if (confirmUnpair) {
        AlertDialog(
            onDismissRequest = { confirmUnpair = false },
            title = { Text("Unpair from ${state.pcName ?: "your PC"}?") },
            text = { Text("What already synced stays on this phone; nothing new syncs until you pair again.") },
            confirmButton = { TextButton(onClick = { confirmUnpair = false; legato.run { unpair() } }) { Text("Unpair") } },
            dismissButton = { TextButton(onClick = { confirmUnpair = false }) { Text("Cancel") } },
        )
    }
}

/** The speech model: download it, watch it come in, see that it's loaded. */
@Composable
fun ModelRow(legato: Legato) {
    val model by legato.model.collectAsStateWithLifecycle()
    Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text("Moonshine Base", style = MaterialTheme.typography.bodyLarge)
        Text(
            when (val m = model) {
                ModelStatus.Missing -> "About 60 MB. It runs on this phone, so nothing you say leaves it."
                is ModelStatus.Downloading -> if (m.percent >= 0) "Downloading · ${m.percent}%" else "Downloading…"
                ModelStatus.Downloaded -> "Downloaded · loading"
                is ModelStatus.Loaded -> "Ready on this phone"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        when (val m = model) {
            ModelStatus.Missing -> {
                Spacer(Modifier.height(10.dp))
                Button(onClick = { legato.fetchModel() }) { Text("Download") }
            }
            is ModelStatus.Downloading -> {
                Spacer(Modifier.height(10.dp))
                if (m.percent >= 0) LinearProgressIndicator(progress = { m.percent / 100f }, modifier = Modifier.fillMaxWidth())
                else LinearProgressIndicator(Modifier.fillMaxWidth())
            }
            else -> Unit
        }
    }
}

/** The Android switches the bubble needs, each with a way to turn it on. */
@Composable
fun BubbleRows() {
    val context = LocalContext.current
    val serviceOn by BubbleAccessibilityService.running.collectAsStateWithLifecycle()
    var overlayOn by remember { mutableStateOf(Settings.canDrawOverlays(context)) }
    var micOn by remember { mutableStateOf(micGranted(context)) }
    var notifyOn by remember { mutableStateOf(notificationsGranted(context)) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        overlayOn = Settings.canDrawOverlays(context)
        micOn = micGranted(context)
        notifyOn = notificationsGranted(context)
    }
    val mic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { micOn = it }
    val notify = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { notifyOn = it }

    PermissionRow("Microphone", "Legato hears you only while you dictate.", micOn, "Allow") {
        mic.launch(Manifest.permission.RECORD_AUDIO)
    }
    PermissionRow("Legato under Accessibility", "Lets the bubble see the keyboard open and type into the field.", serviceOn) {
        openAccessibilitySettings(context)
    }
    PermissionRow("Display over other apps", "So the bubble can sit over the app you're typing in.", overlayOn) {
        openOverlaySettings(context)
    }
    if (android.os.Build.VERSION.SDK_INT >= 33) {
        PermissionRow("Notifications", "Android shows a small notice while Legato holds the microphone.", notifyOn, "Allow") {
            notify.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}
