package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.core.TransformItem

/**
 * Your PC's transforms on a piece of text in Legato: a dictation in Home, a
 * Scratchpad note, a note's transcript. Runs here with the AI key the PC
 * shared, else on the PC; the result can be copied, or take the text's
 * place where that makes sense.
 */
object TransformSheet {
    private class Request(val text: String, val onReplace: ((String) -> Unit)?)

    private var request by mutableStateOf<Request?>(null)

    /** Opens the transforms for [text]; with [onReplace], Replace puts the result in its place. */
    fun open(text: String, onReplace: ((String) -> Unit)? = null) {
        request = Request(text, onReplace)
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    fun Host() {
        val req = request ?: return
        val legato = rememberLegato()
        val context = LocalContext.current
        val scope = rememberCoroutineScope()
        var transforms by remember(req) { mutableStateOf<List<TransformItem>?>(null) }
        var running by remember(req) { mutableStateOf<String?>(null) }
        var result by remember(req) { mutableStateOf<Pair<String, String>?>(null) }
        var problem by remember(req) { mutableStateOf<String?>(null) }
        LaunchedEffect(req) {
            transforms = withContext(Dispatchers.IO) {
                if (legato.core.pcTransforms()) runCatching { legato.core.pullAi() }
                runCatching { legato.core.transforms() }.getOrDefault(emptyList())
            }
        }
        fun close() { request = null }
        fun run(t: TransformItem) {
            running = t.name
            problem = null
            scope.launch {
                val out = withContext(Dispatchers.IO) { runCatching { legato.transform(t.id, req.text) } }
                running = null
                out.onSuccess { text ->
                    if (text == null || text == req.text) problem = "${t.name} had nothing to change."
                    else result = t.name to text
                }
                out.onFailure { e -> problem = e.message ?: "${t.name} didn't run" }
            }
        }

        ModalBottomSheet(
            onDismissRequest = ::close,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = MaterialTheme.colorScheme.surfaceContainer,
        ) {
            Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
                val state = when {
                    result != null -> "result"
                    running != null -> "running"
                    else -> "list"
                }
                AnimatedContent(targetState = state, label = "transform") { s ->
                    Column {
                        when (s) {
                            "result" -> result?.let { (name, text) ->
                                Text(name, style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
                                Spacer(Modifier.height(12.dp))
                                Surface(shape = RoundedCornerShape(12.dp), color = MaterialTheme.colorScheme.surfaceContainerHighest) {
                                    Text(
                                        text,
                                        style = MaterialTheme.typography.bodyLarge,
                                        modifier = Modifier.fillMaxWidth().heightIn(max = 360.dp).verticalScroll(rememberScrollState()).padding(16.dp),
                                    )
                                }
                                Spacer(Modifier.height(16.dp))
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                    val replace = req.onReplace
                                    if (replace != null) Button(onClick = { replace(text); close() }) { Text("Replace") }
                                    OutlinedButton(onClick = { copy(context, text); close() }) { Text("Copy") }
                                    Spacer(Modifier.weight(1f))
                                    TextButton(onClick = { result = null }) { Text("Another") }
                                }
                            }
                            "running" -> Row(Modifier.padding(horizontal = 4.dp, vertical = 24.dp), verticalAlignment = Alignment.CenterVertically) {
                                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                                Spacer(Modifier.size(12.dp))
                                Text("${running ?: ""}…", style = MaterialTheme.typography.bodyLarge)
                            }
                            else -> {
                                Text("Transform", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 4.dp))
                                Text(
                                    req.text,
                                    maxLines = 2,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = LocalStatus.current.faint,
                                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                                    modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 2.dp),
                                )
                                Spacer(Modifier.height(8.dp))
                                val list = transforms
                                when {
                                    list == null -> CircularProgressIndicator(Modifier.padding(16.dp).size(20.dp), strokeWidth = 2.dp)
                                    list.isEmpty() -> Text(
                                        "No transforms yet. Switch some on in Legato on your PC; they arrive with the next sync.",
                                        style = MaterialTheme.typography.bodyMedium,
                                        modifier = Modifier.padding(4.dp),
                                    )
                                    else -> list.forEach { t ->
                                        ListItem(
                                            headlineContent = { Text(t.name) },
                                            supportingContent = t.description.takeIf { it.isNotBlank() }?.let { d -> { Text(d, maxLines = 2, color = LocalStatus.current.faint) } },
                                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                            modifier = Modifier.clickable { run(t) },
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
                problem?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(4.dp))
                }
            }
        }
    }
}
