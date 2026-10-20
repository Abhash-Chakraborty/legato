package tech.abhashchakraborty.legato.ui

import android.Manifest
import android.content.pm.PackageManager
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.RepeatMode
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BinaryBitmap
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors
import tech.abhashchakraborty.legato.BluetoothLink

private const val LINK_PREFIX = "legato://pair?"

/**
 * Pairing with the PC: point the camera at the code in Legato › Settings ›
 * Devices, or paste the link from Copy link. A link opened from elsewhere
 * pairs straight away.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairScreen(link: String?, onDone: () -> Unit) {
    val legato = rememberLegato()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var pasted by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var cameraOn by remember {
        mutableStateOf(context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    val camera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { cameraOn = it }

    fun pairNow(code: String) {
        busy = true
        error = null
        scope.launch {
            val r = withContext(Dispatchers.IO) { runCatching { legato.core.pair(code.trim()) } }
            legato.refresh()
            legato.bump()
            busy = false
            r.onSuccess { onDone() }.onFailure { error = it.message ?: "That link didn't work; copy it again." }
        }
    }

    // A PC with Bluetooth on is reachable away from its Wi-Fi too, once
    // Android lets Legato use Bluetooth; ask first, and pair either way.
    var waiting by remember { mutableStateOf<String?>(null) }
    val bluetooth = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        waiting?.let { code -> waiting = null; pairNow(code) }
    }

    fun pairWith(code: String) {
        if (busy || waiting != null) return
        if (BluetoothLink.needsPermission(context) && Uri.decode(code).contains("bt:")) {
            waiting = code
            bluetooth.launch(Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            pairNow(code)
        }
    }

    LaunchedEffect(link) { if (!link.isNullOrBlank()) pairWith(link) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Pair with your PC") },
                navigationIcon = { IconButton(onClick = onDone) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                "On your PC, open Legato › Settings › Devices › Pair a phone, then scan the code.",
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(
                "Your phone syncs over Wi-Fi when it's on the PC's network and over Bluetooth when it's nearby.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Box(
                Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(28.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh),
                contentAlignment = Alignment.Center,
            ) {
                if (cameraOn && !busy) {
                    QrScanner(onCode = { pairWith(it) }, modifier = Modifier.fillMaxSize())
                    ScanFrame()
                } else if (busy) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        CircularProgressIndicator()
                        Text("Pairing…", style = MaterialTheme.typography.bodyMedium)
                    }
                } else {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Icon(Icons.Default.QrCodeScanner, contentDescription = null, modifier = Modifier.size(40.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        FilledTonalButton(onClick = { camera.launch(Manifest.permission.CAMERA) }) { Text("Use the camera") }
                    }
                }
            }
            error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
            Text("Or paste the link from Copy link.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            OutlinedTextField(
                pasted, { pasted = it; error = null }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                label = { Text("Pairing link") },
                placeholder = { Text(LINK_PREFIX + "…") },
            )
            Button(
                onClick = { pairWith(pasted) },
                enabled = pasted.trim().startsWith(LINK_PREFIX) && !busy,
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Pair") }
        }
    }
}

/** Four corners that breathe, so it's plain the camera is looking. */
@Composable
private fun ScanFrame() {
    val inset by rememberInfiniteTransition(label = "scan").animateFloat(
        initialValue = 0.2f, targetValue = 0.24f,
        animationSpec = infiniteRepeatable(tween(900), RepeatMode.Reverse),
        label = "inset",
    )
    val color = MaterialTheme.colorScheme.primary
    Canvas(Modifier.fillMaxSize()) {
        val pad = size.minDimension * inset
        val side = size.minDimension - pad * 2
        drawRoundRect(
            color = color,
            topLeft = Offset(pad, pad),
            size = Size(side, side),
            cornerRadius = CornerRadius(24.dp.toPx()),
            style = Stroke(width = 4.dp.toPx()),
        )
    }
}

/** The back camera, reading QR codes until one is a Legato pairing link. */
@Composable
private fun QrScanner(onCode: (String) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val callback by rememberUpdatedState(onCode)
    val executor = remember { Executors.newSingleThreadExecutor() }
    DisposableEffect(Unit) {
        onDispose {
            val future = ProcessCameraProvider.getInstance(context)
            if (future.isDone) runCatching { future.get().unbindAll() }
            executor.shutdown()
        }
    }
    AndroidView(
        modifier = modifier,
        factory = { ctx ->
            val view = PreviewView(ctx).apply { scaleType = PreviewView.ScaleType.FILL_CENTER }
            val future = ProcessCameraProvider.getInstance(ctx)
            future.addListener({
                val provider = future.get()
                val preview = Preview.Builder().build().also { it.setSurfaceProvider(view.surfaceProvider) }
                val reader = QRCodeReader()
                var found = false
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(executor) { image ->
                    try {
                        if (!found) {
                            val plane = image.planes[0]
                            val buffer = plane.buffer
                            val bytes = ByteArray(buffer.remaining()).also { buffer.get(it) }
                            val source = PlanarYUVLuminanceSource(bytes, plane.rowStride, image.height, 0, 0, image.width, image.height, false)
                            val text = runCatching { reader.decode(BinaryBitmap(HybridBinarizer(source))).text }.getOrNull()
                            reader.reset()
                            if (text != null && text.startsWith(LINK_PREFIX)) {
                                found = true
                                ContextCompat.getMainExecutor(ctx).execute { callback(text) }
                            }
                        }
                    } finally {
                        image.close()
                    }
                }
                runCatching {
                    provider.unbindAll()
                    provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                }
            }, ContextCompat.getMainExecutor(ctx))
            view
        },
    )
}
