package com.lilia.remote

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

@Composable
fun PairingQrScanner(
    onPairingUri: (String) -> Unit,
    onClose: () -> Unit,
    onError: (String) -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) {
        context.findActivity() as? LifecycleOwner
    }
    val executor = remember { Executors.newSingleThreadExecutor() }
    val scanner = remember {
        BarcodeScanning.getClient(
            BarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                .build(),
        )
    }
    val emitted = remember { AtomicBoolean(false) }

    DisposableEffect(context, lifecycleOwner, scanner, executor) {
        onDispose {
            scanner.close()
            executor.shutdown()
            unbindCameraWhenReady(context)
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        if (lifecycleOwner == null) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.background)
                    .padding(20.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text("Camera lifecycle is unavailable.", color = Color(0xFFE07A6F))
                Button(onClick = onClose, modifier = Modifier.padding(top = 12.dp)) {
                    Text("Close")
                }
            }
            return@Box
        }

        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { viewContext ->
                PreviewView(viewContext).apply {
                    implementationMode = PreviewView.ImplementationMode.COMPATIBLE
                    startCamera(
                        context = viewContext,
                        lifecycleOwner = lifecycleOwner,
                        previewView = this,
                        analyzerExecutor = executor,
                        onError = onError,
                        analyzer = { imageProxy ->
                            scanImage(
                                imageProxy = imageProxy,
                                scanner = scanner,
                                emitted = emitted,
                                onPairingUri = onPairingUri,
                                onError = onError,
                            )
                        },
                    )
                }
            },
        )

        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(Color(0xCC151716))
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                "Scan the Lilia Remote pairing QR code",
                color = MaterialTheme.colorScheme.onBackground,
                style = MaterialTheme.typography.titleMedium,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    onClick = onClose,
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Cancel")
                }
            }
        }
    }
}

private fun startCamera(
    context: Context,
    lifecycleOwner: LifecycleOwner,
    previewView: PreviewView,
    analyzerExecutor: java.util.concurrent.ExecutorService,
    onError: (String) -> Unit,
    analyzer: ImageAnalysis.Analyzer,
) {
    val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
    cameraProviderFuture.addListener(
        {
            runCatching {
                val cameraProvider = cameraProviderFuture.get()
                if (!cameraProvider.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA)) {
                    error("No back camera is available on this device.")
                }
                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(previewView.surfaceProvider)
                }
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                    .also {
                        it.setAnalyzer(analyzerExecutor, analyzer)
                    }
                cameraProvider.unbindAll()
                cameraProvider.bindToLifecycle(
                    lifecycleOwner,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    analysis,
                )
            }.onFailure { error ->
                onError(error.message ?: "Failed to start camera")
            }
        },
        ContextCompat.getMainExecutor(context),
    )
}

private fun unbindCameraWhenReady(context: Context) {
    val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
    cameraProviderFuture.addListener(
        {
            runCatching {
                cameraProviderFuture.get().unbindAll()
            }
        },
        ContextCompat.getMainExecutor(context),
    )
}

private fun scanImage(
    imageProxy: ImageProxy,
    scanner: com.google.mlkit.vision.barcode.BarcodeScanner,
    emitted: AtomicBoolean,
    onPairingUri: (String) -> Unit,
    onError: (String) -> Unit,
) {
    if (emitted.get()) {
        imageProxy.close()
        return
    }
    val mediaImage = imageProxy.image
    if (mediaImage == null) {
        imageProxy.close()
        return
    }
    val image = InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
    scanner.process(image)
        .addOnSuccessListener { barcodes ->
            val value = barcodes.firstNotNullOfOrNull { barcode ->
                barcode.rawValue?.takeIf { it.startsWith("lilia-remote://pair") }
            }
            if (value != null && emitted.compareAndSet(false, true)) {
                onPairingUri(value)
            }
        }
        .addOnFailureListener { error ->
            onError(error.message ?: "Failed to scan QR code")
        }
        .addOnCompleteListener {
            imageProxy.close()
        }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}
