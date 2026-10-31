package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.outlined.GraphicEq
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import kotlinx.coroutines.delay
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.Memo
import tech.abhashchakraborty.legato.core.MemoLine
import tech.abhashchakraborty.legato.core.NoteDetail
import tech.abhashchakraborty.legato.core.NoteRow
import java.text.DateFormat
import java.util.Date

private fun NoteRow.meta(): String = listOfNotNull(
    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(startedAt * 1000)),
    minutes(durationMs),
    openActions.toInt().takeIf { it > 0 }?.let { if (it == 1) "1 open action" else "$it open actions" },
).joinToString(" · ")

/**
 * Meetings and voice memos: the PC's after a sync, and memos recorded here.
 * A memo is written up when you stop, by the PC's model while connected or
 * this device's own, and syncs to the PC as a note.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotesScreen(onOpen: (Long) -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val model by legato.model.collectAsStateWithLifecycle()
    val notes by coreList { notes() }
    val memo by Memo.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    // Ticks the clock while a memo records.
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(memo.recording) {
        while (memo.recording) {
            now = System.currentTimeMillis()
            delay(250)
        }
    }

    val mic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (ok) Memo.start(context) }
    fun record() {
        when {
            memo.recording -> Memo.finish(context)
            memo.writing != null -> Unit
            !legato.canTranscribe() && !legato.pcMayTranscribe() -> scope.launch {
                snackbar.showSnackbar(
                    when (model) {
                        is ModelStatus.Downloading -> "The speech model is still downloading."
                        ModelStatus.Downloaded -> "The speech model is loading; try again in a moment."
                        else -> "Download the speech model in Settings, or connect to your PC."
                    },
                )
            }
            micGranted(context) -> Memo.start(context)
            else -> mic.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text("Notes") }, scrollBehavior = scrollBehavior) },
        floatingActionButton = {
            val writing = memo.writing
            ExtendedFloatingActionButton(
                onClick = { record() },
                expanded = true,
                icon = {
                    when {
                        writing != null -> CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onPrimaryContainer)
                        memo.recording -> Icon(Icons.Default.Stop, contentDescription = null)
                        else -> Icon(Icons.Default.Mic, contentDescription = null)
                    }
                },
                text = {
                    when {
                        writing != null -> Text(writing)
                        memo.recording -> Row(verticalAlignment = Alignment.CenterVertically) {
                            Text("Stop · ${clock((now - memo.startedAt).coerceAtLeast(0))}")
                            WaveMeter(memo.level, Modifier.padding(start = 12.dp).width(72.dp).height(24.dp), color = MaterialTheme.colorScheme.onPrimaryContainer)
                        }
                        else -> Text("Record a memo")
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 96.dp),
        ) {
            if (notes.isEmpty()) item {
                Empty(Icons.Outlined.GraphicEq, "Record a memo here, or meetings and memos from your PC show up after a sync.")
            }
            notes.groupBy { dayLabel(it.startedAt) }.forEach { (day, rows) ->
                item(key = "day-$day") { DayHeader(day) }
                itemsIndexed(rows, key = { _, n -> n.id }) { i, n ->
                    Column(Modifier.animateItem()) {
                        Card(first = i == 0, last = i == rows.lastIndex) {
                            ListItem(
                                headlineContent = { Text(n.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                supportingContent = {
                                    Column {
                                        Text(n.meta(legato.device), color = LocalStatus.current.faint)
                                        if (n.summary.isNotBlank()) Text(n.summary, maxLines = 2, overflow = TextOverflow.Ellipsis, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
                                leadingContent = { NoteMark(n.device != null) },
                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                modifier = Modifier.clickable { onOpen(n.id) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun NoteMark(fromPc: Boolean) {
    Box(
        Modifier.size(36.dp).background(MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.6f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(if (fromPc) Icons.Default.Computer else Icons.Outlined.GraphicEq, contentDescription = null, tint = MaterialTheme.colorScheme.onSecondaryContainer, modifier = Modifier.size(18.dp))
    }
}

private fun clock(ms: Long): String {
    val s = ms / 1000
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
}

private fun NoteRow.meta(device: String): String = listOfNotNull(
    DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(startedAt * 1000)),
    minutes(durationMs),
    if (this.device == null) "On this $device" else "On ${this.device}",
    openActions.toInt().takeIf { it > 0 }?.let { if (it == 1) "1 open action" else "$it open actions" },
).joinToString(" · ")

/** One note: its summary, the action items to tick off, and the transcript. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteScreen(id: Long, onBack: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val version by legato.version.collectAsStateWithLifecycle()
    var detail by remember { mutableStateOf<NoteDetail?>(null) }
    var missing by remember { mutableStateOf(false) }
    // Ticks show at once; the core catches up in the background.
    val ticked = remember { mutableStateMapOf<Long, Boolean>() }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    LaunchedEffect(id, version) {
        val r = withContext(Dispatchers.Default) { runCatching { legato.core.note(id) } }
        r.onSuccess { detail = it; missing = false }.onFailure { missing = detail == null }
    }

    val transcript = detail?.transcript.orEmpty()
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            LargeTopAppBar(
                title = { Text(detail?.note?.title ?: "", maxLines = 2, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
                actions = {
                    if (transcript.isNotEmpty()) IconButton(onClick = {
                        copy(context, transcript.joinToString("\n"))
                        scope.launch { snackbar.showSnackbar("Transcript copied") }
                    }) { Icon(Icons.Default.ContentCopy, contentDescription = "Copy transcript") }
                    if (transcript.isNotEmpty()) IconButton(onClick = { TransformSheet.open(transcript.joinToString("\n")) }) {
                        Icon(Icons.Default.SwapHoriz, contentDescription = "Transform the transcript")
                    }
                },
                scrollBehavior = scrollBehavior,
            )
        },
    ) { padding ->
        val d = detail
        if (d == null) {
            Column(Modifier.fillMaxSize().padding(padding)) {
                if (missing) Empty(Icons.Outlined.GraphicEq, "This note isn't on this ${legato.device} any more.")
                else LinearProgressIndicator(Modifier.fillMaxWidth().padding(16.dp))
            }
            return@Scaffold
        }
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp),
        ) {
            item {
                Text(d.note.meta(), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 8.dp))
            }
            if (d.note.summary.isNotBlank()) item {
                DayHeader("Summary")
                Text(d.note.summary, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = 8.dp))
            }
            if (d.actions.isNotEmpty()) {
                item { DayHeader("Action items") }
                itemsIndexed(d.actions, key = { _, a -> "a${a.id}" }) { i, a ->
                    val done = ticked[a.id] ?: a.done
                    Card(first = i == 0, last = i == d.actions.lastIndex) {
                        ListItem(
                            headlineContent = {
                                Text(
                                    a.text,
                                    textDecoration = if (done) TextDecoration.LineThrough else null,
                                    color = if (done) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                                )
                            },
                            leadingContent = {
                                Checkbox(checked = done, onCheckedChange = { on ->
                                    ticked[a.id] = on
                                    legato.run { setActionDone(a.id, on) }
                                })
                            },
                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                        )
                    }
                }
            }
            if (transcript.isNotEmpty()) {
                item { DayHeader("Transcript") }
                items(transcript) { line ->
                    val who = line.substringBefore(": ", "")
                    val text = line.substringAfter(": ", line)
                    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp), verticalAlignment = Alignment.Top) {
                        Text(
                            when (who) { "you" -> "You"; "them" -> "Them"; else -> who.replaceFirstChar { it.uppercase() } },
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.SemiBold,
                            color = if (who == "you") MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.width(56.dp).padding(top = 2.dp),
                        )
                        Text(text, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                    }
                }
            }
            item { Spacer(Modifier.height(16.dp)) }
        }
    }
    TransformSheet.Host()
}
