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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material.icons.outlined.LinkOff
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.BluetoothLink
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.PhoneModels
import tech.abhashchakraborty.legato.bubble.BubbleAccessibilityService
import tech.abhashchakraborty.legato.core.coreVersion

/** Your PC, dictation, the bubble's Android switches, the clipboard, the connection. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onPair: () -> Unit, onDebug: () -> Unit, onGuide: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val state by legato.state.collectAsStateWithLifecycle()
    var confirmUnpair by remember { mutableStateOf(false) }
    var pcMenu by remember { mutableStateOf(false) }
    var clipMode by remember { mutableStateOf(false) }
    var clipsOpen by remember { mutableStateOf(false) }
    var auto by remember { mutableStateOf(legato.autoClipboard) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    val pc = state.pcName ?: "your PC"

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text("Settings") }, scrollBehavior = scrollBehavior) },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 32.dp),
        ) {
            DayHeader("Your PC")
            if (state.paired) {
                PcCard(
                    legato = legato,
                    onSync = { legato.run { if (!syncNow()) legato.notice("Not connected to $pc right now.") } },
                    onMore = { pcMenu = true },
                )
            } else {
                Group {
                    SettingRow(
                        "Pair with your PC",
                        "Scan the code in Legato on Windows › Settings › Devices.",
                        onClick = onPair,
                        trailing = { Chevron() },
                    )
                }
            }

            Group("Dictation") {
                ModelRow(legato)
                Hairline()
                PcModelRow(legato, state.paired, state.pcName)
            }

            Group("Transforms") { TransformRows(legato) }

            Group("Bubble over Gboard") {
                BubbleRows()
                Hairline()
                KeyboardCloseRow(legato)
            }

            Group("Clipboard") {
                SettingRow(
                    "Shared clipboard",
                    when {
                        !state.paired -> "Pair with your PC first."
                        auto -> "Automatic: copies on $pc land here at once."
                        else -> "Manual: nothing moves until you send it or pick a clip."
                    },
                    enabled = state.paired,
                    onClick = { clipMode = true },
                    trailing = { Text(if (auto) "Automatic" else "Manual", style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint) },
                )
                Hairline()
                SettingRow(
                    "Send the clipboard now",
                    if (state.paired) "Also in any app's share menu and on selected text: Send to PC." else "Pair with your PC first.",
                    enabled = state.paired,
                    onClick = {
                        val cm = context.getSystemService(ClipboardManager::class.java)
                        val text = cm.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString().orEmpty()
                        if (text.isBlank()) scope.launch { snackbar.showSnackbar("Nothing on the clipboard") }
                        else scope.launch {
                            val sent = withContext(Dispatchers.Default) { runCatching { legato.core.sendClip(text.trim()) }.getOrDefault(false) }
                            snackbar.showSnackbar(if (sent) "Sent to $pc" else "Not connected; the clip wasn't sent.")
                        }
                    },
                )
                Hairline()
                SettingRow(
                    "Recent clips",
                    if (state.paired) "What $pc copied since Legato opened. Tap one to copy it." else "Pair with your PC first.",
                    enabled = state.paired,
                    onClick = { clipsOpen = true },
                    trailing = { Chevron() },
                )
            }

            Group("Connection") {
                BluetoothRow(onAllowed = { legato.run { connect() } })
            }

            Group("About") {
                SettingRow("Legato for Android", "${tech.abhashchakraborty.legato.BuildConfig.VERSION_NAME} · core ${coreVersion()}")
                Hairline()
                SettingRow("Setup guide", "The first-run steps again.", onClick = onGuide, trailing = { Chevron() })
                Hairline()
                SettingRow("Advanced", "Connection details and the log.", onClick = onDebug, trailing = { Chevron() })
            }
        }
    }

    if (pcMenu) {
        MenuSheet(
            title = pc,
            subtitle = pcStatus(state, legato.device, long = true).text,
            actions = listOfNotNull(
                MenuAction("Reconnect", Icons.Default.Refresh) { legato.run { connect() } },
                MenuAction("Connection details", Icons.Outlined.Tune) { onDebug() },
                MenuAction("Unpair", Icons.Outlined.LinkOff, danger = true) { confirmUnpair = true },
            ),
            onDismiss = { pcMenu = false },
        )
    }

    if (clipMode) {
        ClipModeSheet(
            pc = pc,
            auto = auto,
            onPick = { auto = it; legato.autoClipboard = it },
            onDismiss = { clipMode = false },
        )
    }

    if (clipsOpen) {
        RecentClipsSheet(legato, pc, onCopied = { scope.launch { snackbar.showSnackbar("Copied") } }, onDismiss = { clipsOpen = false })
    }

    if (confirmUnpair) {
        AlertDialog(
            onDismissRequest = { confirmUnpair = false },
            title = { Text("Unpair from $pc?") },
            text = { Text("What already synced stays on this ${legato.device}; nothing new syncs until you pair again.") },
            confirmButton = { TextButton(onClick = { confirmUnpair = false; legato.run { unpair() } }) { Text("Unpair", color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirmUnpair = false }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun Chevron() {
    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = LocalStatus.current.faint)
}

/** The paired PC: its mark, name and status, with Sync now and More. */
@Composable
private fun PcCard(legato: Legato, onSync: () -> Unit, onMore: () -> Unit) {
    val state by legato.state.collectAsStateWithLifecycle()
    val status = pcStatus(state, legato.device, long = true)
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.45f), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 18.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Box(Modifier.size(48.dp).background(MaterialTheme.colorScheme.primaryContainer, CircleShape), contentAlignment = Alignment.Center) {
                    Icon(Icons.Default.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size(22.dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(state.pcName ?: "PC", style = MaterialTheme.typography.titleMedium)
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(top = 2.dp)) {
                        StatusDot(status.color)
                        Text(status.text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = onSync,
                    enabled = state.online,
                    colors = androidx.compose.material3.ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainer,
                        contentColor = MaterialTheme.colorScheme.onSurface,
                    ),
                    modifier = Modifier.weight(1f).heightIn(min = 44.dp),
                ) {
                    Icon(Icons.Default.Sync, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.size(8.dp))
                    Text("Sync now")
                }
                TextButton(onClick = onMore, modifier = Modifier.weight(1f).heightIn(min = 44.dp)) {
                    Icon(Icons.Default.MoreHoriz, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.size(8.dp))
                    Text("More")
                }
            }
        }
    }
}

