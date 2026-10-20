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
    val history by coreList { history(200u) }
    var query by remember { mutableStateOf("") }
    var menuFor by remember { mutableStateOf<HistoryEntry?>(null) }
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
                    SyncChip(
                        online = state.online,
                        text = if (state.online) "${state.pcName} · synced ${state.lastSync?.let { ago(it) } ?: "just now"}" else "${state.pcName} not reachable · changes wait here",
                        onClick = onSettings,
                    )
                } else {
                    Surface(onClick = onPair, shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
                        ListItem(
                            headlineContent = { Text("Pair with your PC") },
                            supportingContent = { Text("Get your Dictionary, snippets and history from Legato on Windows.") },
                            leadingContent = { Icon(Icons.Default.Computer, contentDescription = null) },
                            colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                        )
                    }
                }
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = query, onValueChange = { query = it }, singleLine = true,
                    placeholder = { Text("Search your dictations") },
                    leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
                    shape = RoundedCornerShape(25.dp),
                    modifier = Modifier.fillMaxWidth(),
                )
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
                                Text(listOfNotNull(e.app.ifBlank { null }, timeOf(e.at), e.device?.let { "On $it" }).joinToString(" · "))
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
        ModalBottomSheet(onDismissRequest = { menuFor = null }) {
            SheetItem("Copy") { copy(context, e.text); menuFor = null }
            SheetItem("Share") { share(context, e.text); menuFor = null }
            SheetItem("Add a word from this") { menuFor = null; AddWordSheet.open(e.text) }
            SheetItem("Delete", danger = true) {
                menuFor = null
                legato.run { deleteHistory(e.id) }
                scope.launch {
                    val r = snackbar.showSnackbar("Dictation deleted", actionLabel = "Undo", duration = SnackbarDuration.Short)
                    if (r == SnackbarResult.ActionPerformed) legato.run { dictate(e.text) }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
    AddWordSheet.Host()
}

@Composable
fun SyncChip(online: Boolean, text: String, onClick: () -> Unit) {
    AssistChip(
        onClick = onClick,
        label = { Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingIcon = { Box(Modifier.size(8.dp).background(if (online) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline, CircleShape)) },
    )
}

@Composable
fun AppDot(fromPc: Boolean) {
    Box(
        Modifier.size(36.dp).background(MaterialTheme.colorScheme.primaryContainer, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(if (fromPc) Icons.Default.Computer else Icons.Default.Mic, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size(18.dp))
    }
}

@Composable
fun DayHeader(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp, top = 18.dp, bottom = 8.dp))
}

/** Rows grouped into one rounded card: the first and last get the corners. */
@Composable
fun Card(first: Boolean, last: Boolean, content: @Composable () -> Unit) {
    val r = 20.dp
    val shape = RoundedCornerShape(topStart = if (first) r else 0.dp, topEnd = if (first) r else 0.dp, bottomStart = if (last) r else 0.dp, bottomEnd = if (last) r else 0.dp)
    Surface(shape = shape, color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
        Column {
            if (!first) androidx.compose.material3.HorizontalDivider(Modifier.padding(start = 68.dp, end = 16.dp))
            content()
        }
    }
}

@Composable
fun Empty(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String) {
    Column(Modifier.fillMaxWidth().padding(vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.outline, modifier = Modifier.size(28.dp))
        Spacer(Modifier.height(8.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
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
