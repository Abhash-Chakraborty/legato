package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.core.coreVersion

/**
 * What Settings doesn't show: how the link to the PC stands in detail, and
 * the log of what the core did, newest first, to copy into a bug report.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AdvancedScreen(onBack: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val state by legato.state.collectAsStateWithLifecycle()
    val model by legato.model.collectAsStateWithLifecycle()
    val lines by legato.lines.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    val status = pcStatus(state, legato.device, long = true)
    val newest = lines.reversed()

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            LargeTopAppBar(
                title = { Text("Advanced") },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
                actions = {
                    IconButton(onClick = {
                        copy(context, lines.joinToString("\n"))
                        scope.launch { snackbar.showSnackbar("Log copied") }
                    }, enabled = lines.isNotEmpty()) { Icon(Icons.Default.ContentCopy, contentDescription = "Copy the log") }
                },
                scrollBehavior = scrollBehavior,
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp),
        ) {
            item {
                Group("Connection") {
                    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                        StatusDot(status.color)
                        Text(
                            if (state.paired) state.pcName ?: "PC" else "Not paired",
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.padding(start = 10.dp).weight(1f),
                        )
                        if (state.paired) OutlinedButton(onClick = { legato.run { connect() } }) { Text("Reconnect") }
                    }
                    Text(
                        if (state.paired) status.text else "Pair from Settings › Your PC.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = LocalStatus.current.faint,
                        modifier = Modifier.padding(start = 34.dp, end = 16.dp, bottom = 14.dp),
                    )
                    Hairline()
                    Detail("Reached over", when {
                        !state.online -> "Nothing right now"
                        state.via == "bluetooth" -> "Bluetooth"
                        state.via == "netbird" -> "Netbird"
                        else -> "Wi-Fi"
                    })
                    Hairline()
                    Detail("Writes up what you say", when {
                        state.online && state.pcTranscribes && legato.usePcModel -> "${state.pcName}'s speech model"
                        model is ModelStatus.Loaded || model == ModelStatus.Downloaded -> "The model on this ${legato.device}"
                        else -> "Nothing yet: download the model or connect to your PC"
                    })
                    Hairline()
                    Detail("Speech model", when (val m = model) {
                        ModelStatus.Missing -> "Not downloaded"
                        is ModelStatus.Downloading -> "Downloading"
                        ModelStatus.Downloaded -> "${legato.selectedModel.name}, downloaded"
                        is ModelStatus.Loaded -> "${legato.selectedModel.name}, loaded (${m.arch})"
                    })
                    Hairline()
                    Detail("Core", coreVersion())
                }
                DayHeader("Log")
            }
            if (newest.isEmpty()) item { Empty(Icons.AutoMirrored.Outlined.Notes, "Connections, syncs and clips show up here as they happen.") }
            itemsIndexed(newest) { i, line ->
                Card(first = i == 0, last = i == newest.lastIndex, inset = false) {
                    SelectionContainer {
                        Text(
                            line,
                            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace, fontSize = 12.5.sp, lineHeight = 18.sp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun Detail(label: String, value: String) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint)
        Text(value, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(top = 2.dp))
    }
}
