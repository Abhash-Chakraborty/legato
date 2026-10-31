package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Refresh
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
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
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
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
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
    var menuFor by remember { mutableStateOf<NoteRow?>(null) }
    // Deleted, while Undo is offered; the note goes once the offer does.
    var hidden by remember { mutableStateOf(setOf<Long>()) }
    val shown = notes.filter { it.id !in hidden }
    // Memos with parts not written up yet, their audio kept to try again.
    val kept by tech.abhashchakraborty.legato.KeptMemos.pending.collectAsStateWithLifecycle()
    var retrying by remember { mutableStateOf(setOf<Long>()) }

    fun tryAgain(n: NoteRow) {
        if (n.id in retrying) return
        retrying = retrying + n.id
        scope.launch {
            val done = withContext(Dispatchers.IO) {
                if (legato.readyToTranscribe()) tech.abhashchakraborty.legato.KeptMemos.writeUp(context, n.id) else -1
            }
            retrying = retrying - n.id
            val left = n.id in tech.abhashchakraborty.legato.KeptMemos.pending.value
            snackbar.showSnackbar(
                when {
                    done < 0 -> "Nothing can write it up yet: connect to your PC or download a model."
                    !left -> "Written up"
                    else -> "Part of it still wasn't written up; it stays kept."
                },
            )
        }
    }

    fun delete(n: NoteRow) {
        hidden = hidden + n.id
        scope.launch {
            var undone = false
            try {
                val r = snackbar.showSnackbar("Deleted ${n.title}", actionLabel = "Undo", duration = SnackbarDuration.Short)
                undone = r == SnackbarResult.ActionPerformed
            } finally {
                if (undone) hidden = hidden - n.id
                else {
                    tech.abhashchakraborty.legato.KeptMemos.forget(context, n.id)
                    legato.run { deleteNote(n.id) }
                }
            }
        }
    }

    // The memo under way, and a note just added (the memo saved, or one from
    // the PC), show at the top: the list keeps its place by the row in view,
    // so they'd otherwise arrive out of sight above it.
    val list = rememberLazyListState()
    val newest = shown.firstOrNull()?.id
    LaunchedEffect(memo.recording) { if (memo.recording) list.animateScrollToItem(0) }
    LaunchedEffect(newest) { if (list.firstVisibleItemIndex <= 2) list.animateScrollToItem(0) }

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
            state = list,
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 96.dp),
        ) {
            // The memo so far, written up half a minute at a time as you talk.
            if (memo.recording) item(key = "heard") {
                Card(first = true, last = true) {
                    Column(Modifier.padding(16.dp).animateContentSize()) {
                        Text("Recording", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                        Text(
                            memo.heard.ifBlank { "Your words show here half a minute at a time as you talk." },
                            style = MaterialTheme.typography.bodyLarge,
                            color = if (memo.heard.isBlank()) LocalStatus.current.faint else MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
                Spacer(Modifier.height(12.dp))
            }
            if (shown.isEmpty() && !memo.recording) item {
                Empty(Icons.Outlined.GraphicEq, "Record a memo here, or meetings and memos from your PC show up after a sync.")
            }
            shown.groupBy { dayLabel(it.startedAt) }.forEach { (day, rows) ->
                item(key = "day-$day") { DayHeader(day) }
                itemsIndexed(rows, key = { _, n -> n.id }) { i, n ->
                    Column(Modifier.animateItem()) {
                        Card(first = i == 0, last = i == rows.lastIndex) {
                            ListItem(
                                headlineContent = { Text(n.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                supportingContent = {
                                    Column {
                                        Text(n.meta(legato.device), color = LocalStatus.current.faint)
                                        kept[n.id]?.let { seconds ->
                                            Text(
                                                if (n.id in retrying) "Writing up the rest…" else "${clock(seconds * 1000L)} not written up yet · long-press to try again",
                                                color = MaterialTheme.colorScheme.error,
                                            )
                                        }
                                        if (n.summary.isNotBlank()) Text(n.summary, maxLines = 2, overflow = TextOverflow.Ellipsis, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
                                leadingContent = { NoteMark(n.device != null) },
                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                modifier = Modifier.combinedClickable(onClick = { onOpen(n.id) }, onLongClick = { menuFor = n }),
                            )
                        }
                    }
                }
            }
        }
    }

    menuFor?.let { n ->
        val pc = n.device
        MenuSheet(
            title = n.title,
            // A PC's note is deleted there, where Recently deleted keeps it a while.
            subtitle = if (pc != null) "Made on $pc; delete it there." else n.meta(legato.device),
            actions = listOfNotNull(
                MenuAction("Open", Icons.Default.Edit) { onOpen(n.id) },
                if (n.id in kept) MenuAction("Try again", Icons.Default.Refresh) { tryAgain(n) } else null,
                if (pc == null) MenuAction("Delete", Icons.Default.Delete, danger = true) { delete(n) } else null,
            ),
            onDismiss = { menuFor = null },
        )
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