/** Automatic or Manual, each with what it means on Android. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ClipModeSheet(pc: String, auto: Boolean, onPick: (Boolean) -> Unit, onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 28.dp)) {
            Text("Shared clipboard", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
            Spacer(Modifier.height(8.dp))
            listOf(
                true to ("Automatic" to "Copies on $pc land on this clipboard at once. What you copy here goes to $pc when you open Legato, since Android lets apps read the clipboard only while they're open."),
                false to ("Manual" to "Nothing moves on its own. Send from here, the share menu or selected text, and pick $pc's copies from Recent clips."),
            ).forEach { (value, words) ->
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp))
                        .selectable(selected = auto == value, role = Role.RadioButton) { onPick(value); onDismiss() }
                        .padding(12.dp),
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    RadioButton(selected = auto == value, onClick = null)
                    Column(Modifier.weight(1f)) {
                        Text(words.first, style = MaterialTheme.typography.bodyLarge)
                        Text(words.second, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint, modifier = Modifier.padding(top = 2.dp))
                    }
                }
            }
        }
    }
}

/** The clips the PC sent since the app started, newest first; tap to copy. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RecentClipsSheet(legato: Legato, pc: String, onCopied: () -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val clips = remember { runCatching { legato.core.clips() }.getOrDefault(emptyList()).filter { !it.text.isNullOrBlank() }.reversed() }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 28.dp)) {
            Text("Recent clips", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
            if (clips.isEmpty()) {
                Text("Nothing from $pc yet. Copy something there and it shows up here.", style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint, modifier = Modifier.padding(4.dp, 8.dp))
            }
            clips.forEach { c ->
                Column(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp))
                        .clickable { copy(context, c.text ?: ""); onDismiss(); onCopied() }
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                ) {
                    Text(c.text ?: "", style = MaterialTheme.typography.bodyLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Text(c.fromApp?.let { "From $it on $pc" } ?: "From $pc", style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint)
                }
            }
        }
    }
}

/** Whether what you say goes to the PC's speech model while connected. */
@Composable
private fun PcModelRow(legato: Legato, paired: Boolean, pcName: String?) {
    var on by remember { mutableStateOf(legato.usePcModel) }
    SettingRow(
        "Use your PC's speech model",
        if (paired) "While connected to ${pcName ?: "your PC"}, it writes up what you say, with your Dictionary."
        else "Pair with your PC to have its model write up what you say.",
        enabled = paired,
        trailing = { Switch(checked = on && paired, enabled = paired, onCheckedChange = { on = it; legato.usePcModel = it }) },
    )
}

