package tech.abhashchakraborty.legato.ui

import tech.abhashchakraborty.legato.bubble.Dictation
import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.automirrored.outlined.StickyNote2
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.core.ScratchNote

private fun ScratchNote.heading(): String =
    title.ifBlank { text.lineSequence().firstOrNull { it.isNotBlank() }?.trim()?.take(80) ?: "" }.ifBlank { "Untitled" }

/** Scratchpad notes from both devices, the last edited first. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun ScratchpadScreen(onOpen: (Long) -> Unit) {
    val legato = rememberLegato()
    val notes by coreList { scratchNotes() }
    var menuFor by remember { mutableStateOf<ScratchNote?>(null) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    fun delete(n: ScratchNote) {
        legato.run { deleteScratch(n.id) }
        scope.launch {
            val r = snackbar.showSnackbar("Deleted ${n.heading()}", actionLabel = "Undo", duration = SnackbarDuration.Short)
            if (r == SnackbarResult.ActionPerformed) legato.run { saveScratch(null, n.title, n.text) }
        }
    }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text("Scratchpad") }, scrollBehavior = scrollBehavior) },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { onOpen(-1L) },
                icon = { Icon(Icons.Default.Edit, contentDescription = null) },
                text = { Text("New note") },
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 96.dp),
        ) {
            if (notes.isEmpty()) item {
                Empty(Icons.AutoMirrored.Outlined.StickyNote2, "Notes you dictate or type here sync with the Scratchpad on your PC.")
            }
            itemsIndexed(notes, key = { _, n -> n.id }) { i, n ->
                val dismiss = rememberSwipeToDismissBoxState(
                    confirmValueChange = { value ->
                        if (value == SwipeToDismissBoxValue.EndToStart) { delete(n); true } else false
                    },
                )
                Column(Modifier.animateItem()) {
                    SwipeToDismissBox(
                        state = dismiss,
                        enableDismissFromStartToEnd = false,
                        backgroundContent = {
                            Box(
                                Modifier.fillMaxSize().background(MaterialTheme.colorScheme.errorContainer, RoundedCornerShape(20.dp)).padding(horizontal = 24.dp),
                                contentAlignment = Alignment.CenterEnd,
                            ) { Icon(Icons.Default.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.onErrorContainer) }
                        },
                    ) {
                        Card(first = i == 0, last = i == notes.lastIndex, inset = false) {
                            ListItem(
                                headlineContent = { Text(n.heading(), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                supportingContent = {
                                    val body = n.text.trim().replace(Regex("\\s+"), " ")
                                    Text(listOf(ago(n.updatedAt), body).filter { it.isNotBlank() }.joinToString(" · "), maxLines = 2, overflow = TextOverflow.Ellipsis, color = LocalStatus.current.faint)
                                },
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
        MenuSheet(
            title = n.heading(),
            subtitle = ago(n.updatedAt),
            actions = listOf(
                MenuAction("Open", Icons.Default.Edit) { onOpen(n.id) },
                MenuAction("Copy", Icons.Default.ContentCopy) { copy(context, n.text); scope.launch { snackbar.showSnackbar("Copied") } },
                MenuAction("Share", Icons.Default.Share) { share(context, n.text) },
                MenuAction("Delete", Icons.Default.Delete, danger = true) { delete(n) },
            ),
            onDismiss = { menuFor = null },
        )
    }
}

/** Saves one note in order, so a quick second save can't make a second note. */
private class NoteSaver(private val legato: Legato, var id: Long?) {
    private val lock = Mutex()
    private var last: Pair<String, String>? = null

    fun save(title: String, text: String) {
        legato.scope.launch {
            lock.withLock {
                if (last == title to text) return@withLock
                if (id == null && title.isBlank() && text.isBlank()) return@withLock
                try {
                    id = legato.core.saveScratch(id, title, text)
                    last = title to text
                    legato.bump()
                } catch (e: Exception) {
                    legato.notice(e.message ?: "Couldn't save the note")
                }
            }
        }
    }

    fun loaded(title: String, text: String) {
        last = title to text
    }
}

