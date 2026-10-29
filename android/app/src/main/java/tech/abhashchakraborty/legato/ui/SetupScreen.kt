package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tech.abhashchakraborty.legato.ModelStatus
import kotlin.math.PI
import kotlin.math.sin

/**
 * First run, one step at a time: what Legato does, the speech model, the
 * microphone and the bubble's switches, the PC if there is one, and how a
 * dictation goes. Every step can wait; Settings has them all again.
 */
@Composable
fun SetupScreen(onDone: () -> Unit, onPair: () -> Unit) {
    val legato = rememberLegato()
    val state by legato.state.collectAsStateWithLifecycle()
    val model by legato.model.collectAsStateWithLifecycle()
    var step by rememberSaveable { mutableIntStateOf(0) }
    val last = STEPS.lastIndex

    fun finish() {
        legato.prefs.edit().putBoolean("setup_done", true).apply()
        onDone()
    }

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).statusBarsPadding().navigationBarsPadding()) {
        // Where you are: one segment per step, the ones done filled in.
        Row(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            STEPS.indices.forEach { i ->
                Box(
                    Modifier.weight(1f).height(4.dp).background(
                        if (i <= step) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHighest,
                        RoundedCornerShape(2.dp),
                    ),
                )
            }
        }
        AnimatedContent(
            targetState = step,
            transitionSpec = {
                val dir = if (targetState > initialState) 1 else -1
                (fadeIn(tween(220)) + slideInHorizontally(tween(220)) { dir * it / 10 }) togetherWith fadeOut(tween(160))
            },
            modifier = Modifier.weight(1f),
            label = "step",
        ) { s ->
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
                Text(STEPS[s].title, style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(horizontal = 8.dp))
                Spacer(Modifier.height(8.dp))
                Text(STEPS[s].body(legato.device), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 8.dp))
                Spacer(Modifier.height(20.dp))
                when (s) {
                    0 -> BarPicture(Modifier.align(Alignment.CenterHorizontally))
                    1 -> Group { ModelRow(legato) }
                    2 -> Group { BubbleRows() }
                    3 -> Group {
                        ListItem(
                            headlineContent = { Text(if (state.paired) "Paired with ${state.pcName ?: "your PC"}" else "Pair with your PC") },
                            supportingContent = {
                                Text(if (state.paired) "Your words, notes and history sync, and its speech model writes up what you say." else "Scan the code in Legato › Settings › Devices.")
                            },
                            leadingContent = { Icon(Icons.Default.Computer, contentDescription = null) },
                            trailingContent = { if (!state.paired) Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null) },
                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                            modifier = Modifier.clickable(enabled = !state.paired, onClick = onPair),
                        )
                    }
                    else -> TryIt()
                }
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (step == 0) TextButton(onClick = { finish() }) { Text("Skip") }
            else TextButton(onClick = { step-- }) { Text("Back") }
            Spacer(Modifier.weight(1f))
            // Something has to write up speech before the guide can end: this device's model or the PC's.
            val ready = model !is ModelStatus.Missing || state.paired
            Button(onClick = { if (step < last) step++ else finish() }, enabled = step < last || ready) {
                Text(if (step < last) "Next" else "Start dictating")
            }
        }
    }
}

private class Step(val title: String, val body: (device: String) -> String)

private val STEPS = listOf(
    Step("Talk instead of typing") { device -> "Whenever the keyboard is open, a small bubble sits beside it. Tap it, say what you'd type, and tap the check. Your words land in the field, on this $device or written up by your PC." },
    Step("A speech model") { device -> "It turns your voice into text right on this $device, with no internet. Paired with your PC, the PC's model can do it instead." },
    Step("Microphone and bubble") { _ -> "Legato hears you only while you dictate. Accessibility lets the bubble see the keyboard open and type for you." },
    Step("Your PC, if you have one") { _ -> "Your Dictionary, snippets, notes and history stay in step, and copying on one shows up on the other." },
    Step("How a dictation goes") { _ -> "Three moves, in any app with a text field." },
)

/** The bubble opening into the bar, as it looks over the keyboard. */
@Composable
private fun BarPicture(modifier: Modifier = Modifier) {
    val wave = rememberInfiniteTransition(label = "wave")
    val phase by wave.animateFloat(0f, 2f * PI.toFloat(), infiniteRepeatable(tween(1400, easing = LinearEasing)), label = "phase")
    val level by wave.animateFloat(0.3f, 1f, infiniteRepeatable(tween(900, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "level")
    val accent = MaterialTheme.colorScheme.primary
    Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = modifier) {
        Row(Modifier.padding(6.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(36.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape), contentAlignment = Alignment.Center) {
                Icon(Icons.Default.Close, contentDescription = null, modifier = Modifier.size(18.dp))
            }
            Canvas(Modifier.width(96.dp).height(24.dp).padding(horizontal = 8.dp)) {
                val mid = size.height / 2
                var x = 0f
                var prev = Offset(0f, mid)
                while (x <= size.width) {
                    val y = mid + sin(x / 9f + phase) * (mid - 2f) * level
                    val p = Offset(x, y)
                    if (x > 0f) drawLine(accent, prev, p, strokeWidth = 2.5.dp.toPx(), cap = StrokeCap.Round)
                    prev = p
                    x += 2f
                }
            }
            Box(Modifier.size(36.dp).background(accent, CircleShape), contentAlignment = Alignment.Center) {
                Icon(Icons.Default.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(18.dp))
            }
        }
    }
}

/** The three moves of a dictation, each with the part of the bar it uses. */
@Composable
private fun TryIt() {
    val accent = MaterialTheme.colorScheme.primary
    Group {
        Move("1", "Tap a text field", "The bubble shows beside the keyboard.") {
            Box(Modifier.size(28.dp).background(accent.copy(alpha = 0.72f), CircleShape))
        }
        Hairline()
        Move("2", "Tap the bubble and talk", "The line moves with your voice. ✕ stops, and keeps what you said in Home.") {
            Canvas(Modifier.size(28.dp)) {
                val mid = size.height / 2
                var prev = Offset(0f, mid)
                var x = 2f
                while (x <= size.width) {
                    val p = Offset(x, mid + sin(x / 3f) * mid * 0.5f)
                    drawLine(accent, prev, p, strokeWidth = 2.5.dp.toPx(), cap = StrokeCap.Round)
                    prev = p
                    x += 2f
                }
            }
        }
        Hairline()
        Move("3", "Tap the check", "A spinner while it's written up, then the words are in the field.") {
            Box(Modifier.size(28.dp).background(accent, CircleShape), contentAlignment = Alignment.Center) {
                Icon(Icons.Default.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(16.dp))
            }
        }
    }
}

@Composable
private fun Move(n: String, title: String, body: String, mark: @Composable () -> Unit) {
    ListItem(
        overlineContent = { Text("Step $n") },
        headlineContent = { Text(title) },
        supportingContent = { Text(body, color = LocalStatus.current.faint) },
        leadingContent = { Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) { mark() } },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
    )
}
