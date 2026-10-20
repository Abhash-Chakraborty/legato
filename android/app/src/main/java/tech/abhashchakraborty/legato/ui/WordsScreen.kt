package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.foundation.combinedClickable
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import tech.abhashchakraborty.legato.core.SnippetRow
import tech.abhashchakraborty.legato.core.WordRow

/** The Dictionary and snippets, shared with the PC. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun WordsScreen() {
    val legato = rememberLegato()
    val words by coreList { wordRows() }
    val snippets by coreList { snippetRows() }
    var tab by rememberSaveable { mutableStateOf(0) }
    var query by rememberSaveable { mutableStateOf("") }
    var editing by remember { mutableStateOf<SnippetRow?>(null) }
    var wordMenu by remember { mutableStateOf<WordRow?>(null) }
    var snippetMenu by remember { mutableStateOf<SnippetRow?>(null) }
    val context = androidx.compose.ui.platform.LocalContext.current
    var newSnippet by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())

    fun removeWord(w: WordRow) {
        legato.run { removeWord(w.text) }
        scope.launch {
            val r = snackbar.showSnackbar("Removed ${w.text}", actionLabel = "Undo", duration = SnackbarDuration.Short)
            if (r == SnackbarResult.ActionPerformed) legato.run {
                if (w.heard.isEmpty()) addWordHeard(w.text, "") else w.heard.forEach { addWordHeard(w.text, it) }
            }
        }
    }

    fun removeSnippet(s: SnippetRow) {
        legato.run { removeSnippet(s.id) }
        scope.launch {
            val r = snackbar.showSnackbar("Removed “${s.trigger}”", actionLabel = "Undo", duration = SnackbarDuration.Short)
            if (r == SnackbarResult.ActionPerformed) legato.run { saveSnippet(null, s.trigger, s.text) }
        }
    }

    val q = query.trim()
    val shownWords = if (q.isEmpty()) words else words.filter { w ->
        w.text.contains(q, ignoreCase = true) || w.heard.any { it.contains(q, ignoreCase = true) }
    }
    val shownSnippets = if (q.isEmpty()) snippets else snippets.filter {
        it.trigger.contains(q, ignoreCase = true) || it.text.contains(q, ignoreCase = true)
    }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = { LargeTopAppBar(title = { Text(if (tab == 0) "Dictionary" else "Snippets") }, scrollBehavior = scrollBehavior) },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { if (tab == 0) AddWordSheet.open("") else newSnippet = true },
                icon = { Icon(Icons.Default.Add, contentDescription = null) },
                text = { Text(if (tab == 0) "Add word" else "Add snippet") },
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 96.dp),
        ) {
            item {
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    listOf("Words · ${words.size}", "Snippets · ${snippets.size}").forEachIndexed { i, label ->
                        SegmentedButton(
                            selected = tab == i,
                            onClick = { tab = i },
                            shape = SegmentedButtonDefaults.itemShape(i, 2),
                        ) { Text(label) }
                    }
                }
                Spacer(Modifier.height(14.dp))
                SearchField(query, { query = it }, if (tab == 0) "Search words" else "Search snippets")
                Spacer(Modifier.height(14.dp))
            }
            if (tab == 0) {
                if (shownWords.isEmpty()) item {
                    Empty(
                        if (words.isEmpty()) Icons.AutoMirrored.Outlined.MenuBook else Icons.Default.Search,
                        if (words.isEmpty()) "Names and terms you add here are spelled right in every dictation." else "No word matches “$q”.",
                    )
                }
                itemsIndexed(shownWords, key = { _, w -> "w${w.id}" }) { i, w ->
                    Column(Modifier.animateItem()) {
                        Card(first = i == 0, last = i == shownWords.lastIndex, inset = false) {
                            ListItem(
                                headlineContent = { Text(w.text) },
                                supportingContent = {
                                    Text(
                                        if (w.heard.isEmpty()) "Spelled as written" else "Heard as ${w.heard.joinToString(", ") { "“$it”" }}",
                                        maxLines = 2, overflow = TextOverflow.Ellipsis, color = LocalStatus.current.faint,
                                    )
                                },
                                trailingContent = if (w.pinned) {
                                    { Icon(Icons.Default.PushPin, contentDescription = "Pinned", tint = LocalStatus.current.faint, modifier = Modifier.size(16.dp)) }
                                } else null,
                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                modifier = Modifier.combinedClickable(onClick = { wordMenu = w }, onLongClick = { wordMenu = w }),
                            )
                        }
                    }
                }
            } else {
                if (shownSnippets.isEmpty()) item {
                    Empty(
                        if (snippets.isEmpty()) Icons.Default.Add else Icons.Default.Search,
                        if (snippets.isEmpty()) "Say a trigger phrase and Legato types the saved text." else "No snippet matches “$q”.",
                    )
                }
                itemsIndexed(shownSnippets, key = { _, s -> "s${s.id}" }) { i, s ->
                    Column(Modifier.animateItem()) {
                        Card(first = i == 0, last = i == shownSnippets.lastIndex, inset = false) {
                            ListItem(
                                headlineContent = { Text("“${s.trigger}”") },
                                supportingContent = { Text(s.text, maxLines = 2, overflow = TextOverflow.Ellipsis, color = LocalStatus.current.faint) },
                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                modifier = Modifier.combinedClickable(onClick = { snippetMenu = s }, onLongClick = { snippetMenu = s }),
                            )
                        }
                    }
                }
            }
        }
    }

    wordMenu?.let { w ->
        MenuSheet(
            title = w.text,
            subtitle = if (w.heard.isEmpty()) "Spelled as written" else "Heard as ${w.heard.joinToString(", ") { "“$it”" }}",
            actions = listOf(
                MenuAction("Add what it's heard as", Icons.Default.Add) { AddWordSheet.open("", word = w.text) },
                MenuAction("Copy", Icons.Default.ContentCopy) { copy(context, w.text); scope.launch { snackbar.showSnackbar("Copied") } },
                MenuAction("Remove from Dictionary", Icons.Outlined.Delete, danger = true) { removeWord(w) },
            ),
            onDismiss = { wordMenu = null },
        )
    }
    snippetMenu?.let { sn ->
        MenuSheet(
            title = "“${sn.trigger}”",
            subtitle = sn.text,
            actions = listOf(
                MenuAction("Edit", Icons.Default.Edit) { editing = sn },
                MenuAction("Copy the text", Icons.Default.ContentCopy) { copy(context, sn.text); scope.launch { snackbar.showSnackbar("Copied") } },
                MenuAction("Remove snippet", Icons.Outlined.Delete, danger = true) { removeSnippet(sn) },
            ),
            onDismiss = { snippetMenu = null },
        )
    }

    if (newSnippet || editing != null) {
        SnippetSheet(editing, onDismiss = { newSnippet = false; editing = null })
    }
    AddWordSheet.Host()
}

/** Adds a snippet or edits one: the phrase you say and what Legato types. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SnippetSheet(snippet: SnippetRow?, onDismiss: () -> Unit) {
    val legato = rememberLegato()
    var trigger by remember { mutableStateOf(snippet?.trigger ?: "") }
    var text by remember { mutableStateOf(snippet?.text ?: "") }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 24.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(if (snippet == null) "New snippet" else "Edit snippet", style = MaterialTheme.typography.titleLarge)
            OutlinedTextField(
                trigger, { trigger = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                label = { Text("When you say") },
                supportingText = { Text("A short phrase, like “my address”.") },
            )
            OutlinedTextField(
                text, { text = it }, minLines = 3, modifier = Modifier.fillMaxWidth(),
                label = { Text("Legato types") },
            )
            Button(
                onClick = {
                    val (t, x) = trigger to text
                    legato.run { saveSnippet(snippet?.id, t, x) }
                    onDismiss()
                },
                enabled = trigger.isNotBlank() && text.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Save") }
        }
    }
}

/**
 * Adds a word to the Dictionary, optionally with the form the speech model
 * writes instead. Opened from Home's "Add a word from this" with the
 * dictation's words as chips, so a mishearing is one tap away.
 */
