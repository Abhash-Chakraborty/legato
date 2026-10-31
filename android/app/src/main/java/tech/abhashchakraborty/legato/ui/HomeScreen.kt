package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material3.IconButton
import androidx.compose.material3.AssistChip
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
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
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import tech.abhashchakraborty.legato.core.HistoryEntry
import java.text.DateFormat
import java.util.Calendar
import java.util.Date

/** Dictations from both devices, newest first, with the PC's ones marked. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun HomeScreen(onSettings: () -> Unit, onPair: () -> Unit) {
    val legato = rememberLegato()
    val state by legato.state.collectAsStateWithLifecycle()
    // A dictation that came out empty on the PC has nothing to show or copy.
    val history by coreList { history(200u).filter { it.text.isNotBlank() } }
    var query by remember { mutableStateOf("") }
    var menuFor by remember { mutableStateOf<HistoryEntry?>(null) }
    val cancelled by legato.cancelled.collectAsStateWithLifecycle()
    var writing by remember { mutableStateOf(false) }
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    val shown = if (query.isBlank()) history else history.filter {
        it.text.contains(query, ignoreCase = true) || it.app.contains(query, ignoreCase = true)
    }
    val days = shown.groupBy { dayLabel(it.at) }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text("Home") }, scrollBehavior = scrollBehavior) },
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 4.dp)) {
            item {
                if (state.paired) {
                    val status = pcStatus(state, legato.device)
                    SyncChip(status, onClick = onSettings)
                } else {
                    Surface(onClick = onPair, shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
                        ListItem(
                            headlineContent = { Text("Pair with your PC") },
                            supportingContent = { Text("Get your Dictionary, snippets and history from Legato on Windows.", color = LocalStatus.current.faint) },
                            leadingContent = { AppDot(true) },
                            trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = LocalStatus.current.faint) },
                            colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                        )
                    }
                }
                Spacer(Modifier.height(14.dp))
                SearchField(query, { query = it }, "Search your dictations")
            }
            cancelled?.let { seconds ->
                item(key = "cancelled") {
                    Spacer(Modifier.height(10.dp))
                    Card(first = true, last = true) {
                        ListItem(
                            headlineContent = { Text(if (writing) "Writing it up…" else "Cancelled dictation, $seconds s") },
                            supportingContent = { Text("Kept in case cancelling was a slip.", color = LocalStatus.current.faint) },
                            leadingContent = { AppDot(false) },
                            trailingContent = {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    IconButton(enabled = !writing, onClick = { legato.discardCancelled() }) {
                                        Icon(Icons.Outlined.Delete, contentDescription = "Discard it", tint = LocalStatus.current.faint)
                                    }
                                    androidx.compose.material3.TextButton(
                                        enabled = !writing,
                                        onClick = {
                                            writing = true
                                            scope.launch {
                                                val ok = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { legato.writeUpCancelled() }
                                                writing = false
                                                snackbar.showSnackbar(if (ok) "Written up below" else "No words in it")
                                            }
                                        },
                                    ) { Text("Write it up") }
                                }
                            },
                            colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                        )
                    }
                }
            }
            if (shown.isEmpty()) {
                item {
                    Empty(
                        icon = if (history.isEmpty()) Icons.Default.Mic else Icons.Default.Search,
                        text = if (history.isEmpty()) "Your dictations show up here." else "Nothing matches “$query”.",
                    )
                }
            }
            days.forEach { (day, entries) ->
                item(key = "day-$day") { DayHeader(day) }
                itemsIndexed(entries, key = { _, e -> e.id }) { index, e ->
                    Card(first = index == 0, last = index == entries.lastIndex) {
                        ListItem(
                            headlineContent = { Text(e.text, maxLines = 3, overflow = TextOverflow.Ellipsis) },
                            supportingContent = {
                                Text(listOfNotNull(e.app.ifBlank { null }, timeOf(e.at), e.device?.let { "On $it" }, "Cancelled".takeIf { e.cancelled }).joinToString(" · "), color = LocalStatus.current.faint)
                            },
                            trailingContent = {
                                IconButton(onClick = { menuFor = e }) { Icon(Icons.Default.MoreVert, contentDescription = "More for this dictation", tint = LocalStatus.current.faint) }
                            },
                            leadingContent = { AppDot(e.device != null) },
                            colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                            modifier = Modifier.combinedClickable(
                                onClick = { copy(context, e.text); scope.launch { snackbar.showSnackbar("Copied") } },
                                onLongClick = { menuFor = e },
                            ),
                        )
                    }
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }

    menuFor?.let { e ->
        MenuSheet(
            title = e.text,
            subtitle = listOfNotNull(e.app.ifBlank { null }, timeOf(e.at), e.device?.let { "On $it" }).joinToString(" · "),
            actions = listOf(
                MenuAction("Copy", Icons.Default.ContentCopy) { copy(context, e.text); scope.launch { snackbar.showSnackbar("Copied") } },
                MenuAction("Share", Icons.Default.Share) { share(context, e.text) },
                MenuAction("Transform", Icons.Default.SwapHoriz) { TransformSheet.open(e.text) },
                MenuAction("Add a word from this", Icons.AutoMirrored.Outlined.MenuBook) { AddWordSheet.open(e.text) },
                MenuAction("Delete", Icons.Outlined.Delete, danger = true) {
                    legato.run { deleteHistory(e.id) }
                    scope.launch {
                        val r = snackbar.showSnackbar("Dictation deleted", actionLabel = "Undo", duration = SnackbarDuration.Short)
                        if (r == SnackbarResult.ActionPerformed) legato.run { dictate(e.text) }
                    }
                },
            ),
            onDismiss = { menuFor = null },
        )
    }
    AddWordSheet.Host()
    TransformSheet.Host()
}

/** The PC's name and how it stands, as a pill that opens Settings. */
@Composable
fun SyncChip(status: PcStatus, onClick: () -> Unit) {
    Surface(onClick = onClick, shape = RoundedCornerShape(17.dp), color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(Modifier.height(34.dp).padding(start = 12.dp, end = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            StatusDot(status.color)
            Text(status.text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = LocalStatus.current.faint, modifier = Modifier.size(16.dp))
        }
    }
}

@Composable
fun AppDot(fromPc: Boolean) {
    Box(
        Modifier.size(36.dp).background(MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.6f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(if (fromPc) Icons.Default.Computer else Icons.Default.Mic, contentDescription = null, tint = MaterialTheme.colorScheme.onSecondaryContainer, modifier = Modifier.size(18.dp))
    }
}

@Composable
fun DayHeader(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp, top = 18.dp, bottom = 8.dp))
}

