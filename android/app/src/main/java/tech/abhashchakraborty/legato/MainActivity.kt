package tech.abhashchakraborty.legato

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import tech.abhashchakraborty.legato.bubble.BubbleAccessibilityService
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.background
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.core.HistoryEntry
import tech.abhashchakraborty.legato.core.coreVersion
import java.text.DateFormat
import java.util.Date

/**
 * Step A1: a plain screen that proves the pieces on a real phone: pairing
 * and sync through the shared core, speech on the phone, and the clipboard
 * both ways. The real screens come in A3.
 */
class MainActivity : ComponentActivity() {
    private val recorder = Recorder()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val linkFromIntent = pairingLink(intent)
        setContent {
            LegatoTheme {
                DebugScreen(recorder, linkFromIntent)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        pairingLink(intent)?.let { link -> legato.run { pair(link) } }
    }

    override fun onStart() {
        super.onStart()
        if (legato.state.value.paired) legato.run { connect() }
    }

    private fun pairingLink(intent: Intent?): String? {
        val uri = intent?.data ?: return null
        return if (uri.scheme == "legato" && uri.host == "pair") uri.toString() else null
    }
}

@Composable
fun LegatoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val context = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        dark -> darkColorScheme(primary = Color(0xFF66B8AD))
        else -> lightColorScheme(primary = Color(0xFF1A5C5C))
    }
    MaterialTheme(colorScheme = scheme, content = content)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DebugScreen(recorder: Recorder, linkFromIntent: String?) {
    val context = LocalContext.current
    val legato = context.legato
    val state by legato.state.collectAsStateWithLifecycle()
    val lines by legato.lines.collectAsStateWithLifecycle()
    val model by legato.model.collectAsStateWithLifecycle()
    var link by remember { mutableStateOf(linkFromIntent ?: "") }
    var words by remember { mutableStateOf(listOf<String>()) }
    var history by remember { mutableStateOf(listOf<HistoryEntry>()) }
    var newWord by remember { mutableStateOf("") }
    var recording by remember { mutableStateOf(false) }
    var heard by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val scope = androidx.compose.runtime.rememberCoroutineScope()

    fun reload() {
        scope.launch {
            words = runCatching { withContext(Dispatchers.Default) { legato.core.words() } }.getOrDefault(words)
            history = runCatching { withContext(Dispatchers.Default) { legato.core.history(20u) } }.getOrDefault(history)
        }
    }
    LaunchedEffect(lines.size) { reload() }
    LaunchedEffect(linkFromIntent) { if (!linkFromIntent.isNullOrBlank() && !state.paired) legato.run { pair(linkFromIntent) } }

    val micPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) { recorder.start(); recording = true }
    }

    fun toggleRecording() {
        if (recording) {
            recording = false
            busy = true
            scope.launch {
                val samples = recorder.stop()
                val text = runCatching {
                    withContext(Dispatchers.Default) { legato.core.transcribe(samples.toList()) }
                }.getOrElse { e -> legato.onLine(e.message ?: "Transcription failed"); "" }
                heard = text
                if (text.isNotBlank()) legato.run { dictate(text) }
                busy = false
            }
        } else if (context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            recorder.start(); recording = true
        } else {
            micPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    Scaffold(topBar = { TopAppBar(title = { Text("Legato · test build") }) }) { padding ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                Section("Your PC") {
                    if (!state.paired) {
                        Text("On the PC: Legato › Settings › Devices › Pair a phone › Copy link. Paste the link here.", style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(8.dp))
                        OutlinedTextField(link, { link = it }, label = { Text("Pairing link") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
                        Spacer(Modifier.height(8.dp))
                        Button(onClick = { legato.run { pair(link) } }, enabled = link.startsWith("legato://pair?")) { Text("Pair") }
                    } else {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Dot(state.online)
                            Spacer(Modifier.width(8.dp))
                            Column {
                                Text(state.pcName ?: "PC", style = MaterialTheme.typography.titleMedium)
                                Text(
                                    (if (state.online) "Connected" else "Not connected") +
                                        (state.lastSync?.let { " · last synced ${timeOf(it)}" } ?: ""),
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                        }
                        Spacer(Modifier.height(8.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FilledTonalButton(onClick = { legato.run { if (!syncNow()) legato.onLine("Not connected") } }) { Text("Sync now") }
                            OutlinedButton(onClick = { legato.run { connect() } }) { Text("Connect") }
                            TextButton(onClick = { legato.run { unpair() } }) { Text("Unpair") }
                        }
                    }
                }
            }
            item {
                val serviceOn by BubbleAccessibilityService.running.collectAsStateWithLifecycle()
                var overlayOn by remember { mutableStateOf(Settings.canDrawOverlays(context)) }
                val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
                Section("Bubble over Gboard") {
                    Text(
                        "The bubble shows when your keyboard opens and types what you dictate. Two Android switches make it work.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Spacer(Modifier.height(8.dp))
                    SetupRow(
                        title = "Legato under Accessibility",
                        done = serviceOn,
                        detail = "Lets Legato see the keyboard open and type into the field.",
                    ) { context.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                    SetupRow(
                        title = "Display over other apps",
                        done = overlayOn,
                        detail = "So the bubble can sit over other apps and keep the microphone while you talk.",
                    ) {
                        context.startActivity(
                            Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}"))
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                        )
                    }
                    LaunchedEffect(lines.size, serviceOn) { overlayOn = Settings.canDrawOverlays(context) }
                    if (Build.VERSION.SDK_INT >= 33 && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                        SetupRow(
                            title = "Notifications",
                            done = false,
                            detail = "Android shows a small notice while Legato holds the microphone.",
                        ) { notifications.launch(Manifest.permission.POST_NOTIFICATIONS) }
                    }
                    if (context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
                        SetupRow(title = "Microphone", done = false, detail = "Needed to hear you.") { micPermission.launch(Manifest.permission.RECORD_AUDIO) }
                    }
                }
            }
            item {
                Section("Speech model") {
                    when (val m = model) {
                        ModelStatus.Missing -> {
                            Text("Moonshine Base, about 60 MB, runs on this phone.", style = MaterialTheme.typography.bodyMedium)
                            Spacer(Modifier.height(8.dp))
                            Button(onClick = { legato.fetchModel() }) { Text("Download") }
                        }
                        is ModelStatus.Downloading -> {
                            Text(if (m.percent >= 0) "Downloading · ${m.percent}%" else "Downloading…")
                            Spacer(Modifier.height(8.dp))
                            if (m.percent >= 0) LinearProgressIndicator(progress = { m.percent / 100f }, modifier = Modifier.fillMaxWidth())
                            else LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                        }
                        ModelStatus.Downloaded -> {
                            Text("Downloaded, not loaded yet.")
                            Spacer(Modifier.height(8.dp))
                            Button(onClick = { legato.loadModel() }) { Text("Load") }
                        }
                        is ModelStatus.Loaded -> Text("Loaded (${m.arch}).")
                    }
                }
            }
            item {
                Section("Dictate") {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        FilledTonalButton(onClick = { toggleRecording() }, enabled = model is ModelStatus.Loaded && !busy) {
                            Icon(if (recording) Icons.Default.Stop else Icons.Default.Mic, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text(if (recording) "Finish" else if (busy) "Transcribing…" else "Talk")
                        }
                        if (model !is ModelStatus.Loaded) Text("Load the model first", style = MaterialTheme.typography.bodySmall)
                    }
                    if (heard.isNotBlank()) {
                        Spacer(Modifier.height(8.dp))
                        Text(heard, style = MaterialTheme.typography.bodyLarge)
                        TextButton(onClick = {
                            val cm = context.getSystemService(ClipboardManager::class.java)
                            cm.setPrimaryClip(ClipData.newPlainText("Legato", heard))
                        }) { Text("Copy") }
                    }
                }
            }
            item {
                Section("Clipboard") {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilledTonalButton(onClick = {
                            val cm = context.getSystemService(ClipboardManager::class.java)
                            val text = cm.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString().orEmpty()
                            if (text.isBlank()) legato.onLine("Nothing on the clipboard")
                            else legato.run { if (sendClip(text)) legato.onLine("Sent to the PC: $text") else legato.onLine("Not connected; the clip wasn't sent") }
                        }, enabled = state.paired) { Text("Send clipboard to PC") }
                    }
                    val clips = remember(lines.size) { runCatching { legato.core.clips() }.getOrDefault(emptyList()) }
                    clips.takeLast(5).reversed().forEach { c ->
                        HorizontalDivider(Modifier.padding(vertical = 6.dp))
                        Text(c.text ?: "(image)", maxLines = 2, overflow = TextOverflow.Ellipsis)
                        TextButton(onClick = {
                            val cm = context.getSystemService(ClipboardManager::class.java)
                            cm.setPrimaryClip(ClipData.newPlainText("Legato", c.text ?: ""))
                        }) { Text("Copy") }
                    }
                }
            }
            item {
                Section("Dictionary · ${words.size}") {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(newWord, { newWord = it }, label = { Text("Word") }, modifier = Modifier.weight(1f), singleLine = true)
                        Button(onClick = { val w = newWord.trim(); newWord = ""; legato.run { addWord(w) } }, enabled = newWord.isNotBlank()) { Text("Add") }
                    }
                    words.forEach { w ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(w, modifier = Modifier.weight(1f))
                            TextButton(onClick = { legato.run { removeWord(w) } }) { Text("Remove") }
                        }
                    }
                }
            }
            item {
                Section("History") {
                    if (history.isEmpty()) Text("Nothing yet.", style = MaterialTheme.typography.bodySmall)
                    history.forEach { h ->
                        Column(Modifier.padding(vertical = 6.dp)) {
                            Text(h.text, maxLines = 3, overflow = TextOverflow.Ellipsis)
                            Text(
                                listOfNotNull(h.app.ifBlank { null }, timeOf(h.at), h.device?.let { "On $it" }).joinToString(" · "),
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                }
            }
            item {
                Section("Log · core ${coreVersion()}") {
                    lines.reversed().take(12).forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
                }
            }
        }
    }
}

@Composable
private fun SetupRow(title: String, done: Boolean, detail: String, onOpen: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Dot(done)
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(detail, style = MaterialTheme.typography.bodySmall)
        }
        if (!done) TextButton(onClick = onOpen) { Text("Open") } else Text("On", style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(10.dp))
            content()
        }
    }
}

@Composable
private fun Dot(on: Boolean) {
    androidx.compose.foundation.layout.Box(
        Modifier.size(10.dp).background(if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline, CircleShape),
    )
}

private fun timeOf(epochSeconds: Long): String =
    DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(epochSeconds * 1000))
