package tech.abhashchakraborty.legato.ui

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import kotlin.math.PI
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * A wavy line that swells with your voice: the meter while Legato listens.
 * The wave keeps moving at rest so it reads as "listening", and its height
 * follows [level] (0..1) with a springy ease.
 */
@Composable
fun WaveMeter(level: Float, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.primary) {
    val phase by rememberInfiniteTransition(label = "wave").animateFloat(
        initialValue = 0f,
        targetValue = (2 * PI).toFloat(),
        animationSpec = infiniteRepeatable(tween(1400, easing = LinearEasing)),
        label = "phase",
    )
    // Speech peaks sit low on the scale; the square root lifts them.
    val amp by animateFloatAsState(sqrt(level.coerceIn(0f, 1f)).coerceAtMost(1f), expressiveSpring(), label = "amp")
    Canvas(modifier) {
        val mid = size.height / 2
        val height = (0.12f + amp * 0.88f) * mid * 0.9f
        val path = Path().apply { moveTo(0f, mid) }
        var x = 0f
        while (x <= size.width) {
            val t = x / size.width
            // Pinned at both ends so the wave grows from the middle.
            val envelope = sin(PI * t).toFloat()
            path.lineTo(x, mid + height * envelope * sin(2 * PI * 2.5 * t + phase).toFloat())
            x += 3f
        }
        drawPath(path, color, style = Stroke(width = 3.dp.toPx(), cap = StrokeCap.Round))
    }
}

/** A row in a setup list: what it is, why, and Open until it's done. */
@Composable
fun PermissionRow(title: String, detail: String, done: Boolean, action: String = "Open", onOpen: () -> Unit) {
    val tint by animateColorAsState(
        if (done) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh,
        plainSpring(),
        label = "done",
    )
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(detail) },
        leadingContent = {
            Box(Modifier.size(28.dp).background(tint, CircleShape), contentAlignment = Alignment.Center) {
                if (done) Icon(Icons.Default.Check, contentDescription = "Done", tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(16.dp))
            }
        },
        trailingContent = { if (!done) TextButton(onClick = onOpen) { Text(action) } },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
    )
}

/** A titled group of rows on one rounded surface. */
@Composable
fun Group(title: String? = null, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        if (title != null) DayHeader(title)
        Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.padding(vertical = 4.dp)) { content() }
        }
    }
}

fun micGranted(context: Context): Boolean =
    context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

fun notificationsGranted(context: Context): Boolean =
    Build.VERSION.SDK_INT < 33 ||
        context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

fun openAccessibilitySettings(context: Context) {
    context.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

fun openOverlaySettings(context: Context) {
    context.startActivity(
        Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
    )
}

fun minutes(ms: Long): String {
    val m = ms / 60_000
    return if (m < 1) "under a minute" else if (m < 60) "$m min" else "${m / 60} h ${m % 60} min"
}
