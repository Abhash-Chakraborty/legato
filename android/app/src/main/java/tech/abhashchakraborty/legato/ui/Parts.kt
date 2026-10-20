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
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Circle
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import tech.abhashchakraborty.legato.connectedText
import tech.abhashchakraborty.legato.core.PhoneState
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
        if (done) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
        plainSpring(),
        label = "done",
    )
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(detail, color = LocalStatus.current.faint) },
        leadingContent = {
            Icon(
                if (done) Icons.Outlined.CheckCircle else Icons.Outlined.Circle,
                contentDescription = if (done) "Done" else "Not yet",
                tint = tint,
                modifier = Modifier.size(22.dp),
            )
        },
        trailingContent = { if (!done) TextButton(onClick = onOpen) { Text(action) } },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
    )
}

/** A titled group of rows on one rounded surface, with hairlines between them. */
@Composable
fun Group(title: String? = null, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        if (title != null) DayHeader(title)
        Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
            Column { content() }
        }
    }
}

/** The hairline between rows in a group. */
@Composable
fun Hairline() {
    HorizontalDivider(Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
}

/** How the link to the PC stands, as a status colour and words. */
data class PcStatus(val color: Color, val text: String)

/**
 * Green while connected (and how), amber while the PC can't be reached and
 * Legato keeps trying, grey before pairing. Never the accent colour.
 */
@Composable
fun pcStatus(state: PhoneState, device: String, long: Boolean = false): PcStatus {
    val s = LocalStatus.current
    val synced = state.lastSync?.let { ago(it) }
    return when {
        !state.paired -> PcStatus(s.faint, "Not paired")
        state.online -> PcStatus(
            s.ok,
            if (long) state.connectedText + (synced?.let { " · synced $it" } ?: "")
            else "${state.pcName} · synced ${synced ?: "just now"}",
        )
        else -> PcStatus(
            s.warn,
            if (long) "Can't reach it right now · changes wait on this $device" + (synced?.let { " · last synced $it" } ?: "")
            else "${state.pcName} not reachable · changes wait here",
        )
    }
}

@Composable
fun StatusDot(color: Color, modifier: Modifier = Modifier) {
    Box(modifier.size(8.dp).background(color, CircleShape))
}

/** The search pill: a filled field with no outline, as in the mockup. */
@Composable
fun SearchField(value: String, onValueChange: (String) -> Unit, placeholder: String, modifier: Modifier = Modifier) {
    TextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        placeholder = { Text(placeholder) },
        leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
        trailingIcon = {
            if (value.isNotEmpty()) IconButton(onClick = { onValueChange("") }) { Icon(Icons.Default.Close, contentDescription = "Clear search") }
        },
        shape = RoundedCornerShape(25.dp),
        colors = TextFieldDefaults.colors(
            focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
            focusedLeadingIconColor = MaterialTheme.colorScheme.onSurfaceVariant,
            unfocusedLeadingIconColor = LocalStatus.current.faint,
            unfocusedPlaceholderColor = LocalStatus.current.faint,
            focusedPlaceholderColor = LocalStatus.current.faint,
        ),
        modifier = modifier.fillMaxWidth().height(52.dp),
    )
}

/** One choice in a row's menu sheet. */
data class MenuAction(val label: String, val icon: ImageVector, val danger: Boolean = false, val run: () -> Unit)

/**
 * A row's menu as a bottom sheet: a title, then its actions. Tap and
 * long-press on a row open the same one, so delete is never a single tap.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MenuSheet(title: String, subtitle: String? = null, actions: List<MenuAction>, onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
            Text(title, style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 4.dp))
            if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint, modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 2.dp))
            Spacer(Modifier.height(8.dp))
            actions.forEach { a ->
                val tint = if (a.danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 56.dp).clip(RoundedCornerShape(14.dp)).clickable { onDismiss(); a.run() }.padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Icon(a.icon, contentDescription = null, tint = tint)
                    Text(a.label, style = MaterialTheme.typography.bodyLarge, color = if (a.danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
                }
            }
        }
    }
}

/** A settings row: title, one line of help, and whatever sits at the end. */
@Composable
fun SettingRow(
    title: String,
    detail: String? = null,
    enabled: Boolean = true,
    onClick: (() -> Unit)? = null,
    trailing: @Composable (() -> Unit)? = null,
) {
    val alpha = if (enabled) 1f else 0.5f
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp)
            .then(if (onClick != null) Modifier.clickable(enabled = enabled, onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface.copy(alpha = alpha))
            if (detail != null) Text(detail, style = MaterialTheme.typography.bodyMedium, color = LocalStatus.current.faint.copy(alpha = alpha), modifier = Modifier.padding(top = 2.dp))
        }
        trailing?.invoke()
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
