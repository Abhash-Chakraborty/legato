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
import tech.abhashchakraborty.legato.core.NoteDetail
import tech.abhashchakraborty.legato.core.NoteRow
import java.text.DateFormat
import java.util.Date

private fun NoteRow.meta(): String = listOfNotNull(
    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(startedAt * 1000)),
    minutes(durationMs),
    openActions.toInt().takeIf { it > 0 }?.let { if (it == 1) "1 open action" else "$it open actions" },
).joinToString(" · ")

/** Meetings and voice memos recorded on the PC, after a sync. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotesScreen(onOpen: (Long) -> Unit) {
    val notes by coreList { notes() }
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = { LargeTopAppBar(title = { Text("Notes") }, scrollBehavior = scrollBehavior) },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 24.dp),
        ) {
            if (notes.isEmpty()) item {
                Empty(Icons.Outlined.GraphicEq, "Meetings and voice memos from your PC show up here after a sync.")
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
                                        Text(n.meta())
                                        if (n.summary.isNotBlank()) Text(n.summary, maxLines = 2, overflow = TextOverflow.Ellipsis, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
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
}