/** Whether a dictation listens on when the keyboard closes. */
@Composable
private fun KeyboardCloseRow(legato: Legato) {
    var on by remember { mutableStateOf(legato.listenThroughKeyboardClose) }
    SettingRow(
        "Keep listening when the keyboard closes",
        if (on) "Switching apps or pulling down the notifications doesn't stop a dictation."
        else "Closing the keyboard ends the dictation and writes up what you said.",
        trailing = { Switch(checked = on, onCheckedChange = { on = it; legato.listenThroughKeyboardClose = it }) },
    )
}

/** Whether the phone may use Bluetooth, for when it isn't on the PC's network. */
@Composable
private fun BluetoothRow(onAllowed: () -> Unit) {
    val context = LocalContext.current
    var allowed by remember { mutableStateOf(!BluetoothLink.needsPermission(context)) }
    var on by remember { mutableStateOf(BluetoothLink.isOn(context)) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        allowed = !BluetoothLink.needsPermission(context)
        on = BluetoothLink.isOn(context)
    }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        allowed = it
        if (it) onAllowed()
    }
    SettingRow(
        "Bluetooth",
        when {
            !allowed -> "Allow it to sync with your PC when you're not on the same Wi-Fi."
            !on -> "Turn on Bluetooth to sync with your PC when you're not on the same Wi-Fi."
            else -> "Syncs over Bluetooth when your PC is nearby and not on the same Wi-Fi."
        },
        trailing = {
            if (!allowed) OutlinedButton(onClick = { ask.launch(Manifest.permission.BLUETOOTH_CONNECT) }) { Text("Allow") }
        },
    )
}

