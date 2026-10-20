package tech.abhashchakraborty.legato.ui

import android.os.Build
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/**
 * Material 3 with the phone's own colours where Android offers them
 * (Android 12 and later), Legato's teal otherwise. Motion follows Material 3
 * Expressive: springy for things that move, plain for colour.
 */
@Composable
fun LegatoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val context = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        dark -> darkColorScheme(
            primary = Color(0xFF66B8AD),
            onPrimary = Color(0xFF0B2F2C),
            primaryContainer = Color(0xFF1D6F6A),
            onPrimaryContainer = Color(0xFFCEEDEA),
            surface = Color(0xFF151615),
            surfaceContainer = Color(0xFF1D1E1D),
            surfaceContainerHigh = Color(0xFF242624),
        )
        else -> lightColorScheme(
            primary = Color(0xFF1A5C5C),
            onPrimary = Color.White,
            primaryContainer = Color(0xFFCEEDEA),
            onPrimaryContainer = Color(0xFF034F46),
            surface = Color(0xFFF3F2EE),
            surfaceContainer = Color(0xFFFCFCFB),
            surfaceContainerHigh = Color(0xFFF5F4F0),
        )
    }
    MaterialTheme(colorScheme = scheme, content = content)
}

/** Spatial motion: a little overshoot, as Material 3 Expressive does. */
fun <T> expressiveSpring() = spring<T>(dampingRatio = 0.7f, stiffness = Spring.StiffnessMediumLow)

/** Motion for colour and opacity: no overshoot. */
fun <T> plainSpring() = spring<T>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)
