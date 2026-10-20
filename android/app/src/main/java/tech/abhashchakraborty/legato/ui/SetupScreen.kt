package tech.abhashchakraborty.legato.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tech.abhashchakraborty.legato.ModelStatus

/**
 * First run: the speech model, the microphone and the bubble's switches,
 * then pairing with the PC if there is one. Every step can wait; Settings
 * has them all again.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SetupScreen(onDone: () -> Unit, onPair: () -> Unit) {
    val legato = rememberLegato()
    val state by legato.state.collectAsStateWithLifecycle()
    val model by legato.model.collectAsStateWithLifecycle()

    fun finish() {
        legato.prefs.edit().putBoolean("setup_done", true).apply()
        onDone()
    }

    Scaffold(
        topBar = { LargeTopAppBar(title = { Text("Set up Legato") }) },
        bottomBar = {
            Row(
                Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 24.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                TextButton(onClick = { finish() }, modifier = Modifier.weight(1f)) { Text("Later") }
                Button(onClick = { finish() }, modifier = Modifier.weight(1f), enabled = model !is ModelStatus.Missing || state.paired) { Text("Done") }
            }
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                "Legato turns your voice into text on this ${legato.device}, and keeps your words in step with Legato on your PC.",
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(horizontal = 8.dp),
            )
            Group("1 · Speech model") { ModelRow(legato) }
            Group("2 · Microphone and bubble") { BubbleRows() }
            Group("3 · Your PC, if you have one") {
                ListItem(
                    headlineContent = { Text(if (state.paired) "Paired with ${state.pcName ?: "your PC"}" else "Pair with your PC") },
                    supportingContent = {
                        Text(if (state.paired) "Your words, notes and history sync, and its speech model can write up what you say." else "Scan the code in Legato › Settings › Devices.")
                    },
                    leadingContent = { Icon(Icons.Default.Computer, contentDescription = null) },
                    trailingContent = { if (!state.paired) Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null) },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    modifier = Modifier.clickable(enabled = !state.paired, onClick = onPair),
                )
            }
        }
    }
}