/** The speech model: which one, download it, watch it come in; tap to choose another. */
@Composable
fun ModelRow(legato: Legato) {
    val model by legato.model.collectAsStateWithLifecycle()
    val state by legato.state.collectAsStateWithLifecycle()
    var choosing by remember { mutableStateOf(false) }
    val pcWrites = state.online && state.pcTranscribes && legato.usePcModel
    val m = legato.selectedModel
    Column(
        Modifier.fillMaxWidth().clickable(enabled = model !is ModelStatus.Downloading) { choosing = true }
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Speech model on this ${legato.device}", style = MaterialTheme.typography.bodyLarge)
                Text(
                    when (val s = model) {
                        ModelStatus.Missing ->
                            if (pcWrites) "Optional while ${state.pcName ?: "your PC"} writes up what you say. ${m.name}, ${m.size}, for when it's away."
                            else "${m.name}, ${m.size}. Nothing you say leaves the ${legato.device}."
                        is ModelStatus.Downloading -> if (s.percent >= 0) "${m.name} · downloading · ${s.percent}%" else "${m.name} · downloading"
                        ModelStatus.Downloaded, is ModelStatus.Loaded -> "${m.name} · ready"
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = LocalStatus.current.faint,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            Spacer(Modifier.size(12.dp))
            if (model == ModelStatus.Missing) Button(onClick = { legato.fetchModel() }) { Text("Download") }
            else if (model !is ModelStatus.Downloading) Chevron()
        }
        (model as? ModelStatus.Downloading)?.let { s ->
            Spacer(Modifier.height(10.dp))
            if (s.percent >= 0) LinearProgressIndicator(progress = { s.percent / 100f }, modifier = Modifier.fillMaxWidth())
            else LinearProgressIndicator(Modifier.fillMaxWidth())
        }
    }
    if (choosing) ModelSheet(legato, onDismiss = { choosing = false })
}

/**
 * The models this device can run, small to large. Picking one that isn't
 * here downloads it; the others downloaded can be deleted for the space.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModelSheet(legato: Legato, onDismiss: () -> Unit) {
    val version by legato.version.collectAsStateWithLifecycle()
    val selected = legato.selectedModel.id
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 28.dp).verticalScroll(rememberScrollState())) {
            Text("Speech model", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
            Text(
                "Writes up what you say when your PC doesn't. Bigger models are more accurate and use more space, memory and battery. Each holds about its size in memory while it's loaded. It's put away after five minutes unused.",
                style = MaterialTheme.typography.bodyMedium,
                color = LocalStatus.current.faint,
                modifier = Modifier.padding(4.dp, 4.dp, 4.dp, 8.dp),
            )
            PhoneModels.all.forEachIndexed { i, m ->
                val heavy = m.bytes >= PhoneModels.HEAVY_BYTES
                if (i == 0 || heavy != (PhoneModels.all[i - 1].bytes >= PhoneModels.HEAVY_BYTES)) {
                    Text(
                        if (heavy) "Heavier and more accurate" else "Light and quick",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(start = 4.dp, top = 12.dp, bottom = 4.dp),
                    )
                }
                val here = remember(version) { legato.isDownloaded(m) }
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp))
                        .selectable(selected = m.id == selected, role = Role.RadioButton) { legato.selectModel(m); onDismiss() }
                        .padding(12.dp),
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    RadioButton(selected = m.id == selected, onClick = null)
                    Column(Modifier.weight(1f)) {
                        Text(m.name, style = MaterialTheme.typography.bodyLarge)
                        Text(
                            m.blurb + " " + if (here) "${m.size}, on this ${legato.device}." else "${m.size} to download.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = LocalStatus.current.faint,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    }
                    if (here && m.id != selected) {
                        TextButton(onClick = { legato.deleteModel(m) }) { Text("Delete") }
                    }
                }
            }
        }
    }
}

/**
 * The PC's transforms on the phone: one run on every dictation, and where
 * they run (the PC while connected, else the provider it shared, with the key
 * kept sealed on the phone).
 */
@Composable
private fun TransformRows(legato: Legato) {
    val state by legato.state.collectAsStateWithLifecycle()
    val version by legato.version.collectAsStateWithLifecycle()
    var picking by remember { mutableStateOf(false) }
    var after by remember { mutableStateOf(legato.afterDictation) }
    val pc = state.pcName ?: "your PC"
    val transforms = remember(version, state.paired) { runCatching { legato.core.transforms() }.getOrDefault(emptyList()) }
    val providers = remember(version, state.online) { legato.core.aiProviders() }
    LaunchedEffect(state.online) {
        if (state.online) withContext(Dispatchers.Default) { runCatching { legato.core.pullAi() } }
    }
    val chosen = transforms.firstOrNull { it.id == after }
    SettingRow(
        "After each dictation",
        when {
            !state.paired -> "Pair with your PC to use its transforms."
            transforms.isEmpty() -> "No transforms yet. Switch some on in Legato on $pc."
            chosen != null -> "${chosen.name} rewrites what you say before it's typed."
            else -> "Typed as you said it. Pick a transform to rewrite every dictation."
        },
        enabled = state.paired && transforms.isNotEmpty(),
        onClick = { picking = true },
        trailing = { Text(chosen?.name ?: "None", style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint) },
    )
    Hairline()
    val provider = providers.firstOrNull()
    SettingRow(
        "Where they run",
        when {
            !state.paired -> "On your PC, or with the AI provider it shares."
            state.online && legato.core.pcTransforms() -> "On $pc while connected. Its AI key stays there."
            state.online && provider == null -> "Update Legato on $pc to run them from this ${legato.device}."
            provider != null && legato.hasKey(provider.id) -> "Away from $pc: ${provider.label} · ${provider.model}. The key is sealed in this ${legato.device}'s secure hardware."
            provider != null -> "Away from $pc: ${provider.label} · ${provider.model}."
            else -> "Only while connected. To run them away, turn on Share AI keys with your phone on $pc."
        },
    )
    Hairline()
    SettingRow(
        "On selected text",
        "Select text in any app, then pick Transform from its menu.",
    )
    if (picking) {
        AfterDictationSheet(
            transforms = transforms,
            selected = after,
            onPick = { after = it; legato.afterDictation = it },
            onDismiss = { picking = false },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AfterDictationSheet(
    transforms: List<tech.abhashchakraborty.legato.core.TransformItem>,
    selected: String?,
    onPick: (String?) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 28.dp).verticalScroll(rememberScrollState())) {
            Text("After each dictation", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
            Text(
                "The transform rewrites what you say before it's typed. If it can't run, your words are typed as you said them.",
                style = MaterialTheme.typography.bodyMedium,
                color = LocalStatus.current.faint,
                modifier = Modifier.padding(4.dp, 4.dp, 4.dp, 8.dp),
            )
            val options = listOf<Pair<String?, Pair<String, String>>>(null to ("None" to "Typed as you said it.")) +
                transforms.map { it.id to (it.name to it.description) }
            options.forEach { (id, text) ->
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp))
                        .selectable(selected = id == selected, role = Role.RadioButton) { onPick(id); onDismiss() }
                        .padding(12.dp),
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    RadioButton(selected = id == selected, onClick = null)
                    Column(Modifier.weight(1f)) {
                        Text(text.first, style = MaterialTheme.typography.bodyLarge)
                        if (text.second.isNotBlank()) {
                            Text(text.second, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint, modifier = Modifier.padding(top = 2.dp))
                        }
                    }
                }
            }
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