object AddWordSheet {
    private var source by mutableStateOf<String?>(null)
    private var preset by mutableStateOf("")

    /** Opens the sheet with [text]'s words as chips, and [word] already filled in. */
    fun open(text: String, word: String = "") {
        preset = word
        source = text
    }

    @OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
    @Composable
    fun Host() {
        val from = source ?: return
        val legato = rememberLegato()
        var word by remember(from, preset) { mutableStateOf(preset) }
        var heard by remember(from) { mutableStateOf("") }
        val tokens = remember(from) {
            from.split(Regex("\\s+")).map { it.trim { c -> !c.isLetterOrDigit() } }.filter { it.length > 1 }.distinct().take(16)
        }
        val picked = heard.split(' ').filter { it.isNotBlank() }
        ModalBottomSheet(onDismissRequest = { source = null }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
            Column(Modifier.padding(horizontal = 24.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(if (preset.isEmpty()) "Add to Dictionary" else "How Legato hears $preset", style = MaterialTheme.typography.titleLarge)
                if (tokens.isNotEmpty()) {
                    Text("Tap the words Legato got wrong.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        tokens.forEach { t ->
                            val on = picked.any { it.equals(t, ignoreCase = true) }
                            FilterChip(
                                selected = on,
                                onClick = {
                                    heard = if (on) picked.filterNot { it.equals(t, ignoreCase = true) }.joinToString(" ")
                                    else (picked + t.lowercase()).joinToString(" ")
                                },
                                label = { Text(t) },
                            )
                        }
                    }
                }
                AnimatedContent(targetState = tokens.isEmpty() || heard.isNotBlank() || preset.isNotEmpty(), label = "fields") { show ->
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        OutlinedTextField(
                            word, { word = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                            label = { Text("Word, spelled right") },
                        )
                        if (show) OutlinedTextField(
                            heard, { heard = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                            label = { Text("Heard as (optional)") },
                            supportingText = { Text("What the model writes instead, like “net bird”.") },
                        )
                    }
                }
                Button(
                    onClick = {
                        val (w, h) = word to heard
                        legato.run { addWordHeard(w, h) }
                        source = null
                    },
                    enabled = word.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Add") }
                TextButton(onClick = { source = null }, modifier = Modifier.fillMaxWidth()) { Text("Cancel") }
            }
        }
    }
}
