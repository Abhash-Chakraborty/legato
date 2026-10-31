package tech.abhashchakraborty.legato.ui

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.automirrored.filled.StickyNote2
import androidx.compose.material.icons.filled.GraphicEq
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.automirrored.outlined.StickyNote2
import androidx.compose.material.icons.outlined.GraphicEq
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.navigation.NavType
import tech.abhashchakraborty.legato.ModelStatus

/** The five tabs and the screens behind them. */
sealed class Tab(val route: String, val label: String, val icon: ImageVector, val iconOn: ImageVector) {
    data object Home : Tab("home", "Home", Icons.Outlined.Home, Icons.Filled.Home)
    data object Words : Tab("words", "Words", Icons.AutoMirrored.Outlined.MenuBook, Icons.AutoMirrored.Filled.MenuBook)
    data object Scratchpad : Tab("scratchpad", "Scratchpad", Icons.AutoMirrored.Outlined.StickyNote2, Icons.AutoMirrored.Filled.StickyNote2)
    data object Notes : Tab("notes", "Notes", Icons.Outlined.GraphicEq, Icons.Filled.GraphicEq)
    data object Settings : Tab("settings", "Settings", Icons.Outlined.Settings, Icons.Filled.Settings)

    companion object {
        val all = listOf(Home, Words, Scratchpad, Notes, Settings)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun LegatoApp(pairingLink: String?) {
    val nav = rememberNavController()
    val legato = rememberLegato()
    val state by legato.state.collectAsStateWithLifecycle()
    val setupDone = remember { legato.prefs.getBoolean("setup_done", false) }
    val snackbar = remember { SnackbarHostState() }
    // Failures from anywhere; the connection chatter stays in Advanced.
    LaunchedEffect(Unit) { legato.notices.collect { snackbar.showSnackbar(it) } }
    // A model downloaded on an earlier run loads in the background.
    // Ready for the first recording, unless the PC writes them up; put away again after a while unused.
    LaunchedEffect(Unit) { if (legato.model.value is ModelStatus.Downloaded && !legato.pcWillTranscribe()) legato.loadModel() }
    LaunchedEffect(pairingLink) {
        if (pairingLink != null) nav.navigate("pair?link=${android.net.Uri.encode(pairingLink)}")
    }

    val entry by nav.currentBackStackEntryAsState()
    val onTab = Tab.all.any { tab -> entry?.destination?.hierarchy?.any { it.route == tab.route } == true }
    // A tablet on its side: the tabs move to a rail and the pages stop at a
    // width that still reads well, instead of stretching across the screen.
    val wide = LocalConfiguration.current.screenWidthDp >= 840
    fun selected(tab: Tab) = entry?.destination?.hierarchy?.any { it.route == tab.route } == true
    fun go(tab: Tab) = nav.navigate(tab.route) {
        popUpTo(nav.graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        // Each screen has its own Scaffold for the system bars; this one
        // only makes room for the tabs.
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        bottomBar = {
            if (onTab && !wide) {
                NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh) {
                    Tab.all.forEach { tab ->
                        val on = selected(tab)
                        NavigationBarItem(
                            selected = on,
                            onClick = { go(tab) },
                            icon = { Icon(if (on) tab.iconOn else tab.icon, contentDescription = null) },
                            label = { Text(tab.label) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        Row(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) {
            if (onTab && wide) {
                NavigationRail(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh) {
                    Spacer(Modifier.weight(1f))
                    Tab.all.forEach { tab ->
                        val on = selected(tab)
                        NavigationRailItem(
                            selected = on,
                            onClick = { go(tab) },
                            icon = { Icon(if (on) tab.iconOn else tab.icon, contentDescription = null) },
                            label = { Text(tab.label) },
                            modifier = Modifier.padding(vertical = 4.dp),
                        )
                    }
                    Spacer(Modifier.weight(1f))
                }
            }
            Box(Modifier.weight(1f).fillMaxHeight(), contentAlignment = Alignment.TopCenter) {
                NavHost(
                    navController = nav,
                    startDestination = if (setupDone) Tab.Home.route else "setup",
                    modifier = Modifier.widthIn(max = 760.dp).fillMaxSize(),
                    enterTransition = { fadeIn(tween(220)) + slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.Up, tween(220)) { it / 12 } },
                    exitTransition = { fadeOut(tween(160)) },
                    popEnterTransition = { fadeIn(tween(220)) },
                    popExitTransition = { fadeOut(tween(160)) },
                ) {
                    composable("setup") { SetupScreen(onDone = { nav.navigate(Tab.Home.route) { popUpTo("setup") { inclusive = true } } }, onPair = { nav.navigate("pair") }) }
                    composable(Tab.Home.route) { HomeScreen(onSettings = { nav.navigate(Tab.Settings.route) }, onPair = { nav.navigate("pair") }) }
                    composable(Tab.Words.route) { WordsScreen() }
                    composable(Tab.Scratchpad.route) { ScratchpadScreen(onOpen = { id -> nav.navigate("scratch/$id") }) }
                    composable("scratch/{id}", arguments = listOf(navArgument("id") { type = NavType.LongType })) {
                        ScratchEditScreen(id = it.arguments?.getLong("id") ?: -1L, onBack = { nav.popBackStack() })
                    }
                    composable(Tab.Notes.route) { NotesScreen(onOpen = { id -> nav.navigate("note/$id") }) }
                    composable("note/{id}", arguments = listOf(navArgument("id") { type = NavType.LongType })) {
                        NoteScreen(id = it.arguments?.getLong("id") ?: -1L, onBack = { nav.popBackStack() })
                    }
                    composable(Tab.Settings.route) { SettingsScreen(onPair = { nav.navigate("pair") }, onDebug = { nav.navigate("debug") }, onGuide = { nav.navigate("setup") }) }
                    composable(
                        "pair?link={link}",
                        arguments = listOf(navArgument("link") { type = NavType.StringType; nullable = true; defaultValue = null }),
                    ) { PairScreen(link = it.arguments?.getString("link"), onDone = { nav.popBackStack() }) }
                    composable("debug") { AdvancedScreen(onBack = { nav.popBackStack() }) }
                }
            }
        }
    }
    WhatsNew(legato)
    UpdateOffer(legato)
}
