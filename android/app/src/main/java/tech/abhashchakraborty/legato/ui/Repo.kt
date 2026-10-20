package tech.abhashchakraborty.legato.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.core.Core
import tech.abhashchakraborty.legato.legato

/**
 * Reads a list from the core off the main thread and reads it again whenever
 * anything changed (a sync, an edit here). Screens stay dumb.
 */
@Composable
fun <T> coreList(read: Core.() -> List<T>): State<List<T>> {
    val legato = LocalContext.current.legato
    val version by legato.version.collectAsStateWithLifecycle()
    val state = remember { mutableStateOf(emptyList<T>()) }
    LaunchedEffect(version) {
        state.value = runCatching { withContext(Dispatchers.Default) { legato.core.read() } }.getOrDefault(state.value)
    }
    return state
}

@Composable
fun rememberLegato(): Legato = LocalContext.current.legato