/** Rows grouped into one rounded card: the first and last get the corners. */
@Composable
fun Card(first: Boolean, last: Boolean, inset: Boolean = true, content: @Composable () -> Unit) {
    val r = 20.dp
    val shape = RoundedCornerShape(topStart = if (first) r else 0.dp, topEnd = if (first) r else 0.dp, bottomStart = if (last) r else 0.dp, bottomEnd = if (last) r else 0.dp)
    Surface(shape = shape, color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
        Column {
            if (!first) androidx.compose.material3.HorizontalDivider(Modifier.padding(start = if (inset) 68.dp else 16.dp, end = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
            content()
        }
    }
}

@Composable
fun Empty(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String) {
    Column(Modifier.fillMaxWidth().padding(vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(icon, contentDescription = null, tint = LocalStatus.current.faint, modifier = Modifier.size(28.dp))
        Spacer(Modifier.height(8.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint, textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(horizontal = 24.dp))
    }
}

@Composable
fun SheetItem(label: String, danger: Boolean = false, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(label, color = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface) },
        modifier = Modifier.clickable(onClick = onClick),
    )
}

fun copy(context: android.content.Context, text: String) {
    val cm = context.getSystemService(android.content.ClipboardManager::class.java)
    cm.setPrimaryClip(android.content.ClipData.newPlainText("Legato", text))
}

fun share(context: android.content.Context, text: String) {
    val intent = android.content.Intent(android.content.Intent.ACTION_SEND).setType("text/plain").putExtra(android.content.Intent.EXTRA_TEXT, text)
    context.startActivity(android.content.Intent.createChooser(intent, null).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
}

fun timeOf(epochSeconds: Long): String = DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(epochSeconds * 1000))

fun dayLabel(epochSeconds: Long): String {
    val now = Calendar.getInstance()
    val then = Calendar.getInstance().apply { timeInMillis = epochSeconds * 1000 }
    val sameYear = now.get(Calendar.YEAR) == then.get(Calendar.YEAR)
    val dayDiff = now.get(Calendar.DAY_OF_YEAR) - then.get(Calendar.DAY_OF_YEAR)
    return when {
        sameYear && dayDiff == 0 -> "Today"
        sameYear && dayDiff == 1 -> "Yesterday"
        else -> DateFormat.getDateInstance(DateFormat.MEDIUM).format(then.time)
    }
}

fun ago(epochSeconds: Long): String {
    val s = System.currentTimeMillis() / 1000 - epochSeconds
    return when {
        s < 60 -> "just now"
        s < 3600 -> "${s / 60} min ago"
        s < 86400 -> "${s / 3600} h ago"
        else -> "${s / 86400} d ago"
    }
}
