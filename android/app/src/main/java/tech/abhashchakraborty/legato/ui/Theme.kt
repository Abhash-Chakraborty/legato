package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import tech.abhashchakraborty.legato.R

/**
 * Legato's own look, the same tokens as the Windows app and the phone
 * mockup: warm neutrals, teal as the one accent, Figtree. The wallpaper's
 * colours (Android's dynamic colour) are left out on purpose, so Legato
 * looks like Legato on every phone.
 */
@Composable
fun LegatoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val scheme = if (dark) DarkScheme else LightScheme
    CompositionLocalProvider(LocalStatus provides if (dark) DarkStatus else LightStatus) {
        MaterialTheme(colorScheme = scheme, typography = LegatoType, content = content)
    }
}

private val LightScheme = lightColorScheme(
    primary = Color(0xFF1A5C5C), // teal-700: text buttons, links, day headers
    onPrimary = Color.White,
    primaryContainer = Color(0xFF1A5C5C), // teal-fill: the FAB and filled buttons
    onPrimaryContainer = Color.White,
    secondary = Color(0xFF247872),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFCEEDEA), // teal-100: the nav pill, picked segments
    onSecondaryContainer = Color(0xFF1A5C5C),
    tertiary = Color(0xFF247872),
    background = Color(0xFFF3F2EE),
    onBackground = Color(0xFF1A1A1A),
    surface = Color(0xFFF3F2EE),
    onSurface = Color(0xFF1A1A1A),
    onSurfaceVariant = Color(0xFF4A4845),
    surfaceVariant = Color(0xFFEDEBE5),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF5F4F0),
    surfaceContainer = Color(0xFFFCFCFB), // cards
    surfaceContainerHigh = Color(0xFFF5F4F0),
    surfaceContainerHighest = Color(0xFFEDEBE5), // search field
    outline = Color(0xFFD9D5CC),
    outlineVariant = Color(0xFFEBE8E1),
    error = Color(0xFFC7423D),
    onError = Color.White,
    inverseSurface = Color(0xFF2B2C2B),
    inverseOnSurface = Color(0xFFF1F0EC),
    inversePrimary = Color(0xFF95C7BB),
    scrim = Color(0xFF1A1A1A),
)

private val DarkScheme = darkColorScheme(
    primary = Color(0xFF66B8AD),
    onPrimary = Color(0xFF0B2F2C),
    primaryContainer = Color(0xFF1D6F6A),
    onPrimaryContainer = Color.White,
    secondary = Color(0xFF4FA499),
    onSecondary = Color(0xFF0B2F2C),
    secondaryContainer = Color(0xFF22433E),
    onSecondaryContainer = Color(0xFF66B8AD),
    tertiary = Color(0xFF4FA499),
    background = Color(0xFF151615),
    onBackground = Color(0xFFF1F0EC),
    surface = Color(0xFF151615),
    onSurface = Color(0xFFF1F0EC),
    onSurfaceVariant = Color(0xFFC7C5BF),
    surfaceVariant = Color(0xFF2C2E2C),
    surfaceContainerLowest = Color(0xFF101110),
    surfaceContainerLow = Color(0xFF1A1B1A),
    surfaceContainer = Color(0xFF1D1E1D),
    surfaceContainerHigh = Color(0xFF242624),
    surfaceContainerHighest = Color(0xFF2C2E2C),
    outline = Color(0xFF3A3D3A),
    outlineVariant = Color(0xFF2A2C2A),
    error = Color(0xFFE0605A),
    onError = Color.White,
    inverseSurface = Color(0xFF2B2C2B),
    inverseOnSurface = Color(0xFFF1F0EC),
    inversePrimary = Color(0xFF95C7BB), // the action on a toast, as in the mockup
    scrim = Color.Black,
)

/** Status colours, only for states: connected, trying, failed, and the quiet grey of meta text. */
@Immutable
data class StatusColors(val ok: Color, val warn: Color, val bad: Color, val faint: Color)

private val LightStatus = StatusColors(ok = Color(0xFF2F9E6E), warn = Color(0xFFC98A1A), bad = Color(0xFFC7423D), faint = Color(0xFF8A8580))
private val DarkStatus = StatusColors(ok = Color(0xFF3DB57F), warn = Color(0xFFD9A040), bad = Color(0xFFE0605A), faint = Color(0xFF8E8B85))

val LocalStatus = staticCompositionLocalOf { DarkStatus }

@OptIn(ExperimentalTextApi::class)
private fun figtree(weight: Int) = Font(
    R.font.figtree,
    weight = FontWeight(weight),
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

@OptIn(ExperimentalTextApi::class)
val Figtree = FontFamily(figtree(400), figtree(500), figtree(600), figtree(700))

private val LegatoType = Typography().let { t ->
    fun TextStyle.f() = copy(fontFamily = Figtree)
    Typography(
        displayLarge = t.displayLarge.f(),
        displayMedium = t.displayMedium.f(),
        displaySmall = t.displaySmall.f(),
        // The page title: 28 sp, semibold, a little tight, as in the mockup.
        headlineLarge = t.headlineLarge.f().copy(fontSize = 28.sp, lineHeight = 32.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.02).em),
        headlineMedium = t.headlineMedium.f().copy(fontWeight = FontWeight.SemiBold, letterSpacing = (-0.02).em),
        headlineSmall = t.headlineSmall.f().copy(fontWeight = FontWeight.SemiBold),
        titleLarge = t.titleLarge.f().copy(fontSize = 18.sp, fontWeight = FontWeight.SemiBold),
        titleMedium = t.titleMedium.f().copy(fontWeight = FontWeight.SemiBold),
        titleSmall = t.titleSmall.f(),
        bodyLarge = t.bodyLarge.f().copy(fontSize = 15.sp, lineHeight = 21.sp, letterSpacing = 0.sp),
        bodyMedium = t.bodyMedium.f().copy(fontSize = 13.sp, lineHeight = 18.sp, letterSpacing = 0.sp),
        bodySmall = t.bodySmall.f().copy(letterSpacing = 0.sp),
        labelLarge = t.labelLarge.f().copy(fontSize = 14.sp, letterSpacing = 0.sp),
        labelMedium = t.labelMedium.f().copy(fontSize = 12.sp, letterSpacing = 0.sp),
        labelSmall = t.labelSmall.f(),
    )
}

/** Spatial motion: a little overshoot, as Material 3 Expressive does. */
fun <T> expressiveSpring() = spring<T>(dampingRatio = 0.7f, stiffness = Spring.StiffnessMediumLow)

/** Motion for colour and opacity: no overshoot. */
fun <T> plainSpring() = spring<T>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)
