package org.terminus.mobile.ui.lock

import android.content.Context
import android.content.ContextWrapper
import android.os.Build
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import org.terminus.mobile.R

/**
 * Biometrik ATAU kunci layar HP (PIN/pola/sandi) — mobile/DESIGN.md bagian
 * 3. API 29 tidak mendukung kombinasi BIOMETRIC_STRONG|DEVICE_CREDENTIAL,
 * jadi di sana dipakai BIOMETRIC_WEAK|DEVICE_CREDENTIAL.
 */
private val authenticators: Int =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) BIOMETRIC_STRONG or DEVICE_CREDENTIAL else BIOMETRIC_WEAK or DEVICE_CREDENTIAL

/** HP punya kunci layar/biometrik yang bisa dipakai? null = bisa; selain itu pesan kenapa tidak. */
fun deviceAuthUnavailableReason(context: Context): String? =
    when (BiometricManager.from(context).canAuthenticate(authenticators)) {
        BiometricManager.BIOMETRIC_SUCCESS -> null
        BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> "Atur kunci layar (PIN/pola/sandi) atau sidik jari di Setelan HP dulu."
        else -> "HP ini tidak mendukung kunci biometrik / kunci layar."
    }

/**
 * Tampilkan prompt sistem. [onError] menerima pesan (dibatalkan user juga
 * dianggap error, pesannya dari sistem).
 */
fun authenticate(context: Context, title: String, subtitle: String, onSuccess: () -> Unit, onError: (String) -> Unit) {
    val activity = context.findFragmentActivity() ?: return onError("Tidak bisa menampilkan kunci di layar ini.")
    val prompt = BiometricPrompt(
        activity,
        ContextCompat.getMainExecutor(activity),
        object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) = onSuccess()

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) = onError(errString.toString())
        },
    )
    // Tanpa tombol negatif: dengan DEVICE_CREDENTIAL, sistem yang menyediakan
    // pilihan "pakai PIN/pola" & batal.
    prompt.authenticate(
        BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle(subtitle)
            .setAllowedAuthenticators(authenticators)
            .build(),
    )
}

private tailrec fun Context.findFragmentActivity(): FragmentActivity? = when (this) {
    is FragmentActivity -> this
    is ContextWrapper -> baseContext.findFragmentActivity()
    else -> null
}

/**
 * Layar kunci — menutupi SELURUH isi app. Prompt langsung muncul sekali;
 * kalau dibatalkan, tombol "Buka kunci" mengulanginya.
 */
@Composable
fun LockScreen(onUnlocked: () -> Unit) {
    val context = LocalContext.current
    var message by remember { mutableStateOf<String?>(null) }
    val prompt = {
        message = null
        authenticate(
            context,
            title = "Buka Terminus",
            subtitle = "Pakai sidik jari/wajah atau kunci layar HP",
            onSuccess = onUnlocked,
            onError = { message = it },
        )
    }
    LaunchedEffect(Unit) { prompt() }

    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(painterResource(R.drawable.ic_key), contentDescription = null, modifier = Modifier.size(48.dp))
        Text("Terminus terkunci", style = MaterialTheme.typography.headlineSmall, modifier = Modifier.padding(top = 16.dp))
        Text(
            "Sesi SSH & SFTP tetap berjalan di belakang.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp),
        )
        message?.let {
            Text(it, color = MaterialTheme.colorScheme.error, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 16.dp))
        }
        Button(onClick = prompt, modifier = Modifier.padding(top = 24.dp)) { Text("Buka kunci") }
    }
}