/** One Scratchpad note: a title, the text, and the mic to dictate at the cursor. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScratchEditScreen(id: Long, onBack: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val model by legato.model.collectAsStateWithLifecycle()
    val saver = remember { NoteSaver(legato, id.takeIf { it > 0 }) }
    var title by remember { mutableStateOf("") }
    var body by remember { mutableStateOf(TextFieldValue("")) }
    var loaded by remember { mutableStateOf(id <= 0) }
    // The dictation service holds the microphone, so talking goes on with
    // Legato in the background or the notification shade open.
    val dictation by Dictation.state.collectAsStateWithLifecycle()
    var mine by remember { mutableStateOf(Dictation.target == Dictation.Target.Screen && dictation !is Dictation.State.Idle) }
    val recording = mine && dictation is Dictation.State.Listening
    val busy = mine && dictation is Dictation.State.Transcribing
    val level = (dictation as? Dictation.State.Listening)?.level?.takeIf { mine } ?: 0f
    val heard by Dictation.forScreen.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    LaunchedEffect(id) {
        if (id > 0) {
            val note = withContext(Dispatchers.Default) { runCatching { legato.core.scratchNotes().find { it.id == id } }.getOrNull() }
            if (note != null) {
                title = note.title
                body = TextFieldValue(note.text, TextRange(note.text.length))
                saver.loaded(note.title, note.text)
            }
            loaded = true
        }
    }
    // Saves a moment after you stop typing, and once more on the way out.
    LaunchedEffect(title, body.text, loaded) {
        if (!loaded) return@LaunchedEffect
        delay(600)
        saver.save(title, body.text)
    }
    val latest by androidx.compose.runtime.rememberUpdatedState(title to body.text)
    DisposableEffect(Unit) {
        onDispose {
            if (loaded) saver.save(latest.first, latest.second)
        }
    }

    fun insert(words: String) {
        val t = words.trim()
        if (t.isEmpty()) return
        val v = body
        val start = minOf(v.selection.start, v.selection.end)
        val end = maxOf(v.selection.start, v.selection.end)
        val before = v.text.substring(0, start)
        val space = if (before.isNotEmpty() && !before.last().isWhitespace()) " " else ""
        val piece = space + t
        body = TextFieldValue(before + piece + v.text.substring(end), TextRange(start + piece.length))
    }

    // Words that came while the note loaded, or Legato was in the background, go in once it can take them.
    LaunchedEffect(heard, loaded) {
        val words = heard ?: return@LaunchedEffect
        if (!loaded || !mine) return@LaunchedEffect
        Dictation.forScreen.value = null
        insert(words)
        if (dictation is Dictation.State.Idle) mine = false
    }
    LaunchedEffect(dictation) { if (dictation is Dictation.State.Idle && heard == null) mine = false }

    fun listen() {
        Dictation.forScreen.value = null
        mine = true
        Dictation.start(context, Dictation.Target.Screen)
    }

    val mic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) listen()
    }

    fun talk() {
        when {
            recording -> Dictation.finish(context)
            busy -> Unit
            dictation !is Dictation.State.Idle -> scope.launch { snackbar.showSnackbar("Finish the dictation under way first.") }
            !legato.canTranscribe() && !legato.pcMayTranscribe() -> scope.launch {
                snackbar.showSnackbar(
                    when (model) {
                        is ModelStatus.Downloading -> "The speech model is still downloading."
                        ModelStatus.Downloaded -> "The speech model is loading; try again in a moment."
                        else -> "Download the speech model in Settings, or connect to your PC."
                    },
                )
            }
            micGranted(context) -> listen()
            else -> mic.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    val fieldColors = TextFieldDefaults.colors(
        focusedContainerColor = Color.Transparent,
        unfocusedContainerColor = Color.Transparent,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
    )

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
                actions = {
                    // The selection if there is one, else the whole note; Replace puts the result there.
                    IconButton(
                        onClick = {
                            val sel = body.selection
                            val (start, end) = if (sel.collapsed) 0 to body.text.length else minOf(sel.start, sel.end) to maxOf(sel.start, sel.end)
                            TransformSheet.open(body.text.substring(start, end)) { out ->
                                val text = body.text.substring(0, start) + out + body.text.substring(end)
                                body = TextFieldValue(text, TextRange(start + out.length))
                            }
                        },
                        enabled = body.text.isNotBlank(),
                    ) {
                        Icon(Icons.Default.SwapHoriz, contentDescription = "Transform")
                    }
                    IconButton(onClick = { copy(context, body.text); scope.launch { snackbar.showSnackbar("Copied") } }, enabled = body.text.isNotBlank()) {
                        Icon(Icons.Default.ContentCopy, contentDescription = "Copy")
                    }
                    IconButton(onClick = { share(context, body.text) }, enabled = body.text.isNotBlank()) {
                        Icon(Icons.Default.Share, contentDescription = "Share")
                    }
                },
            )
        },
        floatingActionButton = {
            val width by animateDpAsState(if (recording) 220.dp else 56.dp, expressiveSpring(), label = "mic")
            FloatingActionButton(
                onClick = { talk() },
                containerColor = if (recording) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.primaryContainer,
                modifier = Modifier.width(width),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 16.dp)) {
                    when {
                        busy -> CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.5.dp)
                        recording -> Icon(Icons.Default.Stop, contentDescription = "Finish dictating")
                        else -> Icon(Icons.Default.Mic, contentDescription = "Dictate")
                    }
                    AnimatedVisibility(recording, enter = fadeIn() + scaleIn(), exit = fadeOut() + scaleOut()) {
                        WaveMeter(level, Modifier.padding(start = 12.dp).width(140.dp).height(32.dp), color = MaterialTheme.colorScheme.onPrimary)
                    }
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).imePadding().padding(horizontal = 4.dp)) {
            TextField(
                value = title, onValueChange = { title = it }, singleLine = true,
                placeholder = { Text("Title") },
                textStyle = MaterialTheme.typography.headlineSmall,
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth(),
            )
            TextField(
                value = body, onValueChange = { body = it },
                placeholder = { Text("Type, or tap the mic and talk.") },
                textStyle = MaterialTheme.typography.bodyLarge,
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth().weight(1f),
            )
            Surface(color = Color.Transparent, modifier = Modifier.height(72.dp)) {}
        }
    }
    TransformSheet.Host()
}
